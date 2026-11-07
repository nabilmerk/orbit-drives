//! The per-tool failure vocabularies and the judge that reads a log
//! against them.
//!
//! Every pattern in a vocabulary is a probed fact: it is copied from a
//! live run of the tool, never invented. A tool joins the registry only
//! after its failure faces have been read off a real log. Matching is
//! case-exact substring search, and nothing is line-anchored, because the
//! tools do not speak that way: ghdl opens its failure lines with the
//! offending file path or the image name, not with the severity, while
//! nvc leads every diagnostic with its own `** Severity:` marker.
//!
//! The judge is the gate for one reason probed live: ghdl exits 0 on a
//! violated `severity error` check -- it prints the assertion-error face
//! and continues -- so an exit-code gate alone would pass a failing run.
//! The judge reads the log, where the face always lands. nvc's exits are
//! honest so far as probed (a severity-error run continues but still
//! exits 1), so there the judge is the second witness, not the only one:
//! either witness convicts, the same as anywhere else in this crate.
//! The Verilog-native tools split the same two ways: iverilog's compile
//! exits are honest but vvp exits 0 on a `$error` call -- ghdl's hole
//! again, the judge load-bearing there -- while a verilator model dies
//! by SIGABRT on every probed failure (no exit code at all, the exit
//! witness always convicts, the judge second).

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
    // stops the simulation (exit 1). Also catches the report-statement
    // stop (`./tb_fail:error: report failed`), probed on the same ghdl: a
    // `report ... severity failure` statement stops the run through this
    // run-level line, and so does a `report ... severity error` statement
    // once the drive pins `--assert-level=error` onto the run argv.
    ":error:",
    // `tb_fail.vhd:9:5:@1ns:(assertion failure): the design is wrong` --
    // the point-of-failure face of a violated `severity failure` assert.
    "(assertion failure)",
    // `tb_err.vhd:9:5:@0ms:(assertion error): check failed` -- the face of
    // a violated `severity error` assert, after which ghdl continues and
    // exits 0; the log is the only gate on this failure.
    "(assertion error)",
];

/// nvc's failure faces, probed live on nvc 1.23.0 (2026-11-07, `nvc
/// --std=2008 -a/-e/-r`): every diagnostic nvc writes leads with its own
/// severity marker, so the faces anchor on the `** Severity:` shapes. A
/// violated `assert ... severity failure` stops the run -- `** Failure:
/// 0ms+0: the design is wrong`, then `   Process :tb_fail:_p0 at
/// tb_fail.vhd:5` -- and exits 1, honest. A violated `severity error`
/// check (or a `report ... severity error` statement) prints `** Error:
/// 0ms+0: check failed`, CONTINUES the simulation -- `** Note: 0ms+0:
/// simulation continued` follows -- and STILL exits 1: unlike bare ghdl,
/// whose severity-error run exits 0, nvc's exit convicts on its own, and
/// the face is the second witness. The same `** Error:` marker fronts
/// nvc's analysis errors (`** Error: unexpected end while parsing port
/// clause, ...` above a `> broken.vhd:3` caret block) and its
/// missing-design-unit errors (`** Error: design unit GATE not found in
/// library WORK`), and `** Fatal:` fronts the elaboration refusal
/// (`** Fatal: cannot find unit WORK.TB_NOSUCH in library WORK`) and the
/// option errors (`** Fatal: unrecognised analysis option --std=2008`,
/// probed with the standard flag misplaced after the command -- nvc's
/// global options lead the argv, the opposite of ghdl's command-first
/// shape). The gentler neighbors (`** Warning:`, `** Note:`, probed on a
/// continuing run that exits 0) are not failures and stay out.
/// Case-exact: nvc capitalizes its markers, so a lowercase drift
/// (`** error:`, `** failure:`) is not one of its faces and never judges
/// a log.
pub const NVC_VOCABULARY: &[&str] = &[
    // `** Failure: 0ms+0: the design is wrong` -- the run-level face of a
    // violated `assert ... severity failure` (exit 1, run stopped) and of
    // a `report ... severity failure` statement (`** Failure: 0ms+0: stop
    // now`, exit 1), probed on the same nvc.
    "** Failure:",
    // `** Error: 0ms+0: check failed` -- the face of a violated `severity
    // error` assert or `report ... severity error` statement, after which
    // nvc continues but still exits 1; also the analysis-stage marker
    // (`** Error: unexpected end while parsing port clause, ...`,
    // `** Error: design unit GATE not found in library WORK`), which a
    // run log never carries but the vocabulary records as probed.
    "** Error:",
    // `** Fatal: cannot find unit WORK.TB_NOSUCH in library WORK` -- the
    // elaboration refusal when the top names no analyzed unit, and the
    // face of every option error (`** Fatal: unrecognised analysis option
    // --std=2008`); always exit 1, probed.
    "** Fatal:",
];

