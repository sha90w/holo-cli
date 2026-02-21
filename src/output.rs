//
// Copyright (c) The Holo Core Contributors
//
// SPDX-License-Identifier: MIT
//

use std::io::{self, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

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
