//
// Copyright (c) The Holo Core Contributors
//
// SPDX-License-Identifier: MIT
//

use std::io::{self, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::{self, JoinHandle};

// ===== PagerWriter =====

/// A `Write` wrapper that pipes output to the `less` pager.
///
/// When dropped, closes stdin (signalling EOF) and waits for `less` to exit.
pub struct PagerWriter {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl PagerWriter {
    pub fn new() -> io::Result<Self> {
        let mut child = Command::new("less")
            // Exit immediately if the data fits on one screen.
            .arg("-F")
            // Do not clear the screen on exit.
            .arg("-X")
            .stdin(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take();
        Ok(PagerWriter { child, stdin })
    }
}

impl Write for PagerWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.stdin {
            Some(stdin) => stdin.write(buf),
            None => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.stdin {
            Some(stdin) => stdin.flush(),
            None => Ok(()),
        }
    }
}

impl Drop for PagerWriter {
    fn drop(&mut self) {
        // Close stdin first so `less` receives EOF and can exit.
        drop(self.stdin.take());
        // Wait for the pager process to finish.
        let _ = self.child.wait();
    }
}

// ===== GrepWriter =====

/// A `Write` wrapper that pipes output through the system `grep` binary.
///
/// The show command writes to this wrapper, which forwards the bytes to grep's
/// stdin.  A background thread concurrently reads grep's stdout and copies it
/// to the downstream writer.  On drop, stdin is closed (EOF → grep finishes),
/// the thread is joined, and the child process is waited on.
pub struct GrepWriter {
    stdin: Option<ChildStdin>,
    output_thread: Option<JoinHandle<()>>,
    child: Child,
}

impl GrepWriter {
    pub fn new(
        downstream: Box<dyn Write + Send>,
        args: Vec<String>,
    ) -> io::Result<Self> {
        let mut child = Command::new("grep")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().unwrap();

        let output_thread = thread::spawn(move || {
            let mut downstream = downstream;
            let _ = io::copy(&mut stdout, &mut downstream);
            let _ = downstream.flush();
        });

        Ok(GrepWriter {
            stdin: Some(stdin.unwrap()),
            output_thread: Some(output_thread),
            child,
        })
    }
}

impl Write for GrepWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.stdin {
            Some(stdin) => stdin.write(buf),
            None => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.stdin {
            Some(stdin) => stdin.flush(),
            None => Ok(()),
        }
    }
}

impl Drop for GrepWriter {
    fn drop(&mut self) {
        // Signal EOF to grep by closing its stdin.
        drop(self.stdin.take());
        // Wait for the output thread to drain grep's stdout.
        if let Some(thread) = self.output_thread.take() {
            let _ = thread.join();
        }
        // Wait for the grep process to exit.
        let _ = self.child.wait();
    }
}

// ===== FilterWriter =====

/// A `Write` wrapper that filters lines based on a string pattern.
///
/// When `include` is `true`, only lines *containing* the pattern are forwarded
/// to the downstream writer.  When `include` is `false`, lines containing the
/// pattern are dropped.
pub struct FilterWriter<W: Write> {
    downstream: W,
    pattern: String,
    include: bool,
    buf: Vec<u8>,
}

impl<W: Write> FilterWriter<W> {
    pub fn new(downstream: W, pattern: String, include: bool) -> Self {
        FilterWriter {
            downstream,
            pattern,
            include,
            buf: Vec::new(),
        }
    }

    fn emit_line(&mut self, line: &[u8]) -> io::Result<()> {
        let line_str = String::from_utf8_lossy(line);
        let matches = line_str.contains(self.pattern.as_str());
        if matches == self.include {
            self.downstream.write_all(line)?;
        }
        Ok(())
    }
}

impl<W: Write> Write for FilterWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let total = data.len();
        self.buf.extend_from_slice(data);

        // Process all complete lines.
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            self.emit_line(&line)?;
        }

        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Flush any remaining partial line (no trailing newline).
        if !self.buf.is_empty() {
            let line = std::mem::take(&mut self.buf);
            self.emit_line(&line)?;
        }
        self.downstream.flush()
    }
}