/// iverilog's failure faces, probed live on Icarus Verilog 12.0 (stable)
/// (2026-11-07, `iverilog -g2012` + `vvp`): a testbench calling
/// `$fatal(1, "the design is wrong")` stops the run -- vvp prints
/// `FATAL: tb_fatal.v:4: the design is wrong` and the indented
/// `       Time: 1  Scope: tb_fatal` beneath it, and exits 1, honest. But
/// a `$error("check failed")` call is vvp's hole, probed: vvp prints
/// `ERROR: tb_error.v:4: check failed` with the same Time/Scope lines,
/// CONTINUES the simulation -- the testbench's own `$display` output and
/// the `tb_error.v:6: $finish called at 1 (1s)` line follow -- and EXITS
/// 0, exactly like bare ghdl's severity-error exit. The exit gate alone
/// would pass a failing run; the face in the log is the only witness.
/// The compile faces are recorded too, per the nvc precedent: the parse
/// face (`broken.v:2: syntax error`, on stderr, exit 2) and the
/// unknown-root refusal (`error: Unable to find the root module
/// "nosuch_top" in the Verilog source.`, exit 1) never ride a run log --
/// the compile step gates on the exit -- but they are probed and pinned.
/// The gentler neighbors (`WARNING: tb_warn.v:4: check flagged` on a
/// continuing run that exits 0; `$stop`'s `** VVP Stop(0) **` banner,
/// also exit 0) are not failures and stay out. Case-exact: vvp spells
/// its failure faces in uppercase, so a lowercase drift (`fatal:`,
/// `error:`) is not one of its faces and never judges a log.
pub const IVERILOG_VOCABULARY: &[&str] = &[
    // `FATAL: tb_fatal.v:4: the design is wrong` -- the point-of-failure
    // face of a `$fatal` call (exit 1, run stopped), probed on vvp 12.0.
    // Uppercase, with the indented `       Time: 1  Scope: tb_fatal`
    // lines beneath it.
    "FATAL:",
    // `ERROR: tb_error.v:4: check failed` -- the face of a `$error` call,
    // after which vvp CONTINUES and exits 0 (probed live): the face is
    // the only witness on that failure, the judge's reason to exist for
    // this tool. Recorded too: the compile faces (`: syntax error` on a
    // parse error, `Unable to find the root module` on a `-s` naming no
    // module) never ride a run log -- the compile gates on the exit --
    // but they are probed and pinned.
    "ERROR:",
    // `broken.v:2: syntax error` -- the parse face of a compile the
    // driver never gets past (exit 2, stderr); the compile step gates on
    // the exit and is never judged, the face is recorded.
    ": syntax error",
    // `error: Unable to find the root module "nosuch_top" in the Verilog
    // source.` -- the refusal when a `-s` top names no module in the
    // compiled source (exit 1, probed): the top names are validated at
    // compile time, so a build drive carries the same check for free.
    "Unable to find the root module",
];

