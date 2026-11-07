//! The per-tool failure vocabularies and the judge that reads a log
//! against them.
//!
//! Every pattern in a vocabulary is a probed fact: it is copied from a
//! live run of the tool, never invented. A tool joins the registry only
//! after its failure faces have been read off a real log. Matching is
//! case-exact substring search, and nothing is line-anchored, because the
//! tools do not speak that way: ghdl opens its failure lines with the
//! offending file path or the image name, not with the severity.
//!
//! The judge is the gate for one reason probed live: ghdl exits 0 on a
//! violated `severity error` check -- it prints the assertion-error face
//! and continues -- so an exit-code gate alone would pass a failing run.
//! The judge reads the log, where the face always lands.

use std::fmt;
use std::str::FromStr;

/// ghdl's failure faces, probed live on ghdl 6.0.0 (2026-11-07, `ghdl -r
/// --std=08`): a testbench that stops on a violated `assert ... severity
/// failure` exits 1 and prints `tb_fail.vhd:9:5:@1ns:(assertion failure):
/// the design is wrong` at the point of the failed assertion, then
/// `./tb_fail:error: assertion failed` and `./tb_fail:error: simulation
/// failed`. A violated `severity error` check is a failure of the check
/// too: ghdl prints `tb_err.vhd:9:5:@0ms:(assertion error): check failed`
/// and CONTINUES, exiting 0 -- the honest exit gate sees nothing there, so
/// the log gate is the only gate that catches it. The faces are mid-line --
/// the lines open with the source path or the image name -- so the
/// patterns anchor on the punctuation ghdl wraps its severities in.
/// Case-exact: ghdl spells its failures in lowercase, so an uppercase
/// drift (`ERROR:`, `FATAL:`, `(ASSERTION FAILURE)`) is not one of its
/// faces and never judges a log. The gentler neighbors (`(assertion
/// warning)`, `(assertion note)`, probed on a continuing run) are not
/// failures and stay out.
pub const GHDL_VOCABULARY: &[&str] = &[
    // `./tb_fail:error: assertion failed` and `./tb_fail:error: simulation
    // failed` -- the run-level faces ghdl prints when a violated assertion
    // stops the simulation (exit 1). Also catches the report-shaped stop
    // (`./tb_fail:error: report failed`), probed on the same ghdl: a
    // statically-false `assert false report ... severity failure` renders
    // as a report face but exits 1 through this run-level line.
    ":error:",
    // `tb_fail.vhd:9:5:@1ns:(assertion failure): the design is wrong` --
    // the point-of-failure face of a violated `severity failure` assert.
    "(assertion failure)",
    // `tb_err.vhd:9:5:@0ms:(assertion error): check failed` -- the face of
    // a violated `severity error` assert, after which ghdl continues and
    // exits 0; the log is the only gate on this failure.
    "(assertion error)",
];

/// A tool carrying a probed failure vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// The GHDL simulator, `ghdl` (VHDL only).
    Ghdl,
}

/// The error for a tool name that resolves to nothing in the registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTool {
    /// The name that carried no vocabulary.
    pub name: String,
}

impl fmt::Display for UnknownTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = tools().iter().map(|tool| tool.name()).collect();
        write!(
            f,
            "no probed failure vocabulary for tool {:?}: judging is never guessed; carried tools: {}",
            self.name,
            names.join(", ")
        )
    }
}

impl std::error::Error for UnknownTool {}

impl Tool {
    /// The tool's command-line name.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Ghdl => "ghdl",
        }
    }

    /// The tool's probed failure vocabulary: case-exact substrings, none
    /// line-anchored.
    pub fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Tool::Ghdl => GHDL_VOCABULARY,
        }
    }
}

impl FromStr for Tool {
    type Err = UnknownTool;

    /// Resolve a tool from its command-line name. A name outside the
    /// probed registry is an error, never a guess.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match tools().iter().find(|tool| tool.name() == s) {
            Some(tool) => Ok(*tool),
            None => Err(UnknownTool {
                name: s.to_string(),
            }),
        }
    }
}

/// The registry: every tool carrying a probed failure vocabulary.
pub fn tools() -> &'static [Tool] {
    &[Tool::Ghdl]
}

/// Judge a tool's log text against its probed failure vocabulary.
///
/// Polarity: `true` is clean, `false` is findings -- the judge answers "is
/// this log clean?", never "did this fail?".
///
/// Returns `true` when the log is clean -- no failure face appears in it --
/// and `false` when any face is present. For ghdl this reads the probed
/// faces and nothing else: the gentler continuation faces (`(assertion
/// warning)`, `(assertion note)`) are not failures, a clean run whose log
/// merely mentions "ERROR" mid-text (a filename, an echoed banner) stays
/// clean, and an empty log is clean.
///
/// The search short-circuits: the first face present decides the verdict.
pub fn judge(tool: &Tool, log: &str) -> bool {
    tool.vocabulary().iter().all(|face| !log.contains(face))
}
