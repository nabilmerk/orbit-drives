//! The process plumbing every drive shares: spawn-and-capture, the lossy
//! bytes-to-text read, the stdout/stderr log join, and the verbatim block
//! relays.
//!
//! Each drive carried its own byte-identical copy of these helpers beside
//! the first drive's; one canonical copy lives here. The one parameter is
//! the binary's own name: [`capture`]'s spawn-failure message leads with
//! it (`ghdl: no such file`), and a drive that spawns more than one
//! binary passes each one's own -- iverilog's compile vs vvp's run;
//! verilator's tool vs its built model path.

use std::process::{Command, Output};

/// Spawn a command and capture its output, mapping a spawn failure to the
/// usage-class error. `bin` is the failing binary's own name -- it leads
/// the error message, so a drive spawning several binaries passes each
/// one's own.
pub(crate) fn capture(bin: &str, command: &mut Command) -> Result<Output, String> {
    command.output().map_err(|e| format!("{bin}: {e}"))
}

/// The captured bytes as text: the tools write UTF-8, and the verdict
/// must not panic on anything less.
pub(crate) fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The run log from a captured output's two streams, joined on their
/// boundary; an empty side contributes no line, so a quiet run relays no
/// stray blank line.
pub(crate) fn join_log(stdout: String, stderr: String) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (true, false) => stderr,
        (false, true) => stdout,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

/// Print a captured block verbatim, guaranteeing one final newline.
pub(crate) fn print_block(text: &str) {
    if text.is_empty() {
        return;
    }
    if text.ends_with('\n') {
        print!("{text}");
    } else {
        println!("{text}");
    }
}

/// Print a captured block verbatim to stderr, guaranteeing one final
/// newline.
pub(crate) fn eprint_block(text: &str) {
    if text.is_empty() {
        return;
    }
    if text.ends_with('\n') {
        eprint!("{text}");
    } else {
        eprintln!("{text}");
    }
}