/// verilator's failure faces, probed live on Verilator 5.032 (2026-11-07,
/// `verilator --lint-only --timing` and `verilator --binary --build
/// --timing`, model run `./obj_dir/V<top>`): every diagnostic verilator
/// writes -- lint, build, and the built model's runtime -- leads with its
/// own `%Severity:` marker, so the faces anchor on the `%Error`/`%Fatal:`
/// shapes. The lint face is `%Error: broken.v:2:3: syntax error, ...`
/// above a `    2 |   input a;` / `      |   ^~~~~` caret block, closing
/// with `%Error: Exiting due to 1 error(s)`, exit 1. In the built model a
/// `$fatal(1, "the design is wrong")` prints
/// `%Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is
/// wrong`, then `%Error: tb_fatal.sv:8: Verilog $stop` and `Aborting...`,
/// and the model dies by SIGABRT -- no exit code at all (the shell
/// reports `Aborted`; a raw spawn sees the signal, not a code, and the
/// drive records -1) -- so the exit witness convicts on every model
/// failure probed, and the faces are the second witness. `$error` and
/// even a plain `$stop` ride the same `%Error:` faces to the same SIGABRT
/// (the built model, unlike vvp, has no interactive continue), and
/// `$warning` is the gentler neighbor (`[1000] %Warning:
/// tb_vwarn.sv:8: tb_vwarn: check flagged`, continuing, exit 0) -- never
/// judged. The bare `%Error` face (no colon) is deliberate: it subsumes
/// the coded lint faces too (`%Error-NEEDTIMINGOPT: tb.sv:7:5: Use
/// --timing or --no-timing to specify how delays should be handled`,
/// probed on a lint without `--timing`), which share no shape with
/// `%Error:`. Case-exact: verilator capitalizes its markers, so a
/// lowercase drift (`%error:`, `%fatal:`) is not one of its faces and
/// never judges a log.
pub const VERILATOR_VOCABULARY: &[&str] = &[
    // `%Error: broken.v:2:3: syntax error, ...` (the lint face, exit 1),
    // `%Error: Specified --top-module 'nosuch' was not found in design.`
    // (the lint-time refusal of an unknown top, exit 1),
    // `%Error: tb_err.sv:8: Assertion failed in tb_err: check failed`
    // (the model's `$error`, SIGABRT), `%Error: tb_stop.sv:9: Verilog
    // $stop` (even a plain `$stop` aborts the model), `%Error: Exiting
    // due to 1 error(s)` (the closing line), and the coded lint faces
    // (`%Error-NEEDTIMINGOPT: ...`) -- the bare `%Error` face subsumes
    // them all, probed on verilator 5.032.
    "%Error",
    // `%Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is
    // wrong` -- the point-of-failure face of a model `$fatal` (the
    // `%Error: ... Verilog $stop` line follows it, and the model dies by
    // SIGABRT), probed on verilator 5.032.
    "%Fatal:",
];

/// yosys's failure faces, probed live on Yosys 0.52 (2026-11-07, `yosys
/// -p "read_verilog broken.v; synth"` and siblings): every probed failure
/// face carries the uppercase `ERROR:` marker -- line-start for the
/// script faces (`ERROR: Module \`nosuch' not found!` under a `synth
/// -top` naming no module, `ERROR: No such command: frobnicate ...` under
/// an unknown `-p` command, `ERROR: Can't open output file ... for
/// writing` under a failed netlist write, `ERROR: Module \`\gate'
/// referenced in module `\top' ... is not part of the design` under an
/// instantiation whose provider was never read) and mid-line for the
/// frontend's syntax face, which leads with the source location
/// (`broken.v:3: ERROR: syntax error, unexpected TOK_ENDMODULE`). yosys
/// writes its whole log to stdout and its failure faces to stderr; the
/// drive joins the streams, so the face reads either way. The exit is
/// honest so far as probed: every probed failure exits 1 -- a script
/// error aborts the whole `-p` script before the next command runs -- so
/// the judge is the second witness on this build-only drive, the exit
/// the first. The gentler neighbor (`Warning:` lines on a clean synth,
/// probed) is not a failure and stays out. Case-exact: yosys spells its
/// marker in uppercase, so a lowercase drift (`error:`) is not one of
/// its faces and never judges a log.
pub const YOSYS_VOCABULARY: &[&str] = &["ERROR:"];

/// A tool carrying a probed failure vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// The GHDL simulator, `ghdl` (VHDL only).
    Ghdl,
    /// The nvc simulator, `nvc` (VHDL only).
    Nvc,
    /// The Icarus Verilog compiler and its `vvp` runtime (Verilog/SV
    /// only).
    Iverilog,
    /// The Verilator lint and model builder (Verilog/SV only).
    Verilator,
    /// The yosys synthesis engine (`yosys`, Verilog/SV synthesis only).
    Yosys,
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
            Tool::Nvc => "nvc",
            Tool::Iverilog => "iverilog",
            Tool::Verilator => "verilator",
            Tool::Yosys => "yosys",
        }
    }

    /// The tool's probed failure vocabulary: case-exact substrings, none
    /// line-anchored.
    pub fn vocabulary(self) -> &'static [&'static str] {
        match self {
            Tool::Ghdl => GHDL_VOCABULARY,
            Tool::Nvc => NVC_VOCABULARY,
            Tool::Iverilog => IVERILOG_VOCABULARY,
            Tool::Verilator => VERILATOR_VOCABULARY,
            Tool::Yosys => YOSYS_VOCABULARY,
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
    &[
        Tool::Ghdl,
        Tool::Nvc,
        Tool::Iverilog,
        Tool::Verilator,
        Tool::Yosys,
    ]
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
