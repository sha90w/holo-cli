//
// Copyright (c) The Holo Core Contributors
//
// SPDX-License-Identifier: MIT
//

use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;

// ===== ParsedPipeStage =====

/// A single parsed pipe stage: the command name and its arguments as raw
/// strings, validated against `PipeRegistry` at parse time.
pub struct ParsedPipeStage {
    pub name: String,
    pub args: Vec<String>,
}

// ===== PipeImpl =====

pub enum PipeImpl {
    External {
        binary: &'static str,
        base_args: Vec<&'static str>,
    },
    /// The `&[String]` slice contains the user-supplied arguments for this
    /// pipe stage (e.g. the pattern for `include`/`exclude`/`begin`).
    Internal(fn(&[String], &mut dyn BufRead, &mut dyn Write) -> io::Result<()>),
}

// ===== PipeCommandDef =====

pub struct PipeCommandDef {
    pub name: &'static str,
    pub help: &'static str,
    /// Argument placeholder string shown in help/completion (e.g. "PATTERN"),
    /// or empty string when the command takes no arguments.
    pub args: &'static str,
    pub impl_: PipeImpl,
}

// ===== PipeRegistry =====

pub struct PipeRegistry {
    commands: Vec<PipeCommandDef>,
}

impl PipeRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn add_external(
        mut self,
        name: &'static str,
        help: &'static str,
        args: &'static str,
        binary: &'static str,
        base_args: &[&'static str],
    ) -> Self {
        self.commands.push(PipeCommandDef {
            name,
            help,
            args,
            impl_: PipeImpl::External {
                binary,
                base_args: base_args.to_vec(),
            },
        });
        self
    }

    pub fn add_internal(
        mut self,
        name: &'static str,
        help: &'static str,
        args: &'static str,
        f: fn(&[String], &mut dyn BufRead, &mut dyn Write) -> io::Result<()>,
    ) -> Self {
        self.commands.push(PipeCommandDef {
            name,
            help,
            args,
            impl_: PipeImpl::Internal(f),
        });
        self
    }

    pub fn get(&self, name: &str) -> Option<&PipeCommandDef> {
        self.commands.iter().find(|cmd| cmd.name == name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.commands.iter().map(|cmd| cmd.name)
    }
}

// ===== OutputChain =====

/// Holds the active pipe chain: the writer end the callback writes into,
/// plus all spawned processes and relay threads. Dropping this value flushes
/// the writer and waits for everything to finish.
pub struct OutputChain {
    writer: Option<Box<dyn Write + Send>>,
    children: Vec<Child>,
    threads: Vec<JoinHandle<io::Result<()>>>,
}

impl OutputChain {
    /// Detach and return the writer so it can be placed in the session.
    /// The chain teardown still happens on Drop.
    pub fn take_writer(&mut self) -> Box<dyn Write + Send> {
        self.writer.take().expect("writer already taken")
    }
}

impl Drop for OutputChain {
    fn drop(&mut self) {
        // Close the writer end first so the leftmost process gets EOF.
        drop(self.writer.take());

        // Join relay threads before waiting on processes, since threads hold
        // write-ends of intermediate pipes.
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }

        // Wait for all child processes.
        for child in self.children.iter_mut() {
            let _ = child.wait();
        }
    }
}

// ===== build_output_chain =====

/// Build a right-to-left output pipeline for `stages`, optionally with the
/// pager at the tail. Returns an `OutputChain` whose writer is the leftmost
/// stdin that the callback should write into.
pub fn build_output_chain(
    stages: &[ParsedPipeStage],
    registry: &PipeRegistry,
    use_pager: bool,
) -> io::Result<OutputChain> {
    let mut children: Vec<Child> = Vec::new();
    let mut threads: Vec<JoinHandle<io::Result<()>>> = Vec::new();

    // Start from the rightmost end.
    let mut current: Box<dyn Write + Send> = if use_pager {
        let mut pager = Command::new("less")
            .arg("-F")
            .arg("-X")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .spawn()?;
        let stdin = pager.stdin.take().expect("pager stdin");
        children.push(pager);
        Box::new(stdin)
    } else {
        Box::new(io::stdout())
    };

    // Attach pipe stages in reverse order.
    for stage in stages.iter().rev() {
        let def = registry
            .get(&stage.name)
            .expect("pipe command validated at parse time");

        current = match &def.impl_ {
            PipeImpl::External { binary, base_args } => {
                let mut child = Command::new(binary)
                    .args(base_args)
                    .args(&stage.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()?;

                let child_stdout = child.stdout.take().expect("child stdout");
                let child_stdin = child.stdin.take().expect("child stdin");

                // Thread relays child stdout → next write end.
                let next = current;
                let handle = std::thread::spawn(move || {
                    let mut src = child_stdout;
                    let mut dst = next;
                    io::copy(&mut src, &mut dst)?;
                    Ok(())
                });

                children.push(child);
                threads.push(handle);
                Box::new(child_stdin)
            }
            PipeImpl::Internal(filter_fn) => {
                let (pipe_reader, pipe_writer) = io::pipe()?;

                // Thread runs the filter: reads from pipe_reader, writes to
                // the next write end.
                let args = stage.args.clone();
                let next = current;
                let f = *filter_fn;
                let handle = std::thread::spawn(move || {
                    let mut reader = BufReader::new(pipe_reader);
                    let mut writer = next;
                    f(&args, &mut reader, &mut writer)
                });

                threads.push(handle);
                Box::new(pipe_writer)
            }
        };
    }

    Ok(OutputChain {
        writer: Some(current),
        children,
        threads,
    })
}

// ===== built-in filter functions =====

/// Include only lines that contain the pattern (args[0]).
pub fn filter_include(
    args: &[String],
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<()> {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    for line in input.lines() {
        let line = line?;
        if line.contains(pattern) {
            writeln!(output, "{}", line)?;
        }
    }
    Ok(())
}

/// Exclude lines that contain the pattern (args[0]).
pub fn filter_exclude(
    args: &[String],
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<()> {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    for line in input.lines() {
        let line = line?;
        if !line.contains(pattern) {
            writeln!(output, "{}", line)?;
        }
    }
    Ok(())
}

/// Count the number of lines and write the total.
pub fn filter_count(
    _args: &[String],
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<()> {
    let mut buf = String::new();
    let mut count: u64 = 0;
    while input.read_line(&mut buf)? > 0 {
        count += 1;
        buf.clear();
    }
    writeln!(output, "{}", count)
}

/// Pass through all lines starting from the first one that contains the
/// pattern (args[0]).
pub fn filter_begin(
    args: &[String],
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<()> {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    let mut printing = false;
    for line in input.lines() {
        let line = line?;
        if !printing && line.contains(pattern) {
            printing = true;
        }
        if printing {
            writeln!(output, "{}", line)?;
        }
    }
    Ok(())
}

/// Build the default pipe registry used at startup.
pub fn default_registry() -> PipeRegistry {
    PipeRegistry::new()
        .add_internal(
            "include",
            "Include lines matching pattern",
            "PATTERN",
            filter_include,
        )
        .add_internal(
            "exclude",
            "Exclude lines matching pattern",
            "PATTERN",
            filter_exclude,
        )
        .add_internal("count", "Count output lines", "", filter_count)
        .add_internal(
            "begin",
            "Start output at first line matching pattern",
            "PATTERN",
            filter_begin,
        )
        .add_external(
            "grep",
            "Filter output using grep with arbitrary arguments",
            "ARGS",
            "grep",
            &[],
        )
}
