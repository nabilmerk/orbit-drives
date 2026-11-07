//! The probed ghdl vocabulary, ported from the aquila contract suite
//! (upstream: `github.com/chaseruskin/aquila`, `tests/test_contracts.py`):
//! the fixture logs are the live runs, byte-for-byte, and the judgments
//! are the contract. The retired Python gate counted `error):` and `ERROR`
//! substrings, matched nothing ghdl writes, and false-failed clean logs
//! that merely mentioned "ERROR"; these pins hold that door shut.

use std::str::FromStr;

use orbit_drives::contracts::{self, Tool, UnknownTool};

// a real ghdl failure log, probed live on ghdl 6.0.0 (`ghdl -r --std=08`):
// a testbench asserting `severity failure` at 1 ns exits 1 and prints these
// faces -- every one of them mid-line, none of them uppercase
const FAILING_LOG: &str = "\
tb_fail.vhd:9:5:@1ns:(assertion failure): the design is wrong
./tb_fail:error: assertion failed
  instance: .tb_fail(sim).P0
./tb_fail:error: simulation failed
";

// a clean ghdl run's log: just the testbench's own report notes
const CLEAN_LOG: &str = "\
tb_pass.vhd:11:8:@0ns:(report note): testing started
tb_pass.vhd:14:8:@30ns:(report note): all checks passed
";

// a clean log that merely mentions "ERROR" mid-text (a filename here): the
// old gate's `data.count('ERROR')` failed runs like this one
const CLEAN_LOG_WITH_ERROR_TEXT: &str = "\
tb_pass.vhd:11:8:@0ns:(report note): reading golden vectors from ERROR_MODEL.dat
tb_pass.vhd:14:8:@30ns:(report note): all checks passed
";

// a violated `severity error` check, probed live on ghdl 6.0.0 (`ghdl -r
// --std=08`): ghdl prints the assertion-error face and CONTINUES, exiting 0
// -- the line carries ` error):` with a space before the word, so neither
// `:error:` nor `(assertion failure)` can catch it, and only the
// `(assertion error)` face judges it
const SEVERITY_ERROR_LOG: &str = "\
tb_err.vhd:8:5:@0ms:(assertion error): check failed
tb_err.vhd:9:5:@0ms:(report note): simulation continued
";

// the gentler neighbors, probed live on the same ghdl: `severity warning`
// and `severity note` faces on a continuing run -- continuations, not
// failures, so they must never judge a log
const WARNING_NOTE_LOG: &str = "\
tb_warn.vhd:8:5:@0ms:(assertion warning): check flagged
tb_warn.vhd:9:5:@0ms:(assertion note): marker logged
";

// the probed failure logs with every face drifted to uppercase: not a
// single one of ghdl's faces, since ghdl spells its failures in lowercase
const UPPERCASE_DRIFT_LOG: &str = "\
tb_fail.vhd:9:5:@1ns:(ASSERTION FAILURE): the design is wrong
./tb_fail:ERROR: assertion failed
  instance: .tb_fail(sim).P0
./tb_fail:ERROR: simulation failed
tb_err.vhd:8:5:@0ms:(ASSERTION ERROR): check failed
";

// the other uppercase drift pinned by the contract: a vocabulary ghdl does
// not speak (`FATAL:`), even at a failure-shaped line
const FATAL_DRIFT_LOG: &str = "\
tb_fail.vhd:9:5:@1ns:FATAL: the design is wrong
";

#[test]
fn failing_log_judges_fail() {
    // The probed ghdl failure log judges FAIL. The old gate counted
    // `error):` and `ERROR` in this exact log and found neither: both
    // counts were 0, so a genuinely failing run judged PASS.
    assert!(!contracts::judge(&Tool::Ghdl, FAILING_LOG));
}

#[test]
fn severity_error_log_judges_fail() {
    // A violated `severity error` check judges FAIL. ghdl continues the
    // simulation on severity error and exits 0, so the honest exit gate
    // passes it and this log face is the only gate left. The retired
    // `error):` pattern caught this line; the vocabulary must too.
    assert!(!contracts::judge(&Tool::Ghdl, SEVERITY_ERROR_LOG));
}

#[test]
fn assertion_warning_and_note_stay_clean() {
    // The probed continuation faces (`assertion warning`, `assertion
    // note`) judge PASS: they are not failures.
    assert!(contracts::judge(&Tool::Ghdl, WARNING_NOTE_LOG));
}

#[test]
fn clean_log_judges_pass() {
    // A clean ghdl log judges PASS.
    assert!(contracts::judge(&Tool::Ghdl, CLEAN_LOG));
}

#[test]
fn clean_log_with_error_text_judges_pass() {
    // A clean log that merely contains "ERROR" mid-text judges PASS. The
    // old gate's bare `ERROR` count false-failed any run whose log carried
    // the word in a filename or an echoed banner; that door is closed.
    assert!(contracts::judge(&Tool::Ghdl, CLEAN_LOG_WITH_ERROR_TEXT));
}

#[test]
fn uppercase_drift_never_matches() {
    // An uppercase drift of the real failure log judges PASS. ghdl's faces
    // are lowercase and case-exact: `:ERROR:` and `(ASSERTION FAILURE)`
    // are not among them.
    assert!(contracts::judge(&Tool::Ghdl, UPPERCASE_DRIFT_LOG));
}

#[test]
fn fatal_drift_never_matches() {
    // A `FATAL:` line judges PASS: ghdl has no such face.
    assert!(contracts::judge(&Tool::Ghdl, FATAL_DRIFT_LOG));
}

#[test]
fn each_face_alone_judges_fail() {
    // Each of ghdl's faces judges FAIL on its own, in an otherwise clean
    // log.
    for face in Tool::Ghdl.vocabulary() {
        let log =
            format!("tb_pass.vhd:11:8:@0ns:(report note): testing started\nnoise {face} tail\n");
        assert!(
            !contracts::judge(&Tool::Ghdl, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Ghdl, ""));
}

#[test]
fn unknown_tool_is_an_error() {
    // Judging a tool without a probed vocabulary is an error, never a
    // guess. The guard sits at the resolution: an unknown name never
    // becomes a Tool to judge with.
    assert_eq!(
        Tool::from_str("xsim"),
        Err(UnknownTool {
            name: "xsim".to_string(),
        })
    );
}

#[test]
fn vocabulary_registry() {
    // The registry carries exactly the probed tools, and ghdl's vocabulary
    // is the three probed faces, in probed order.
    assert_eq!(
        contracts::tools(),
        &[
            Tool::Ghdl,
            Tool::Nvc,
            Tool::Iverilog,
            Tool::Verilator,
            Tool::Yosys,
            Tool::Vivado
        ]
    );
    assert_eq!(Tool::Ghdl.name(), "ghdl");
    assert_eq!(
        Tool::Ghdl.vocabulary(),
        &[":error:", "(assertion failure)", "(assertion error)"]
    );
}

// ===== the nvc vocabulary, probed live on nvc 1.23.0, 2026-11-07 =====

// a real nvc severity-failure run (`nvc --std=2008 -r tb_fail`): the run
// stops, exit 1. nvc leads every diagnostic with its own `** Severity:`
// marker -- the opposite of ghdl's path-led lines -- and appends a
// process line indented three spaces.
const NVC_FAILING_LOG: &str = "\
** Failure: 0ms+0: the design is wrong
   Process :tb_fail:_p0 at tb_fail.vhd:5
";

// a violated `severity error` check (`nvc --std=2008 -r tb_err`): nvc
// prints the error face, CONTINUES -- the note follows -- and STILL
// exits 1, unlike bare ghdl's exit-0 hole. The face convicts regardless
// of the exit; here both witnesses agree.
const NVC_SEVERITY_ERROR_LOG: &str = "\
** Error: 0ms+0: check failed
   Process :tb_err:_p0 at tb_err.vhd:5
** Note: 0ms+0: simulation continued
   Process :tb_err:_p0 at tb_err.vhd:5
";

// the gentler neighbors, probed live on a continuing run that exits 0:
// continuations, not failures, so they must never judge a log
const NVC_WARNING_NOTE_LOG: &str = "\
** Warning: 0ms+0: check flagged
   Process :tb_warn:_p0 at tb_warn.vhd:5
** Note: 0ms+0: marker logged
   Process :tb_warn:_p0 at tb_warn.vhd:5
** Note: 0ms+0: all quiet
   Process :tb_warn:_p0 at tb_warn.vhd:5
";

// an analysis error (`nvc --std=2008 -a broken.vhd`, exit 1): the same
// `** Error:` marker nvc uses at runtime, here above its caret block. A
// run log never carries it -- the analyze step is never judged -- but the
// face is probed and recorded.
const NVC_ANALYSIS_ERROR_LOG: &str = "\
** Error: unexpected end while parsing port clause, expecting one of bus, :=, ; or )
   > broken.vhd:3
   |
 3 | end broken;
   | ^^^ this token was unexpected
";

// the elaboration refusal (`nvc --std=2008 -e tb_nosuch`, exit 1): the
// top names no analyzed unit
const NVC_ELAB_FAILURE_LOG: &str = "\
** Fatal: cannot find unit WORK.TB_NOSUCH in library WORK
";

// the lowercase drift of every nvc face: nvc capitalizes its markers, so
// none of these is a face it writes
const NVC_LOWERCASE_DRIFT_LOG: &str = "\
** failure: 0ms+0: the design is wrong
** error: 0ms+0: check failed
** fatal: cannot find unit WORK.TB_NOSUCH in library WORK
";

#[test]
fn nvc_failing_log_judges_fail() {
    // The probed nvc severity-failure log judges FAIL.
    assert!(!contracts::judge(&Tool::Nvc, NVC_FAILING_LOG));
}

#[test]
fn nvc_severity_error_log_judges_fail() {
    // A violated `severity error` check judges FAIL -- and, unlike the
    // ghdl hole, nvc's exit convicts this run too: the sim continued and
    // nvc still exited 1, probed live on nvc 1.23.0 (2026-11-07).
    assert!(!contracts::judge(&Tool::Nvc, NVC_SEVERITY_ERROR_LOG));
}

#[test]
fn nvc_analysis_error_log_judges_fail() {
    // The probed analysis-error face judges FAIL. Recorded for the
    // vocabulary's completeness; the analyze step gates on nvc's honest
    // exit and is never judged.
    assert!(!contracts::judge(&Tool::Nvc, NVC_ANALYSIS_ERROR_LOG));
}

#[test]
fn nvc_elaboration_failure_log_judges_fail() {
    // The probed `** Fatal:` elaboration refusal judges FAIL.
    assert!(!contracts::judge(&Tool::Nvc, NVC_ELAB_FAILURE_LOG));
}

#[test]
fn nvc_warning_and_note_stay_clean() {
    // The probed continuation faces (`** Warning:`, `** Note:`) judge
    // PASS: they are not failures, and a clean nvc run's own report notes
    // render as `** Note:` faces.
    assert!(contracts::judge(&Tool::Nvc, NVC_WARNING_NOTE_LOG));
}

#[test]
fn nvc_lowercase_drift_never_matches() {
    // A lowercase drift of every nvc face judges PASS: nvc capitalizes
    // its markers, case-exactly.
    assert!(contracts::judge(&Tool::Nvc, NVC_LOWERCASE_DRIFT_LOG));
}

#[test]
fn nvc_each_face_alone_judges_fail() {
    // Each of nvc's faces judges FAIL on its own, in an otherwise clean
    // log.
    for face in Tool::Nvc.vocabulary() {
        let log = format!("** Note: 0ms+0: testing started\nnoise {face} tail\n");
        assert!(
            !contracts::judge(&Tool::Nvc, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn nvc_empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Nvc, ""));
}

#[test]
fn nvc_vocabulary_is_the_three_probed_markers() {
    // nvc's vocabulary is the three probed severity markers, in probed
    // order.
    assert_eq!(Tool::Nvc.name(), "nvc");
    assert_eq!(
        Tool::Nvc.vocabulary(),
        &["** Failure:", "** Error:", "** Fatal:"]
    );
}

// ===== the iverilog vocabulary, probed live on Icarus Verilog 12.0
// (stable), 2026-11-07 =====

// a real vvp $fatal run (`iverilog -g2012 -o f.vvp tb_fatal.v && vvp
// f.vvp`): the run stops, exit 1. vvp leads the face with the uppercase
// `FATAL:` marker and indents the Time/Scope lines beneath it.
const IV_FATAL_LOG: &str = "\
FATAL: tb_fatal.v:4: the design is wrong
       Time: 1  Scope: tb_fatal
";

// a `$error` call (`iverilog -g2012 -o e.vvp tb_error.v && vvp e.vvp`):
// vvp prints the ERROR face, CONTINUES the simulation -- the testbench's
// own $display output and the `$finish called` line follow -- and STILL
// EXITS 0, exactly like bare ghdl's severity-error exit. The face
// convicts regardless of the exit; here the exit is no witness at all.
const IV_ERROR_LOG: &str = "\
ERROR: tb_error.v:4: check failed
       Time: 1  Scope: tb_error
simulation continued
tb_error.v:6: $finish called at 1 (1s)
";

// the compile faces, probed live (`iverilog -o snap_broken.vvp
// broken.v`, exit 2 on stderr; `iverilog -g2012 -s nosuch_top -o x.vvp
// tb.v`, exit 1). A run log never carries them -- the compile step gates
// on the exit -- but the faces are probed and recorded, per the nvc
// precedent.
const IV_COMPILE_ERROR_LOG: &str = "\
broken.v:2: syntax error
I give up.
";

// the unknown-`-s`-root refusal (`iverilog -g2012 -s nosuch_top -o
// x.vvp tb.v`, exit 1): the top names are validated at compile time.
const IV_UNKNOWN_ROOT_LOG: &str = "\
error: Unable to find the root module \"nosuch_top\" in the Verilog source.
     : Perhaps ``-s nosuch_top'' is incorrect?
1 error(s) during elaboration.
";

// the gentler neighbors, probed live on continuing runs that exit 0:
// `WARNING:` on a `$warning` call, and `$stop`'s interactive banner
// (exit 0, unlike a verilator model's $stop) -- continuations and
// legitimate stops, not failures, so they must never judge a log
const IV_WARNING_STOP_LOG: &str = "\
WARNING: tb_warn.v:4: check flagged
         Time: 1  Scope: tb_warn
all quiet
tb_warn.v:6: $finish called at 1 (1s)
stopping now
tb_stop.v:5: $stop called at 1 (1s)
** VVP Stop(0) **
** Flushing output streams.
** Current simulation time is 1 ticks.
> ** Continue **
";

// a clean run's log: the testbench's own $display output and the
// $finish line
const IV_CLEAN_LOG: &str = "\
sim done
tb.v:9: $finish called at 1 (1s)
";

// the probed failure logs with every face drifted to lowercase: not a
// single one of vvp's faces, since vvp spells its failure faces in
// uppercase
const IV_LOWERCASE_DRIFT_LOG: &str = "\
fatal: tb_fatal.v:4: the design is wrong
error: tb_error.v:4: check failed
";

#[test]
fn iv_fatal_log_judges_fail() {
    // The probed vvp $fatal log judges FAIL.
    assert!(!contracts::judge(&Tool::Iverilog, IV_FATAL_LOG));
}

#[test]
fn iv_error_log_judges_fail() {
    // A `$error` call judges FAIL -- and, like bare ghdl's severity
    // error, vvp continued the simulation and EXITED 0 (probed live on
    // vvp 12.0, 2026-11-07), so this run convicts on the face alone.
    // The exit is no witness here; the judge is the load-bearing gate.
    assert!(!contracts::judge(&Tool::Iverilog, IV_ERROR_LOG));
}

#[test]
fn iv_compile_faces_judge_fail() {
    // The probed compile faces judge FAIL. Recorded for the vocabulary's
    // completeness; the compile step gates on iverilog's honest exit
    // (exit 2 on a parse error, exit 1 on an unknown root) and is never
    // judged.
    assert!(!contracts::judge(&Tool::Iverilog, IV_COMPILE_ERROR_LOG));
    assert!(!contracts::judge(&Tool::Iverilog, IV_UNKNOWN_ROOT_LOG));
}

#[test]
fn iv_warning_and_stop_stay_clean() {
    // The probed continuation faces (`WARNING:`) and the `$stop` banner
    // judge PASS: they are not failures, and a $stop is a legitimate
    // stop (exit 0).
    assert!(contracts::judge(&Tool::Iverilog, IV_WARNING_STOP_LOG));
}

#[test]
fn iv_clean_log_judges_pass() {
    // A clean vvp log judges PASS.
    assert!(contracts::judge(&Tool::Iverilog, IV_CLEAN_LOG));
}

#[test]
fn iv_lowercase_drift_never_matches() {
    // A lowercase drift of every vvp failure face judges PASS: vvp
    // spells its failure faces in uppercase, case-exactly.
    assert!(contracts::judge(&Tool::Iverilog, IV_LOWERCASE_DRIFT_LOG));
}

#[test]
fn iv_each_face_alone_judges_fail() {
    // Each of iverilog's faces judges FAIL on its own, in an otherwise
    // clean log.
    for face in Tool::Iverilog.vocabulary() {
        let log = format!("tb.v:9: $finish called at 1 (1s)\nnoise {face} tail\n");
        assert!(
            !contracts::judge(&Tool::Iverilog, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn iv_empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Iverilog, ""));
}

#[test]
fn iv_vocabulary_is_the_four_probed_faces() {
    // iverilog's vocabulary is the four probed faces, in probed order:
    // the two runtime faces first (the judged faces), then the two
    // compile faces (recorded, never judged in practice).
    assert_eq!(Tool::Iverilog.name(), "iverilog");
    assert_eq!(
        Tool::Iverilog.vocabulary(),
        &[
            "FATAL:",
            "ERROR:",
            ": syntax error",
            "Unable to find the root module",
        ]
    );
}

// ===== the verilator vocabulary, probed live on Verilator 5.032,
// 2026-11-07 =====

// a real model $fatal run (`verilator --binary --build --timing
// --top-module tb_fatal ... && ./obj_dir/Vtb_fatal`): the model prints
// the %Fatal face, the %Error $stop line, and `Aborting...`, then dies
// by SIGABRT -- no exit code at all (the shell reports `Aborted`; the
// drive records -1). The exit convicts AND the face convicts.
const VER_FATAL_LOG: &str = "\
[1000] %Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is wrong
%Error: tb_fatal.sv:8: Verilog $stop
Aborting...
";

// the model's $error (`./obj_dir/Vtb_err`): the same %Error face and the
// same SIGABRT -- the built model, unlike vvp, has no interactive
// continue. Either witness convicts; here both do.
const VER_ERROR_LOG: &str = "\
[1000] %Error: tb_err.sv:8: Assertion failed in tb_err: check failed
%Error: tb_err.sv:8: Verilog $stop
Aborting...
";

// even a plain `$stop` aborts the model (`./obj_dir/Vtb_stop`): unlike
// vvp's interactive $stop (exit 0), the built model has no continue.
const VER_STOP_LOG: &str = "\
stopping now
%Error: tb_stop.sv:9: Verilog $stop
Aborting...
";

// the lint face (`verilator --lint-only broken.v`, exit 1, stderr): the
// %Error line above its caret block, closing with the Exiting line. A
// run log never carries it -- the lint step is never judged -- but the
// face is probed and recorded, per the nvc precedent.
const VER_LINT_ERROR_LOG: &str = "\
%Error: broken.v:2:3: syntax error, unexpected input, expecting ';'
    2 |   input a;
      |   ^~~~~
%Error: Exiting due to 1 error(s)
";

// the lint-time refusal of an unknown top (`verilator --lint-only
// --timing --top-module nosuch ...`, exit 1): top names are validated at
// lint time, so the build face carries the check for free.
const VER_UNKNOWN_TOP_LOG: &str = "\
%Error: Specified --top-module 'nosuch' was not found in design.
%Error: Exiting due to 1 error(s)
";

// the coded lint face (`verilator --lint-only` without --timing, exit
// 1): it shares no shape with `%Error:` -- the bare `%Error` face
// subsumes it.
const VER_CODED_ERROR_LOG: &str = "\
%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or --no-timing to specify how delays should be handled
                               : ... note: In instance 'tb'
    7 |     #1;
      |     ^
                      ... For error description see https://verilator.org/warn/NEEDTIMINGOPT?v=5.032
%Error: Exiting due to 1 error(s)
";

// the gentler neighbors, probed live on a continuing run that exits 0:
// the model's $warning face, the testbench's own $display, the $finish
// line, and the Simulation Report banner -- continuations, not
// failures, so they must never judge a log
const VER_WARNING_LOG: &str = "\
[1000] %Warning: tb_vwarn.sv:8: tb_vwarn: check flagged
all quiet
- tb_vwarn.sv:10: Verilog $finish
- S i m u l a t i o n   R e p o r t: Verilator 5.032 2025-01-01
- Verilator: $finish at 1ns; walltime 0.001 s; speed 674.149 ns/s
- Verilator: cpu 0.001 s on 1 threads; alloced 121 MB
";

// the probed failure logs with every face drifted to lowercase: not a
// single one of verilator's faces, since verilator capitalizes its
// markers
const VER_LOWERCASE_DRIFT_LOG: &str = "\
[1000] %fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is wrong
%error: tb_err.sv:8: Assertion failed in tb_err: check failed
%error: Specified --top-module 'nosuch' was not found in design.
";

#[test]
fn ver_fatal_log_judges_fail() {
    // The probed model $fatal log judges FAIL -- and the model's
    // SIGABRT death convicts the same run on the exit witness too.
    assert!(!contracts::judge(&Tool::Verilator, VER_FATAL_LOG));
}

#[test]
fn ver_error_and_stop_logs_judge_fail() {
    // The model's $error and $stop faces judge FAIL: the built model
    // aborts on both (SIGABRT), unlike vvp's continue-and-exit-0 $error
    // and interactive $stop.
    assert!(!contracts::judge(&Tool::Verilator, VER_ERROR_LOG));
    assert!(!contracts::judge(&Tool::Verilator, VER_STOP_LOG));
}

#[test]
fn ver_lint_and_top_faces_judge_fail() {
    // The probed lint faces judge FAIL. Recorded for the vocabulary's
    // completeness; the lint step gates on verilator's honest exit
    // (exit 1) and is never judged.
    assert!(!contracts::judge(&Tool::Verilator, VER_LINT_ERROR_LOG));
    assert!(!contracts::judge(&Tool::Verilator, VER_UNKNOWN_TOP_LOG));
}

#[test]
fn ver_coded_error_face_judges_fail() {
    // The coded lint face (`%Error-NEEDTIMINGOPT:`) judges FAIL through
    // the bare `%Error` face: it shares no shape with `%Error:`, which
    // is why the face carries no colon.
    assert!(!contracts::judge(&Tool::Verilator, VER_CODED_ERROR_LOG));
}

#[test]
fn ver_warning_stays_clean() {
    // The probed continuation faces (`%Warning:`, the report banner)
    // judge PASS: they are not failures.
    assert!(contracts::judge(&Tool::Verilator, VER_WARNING_LOG));
}

#[test]
fn ver_lowercase_drift_never_matches() {
    // A lowercase drift of every verilator face judges PASS: verilator
    // capitalizes its markers, case-exactly.
    assert!(contracts::judge(&Tool::Verilator, VER_LOWERCASE_DRIFT_LOG));
}

#[test]
fn ver_each_face_alone_judges_fail() {
    // Each of verilator's faces judges FAIL on its own, in an otherwise
    // clean log.
    for face in Tool::Verilator.vocabulary() {
        let log = format!("- Verilator: $finish at 1ns\nnoise {face} tail\n");
        assert!(
            !contracts::judge(&Tool::Verilator, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn ver_empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Verilator, ""));
}

#[test]
fn ver_vocabulary_is_the_two_probed_faces() {
    // verilator's vocabulary is the two probed faces: the bare `%Error`
    // (subsuming the coded lint faces) and the `%Fatal:` point-of-
    // failure face.
    assert_eq!(Tool::Verilator.name(), "verilator");
    assert_eq!(Tool::Verilator.vocabulary(), &["%Error", "%Fatal:"]);
}

// ===== the yosys vocabulary, probed live on Yosys 0.52, 2026-11-07 =====

// a real yosys syntax failure (`yosys -p "read_verilog broken.v; synth"`,
// exit 1, stderr): the ERROR marker rides mid-line behind the source
// location -- the one probed face that is not line-start
const YOS_SYNTAX_LOG: &str = "\
1. Executing Verilog-2005 frontend: broken.v
broken.v:3: ERROR: syntax error, unexpected TOK_ENDMODULE
";

// the script's refusal of an unknown top (`yosys -p "...; synth -top
// nosuch; ..."`, exit 1, stderr): line-start, backquoted name
const YOS_UNKNOWN_TOP_LOG: &str = "\
ERROR: Module `nosuch' not found!
";

// the unknown-`-p`-command face (`yosys -p "read_verilog gate.v;
// frobnicate"`, exit 1, stderr): line-start
const YOS_UNKNOWN_COMMAND_LOG: &str = "\
ERROR: No such command: frobnicate (type 'help' for a command overview)
";

// the failed-write face (`write_verilog` into a missing directory, exit
// 1, stderr): line-start
const YOS_FAILED_WRITE_LOG: &str = "\
ERROR: Can't open output file `netlists-missing/x.v' for writing: No such file or directory
";

// the missing-provider face (`synth` over an instantiation whose provider
// was never read, exit 1, stderr): line-start, hierarchical names
const YOS_NOT_PART_OF_DESIGN_LOG: &str = "\
ERROR: Module `\\gate' referenced in module `\\top' in cell `\\u_gate' is not part of the design.
";

// a clean synth's log tail (probed): the command echo and the `End of
// script` summary -- no ERROR marker anywhere
const YOS_CLEAN_LOG: &str = "\
-- Running command `read_verilog gate.v; synth -top gate; write_verilog netlists/gate.v' --

2.27. Executing OPT pass (performing simple optimizations).
End of script. Logfile hash: 8f1ba39a8d, CPU: user 0.02s system 0.01s, MEM: 17.06 MB peak
";

// the probed failure logs with the marker drifted to lowercase: not a
// single yosys face, since yosys spells its marker in uppercase. The
// gentler neighbor rides too (`Warning:` on a clean synth, probed)
const YOS_LOWERCASE_DRIFT_LOG: &str = "\
broken.v:3: error: syntax error, unexpected TOK_ENDMODULE
ERROR2: Module `nosuch' not found!
Warning: Re-writing memory cell netlist.
";

#[test]
fn yos_syntax_log_judges_fail() {
    // The probed syntax face judges FAIL: the ERROR marker is mid-line
    // behind the source location, and the substring search reads it
    // anyway -- the one probed yosys face that is not line-start.
    assert!(!contracts::judge(&Tool::Yosys, YOS_SYNTAX_LOG));
}

#[test]
fn yos_script_faces_judge_fail() {
    // The four probed script faces judge FAIL: the unknown top, the
    // unknown -p command, the failed write, the missing provider. All
    // line-start; all carry the same uppercase marker.
    assert!(!contracts::judge(&Tool::Yosys, YOS_UNKNOWN_TOP_LOG));
    assert!(!contracts::judge(&Tool::Yosys, YOS_UNKNOWN_COMMAND_LOG));
    assert!(!contracts::judge(&Tool::Yosys, YOS_FAILED_WRITE_LOG));
    assert!(!contracts::judge(&Tool::Yosys, YOS_NOT_PART_OF_DESIGN_LOG));
}

#[test]
fn yos_clean_log_judges_pass() {
    // A clean synth log judges PASS.
    assert!(contracts::judge(&Tool::Yosys, YOS_CLEAN_LOG));
}

#[test]
fn yos_lowercase_drift_never_matches() {
    // A lowercase drift of the syntax face judges PASS (yosys spells its
    // marker uppercase, case-exactly). The near-miss `ERROR2:` line
    // judges PASS too: the face is `ERROR:`, with the colon.
    assert!(contracts::judge(&Tool::Yosys, YOS_LOWERCASE_DRIFT_LOG));
}

#[test]
fn yos_each_face_alone_judges_fail() {
    // The one yosys face judges FAIL on its own, in an otherwise clean
    // log.
    for face in Tool::Yosys.vocabulary() {
        let log = format!("End of script. Logfile hash: 8f1ba39a8d\nnoise {face} tail\n");
        assert!(
            !contracts::judge(&Tool::Yosys, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn yos_empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Yosys, ""));
}

#[test]
fn yos_vocabulary_is_the_one_probed_face() {
    // yosys's vocabulary is the single probed `ERROR:` marker: it covers
    // every probed failure face, line-start and mid-line alike.
    assert_eq!(Tool::Yosys.name(), "yosys");
    assert_eq!(Tool::Yosys.vocabulary(), &["ERROR:"]);
}

// ===== the vivado vocabulary, probed live on Vivado 2025.2.1, 2026-11-07 =====

// a real xsim severity-failure run (`xsim tb_fail -R` after xvhdl/xelab):
// the face lands at line start and the run EXITS 0 -- the lie this
// vocabulary exists for
const VIV_FAILURE_LOG: &str = "\
source xsim.dir/tb_fail/xsim_script.tcl
# xsim {tb_fail} -autoloadwcfg -runall
Time resolution is 1 ps
run -all
Failure: the design is wrong
Time: 1 ns  Iteration: 0  Process: /tb_fail/stim  File: ./tb_fail.vhd
$finish called at time : 1 ns
exit
INFO: [Common 17-206] Exiting xsim at Sat Nov  7 18:07:29 2026...
";

// a real xsim severity-error run (`xsim tb_err -R`): the face, then the
// continuation note, then exit 0 -- the same lie
const VIV_ERROR_LOG: &str = "\
run -all
Error: check failed
Time: 1 ns  Iteration: 0  Process: /tb_err/stim  File: ./tb_err.vhd
Note: continued past the error
Time: 1 ns  Iteration: 0  Process: /tb_err/stim  File: ./tb_err.vhd
exit
";

// a clean run (`xsim tb_clean -R`): the severity-note face of a passing
// report and the kernel INFO lines -- continuations, never judged
const VIV_CLEAN_LOG: &str = "\
run -all
Note: sim done
Time: 1 ns  Iteration: 0  Process: /tb_clean/stim  File: ./tb_clean.vhd
exit
INFO: xsimkernel Simulation Memory Usage: 269612 KB (Peak: 335148 KB)
INFO: [Common 17-206] Exiting xsim at Sat Nov  7 18:07:16 2026...
";

// the probed compile and elaboration faces (xvhdl and xelab, exit 1 on
// stdout): UPPERCASE `ERROR:`, deliberately outside the vocabulary -- the
// compile steps gate on their honest exits and are never judged, and the
// case-exactness of the mixed-case markers holds this door
const VIV_COMPILE_ERROR_LOG: &str = "\
INFO: [VRFC 10-163] Analyzing VHDL file \"./broken.vhd\" into library work
ERROR: [VRFC 10-4982] syntax error near 'end' [./broken.vhd:9]
ERROR: [VRFC 10-2989] 'y' is not declared [./broken.vhd:8]
INFO: [VRFC 10-8704] VHDL file './broken.vhd' is ignored due to errors
";

// the probed order-sensitivity face (xvhdl, consumer before provider):
const VIV_ORDER_ERROR_LOG: &str = "\
ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work' [./tb_clean.vhd:11]
ERROR: [VRFC 10-9458] unit 'sim' is ignored due to previous errors [./tb_clean.vhd:7]
";

// the probed crash door (xsim on a missing snapshot, exit 1): an
// uppercase face the vocabulary does not carry -- the exit witness
// convicts
const VIV_MISSING_SNAPSHOT_LOG: &str = "\
ERROR: Please check the snapshot name which is created during 'xelab',the current snapshot name \"xsim.dir/nosuch_snap/xsimk\" does not exist
";

#[test]
fn viv_failure_log_judges_fail() {
    // The probed severity-failure log judges FAIL -- while xsim exits 0
    // on that very run (probed live), so the face is the only witness.
    assert!(!contracts::judge(&Tool::Vivado, VIV_FAILURE_LOG));
}

#[test]
fn viv_error_log_judges_fail() {
    // The probed severity-error log judges FAIL the same way: the run
    // continues and exits 0.
    assert!(!contracts::judge(&Tool::Vivado, VIV_ERROR_LOG));
}

#[test]
fn viv_clean_log_judges_pass() {
    // A clean xsim log judges PASS: the `Note:` face and the kernel INFO
    // lines are continuations, never judged.
    assert!(contracts::judge(&Tool::Vivado, VIV_CLEAN_LOG));
}

#[test]
fn viv_compile_faces_never_judge() {
    // The compile, order-sensitivity, elaboration, and crash-door faces
    // are UPPERCASE `ERROR:` -- not among the mixed-case markers, by
    // design: those stages gate on their honest exits.
    assert!(contracts::judge(&Tool::Vivado, VIV_COMPILE_ERROR_LOG));
    assert!(contracts::judge(&Tool::Vivado, VIV_ORDER_ERROR_LOG));
    assert!(contracts::judge(&Tool::Vivado, VIV_MISSING_SNAPSHOT_LOG));
}

#[test]
fn viv_faces_are_line_anchored() {
    // The line anchor, pinned both ways per marker: at line start the
    // face judges; mid-line -- a testbench's own report note, an echoed
    // source line -- it never judges, and neither does the case drift.
    for face in ["Fatal:", "Error:", "Failure:"] {
        let clean_midline = format!("the sim says: {face} retrying\n");
        assert!(
            contracts::judge(&Tool::Vivado, &clean_midline),
            "mid-line {face:?} judged a log"
        );
        let drifted = format!("{}\n{}", face.to_uppercase(), "  indented");
        assert!(
            contracts::judge(&Tool::Vivado, &drifted),
            "the case drift of {face:?} judged a log"
        );
        let clean_indented = format!("  {face} the design is wrong\n");
        assert!(
            contracts::judge(&Tool::Vivado, &clean_indented),
            "an indented {face:?} judged a log"
        );
        let failing = format!("Noise first\n{face} the design is wrong\n");
        assert!(
            !contracts::judge(&Tool::Vivado, &failing),
            "the line-start {face:?} face did not judge its log"
        );
    }
}

#[test]
fn viv_each_face_alone_judges_fail() {
    // Each of vivado's faces judges FAIL on its own, in an otherwise
    // clean log.
    for face in Tool::Vivado.vocabulary() {
        let log = format!("INFO: [Common 17-206] Exiting xsim\n{face} tail\n");
        assert!(
            !contracts::judge(&Tool::Vivado, &log),
            "face {face:?} did not judge its log"
        );
    }
}

#[test]
fn viv_empty_log_judges_pass() {
    // An empty log carries no failure face, so it judges PASS.
    assert!(contracts::judge(&Tool::Vivado, ""));
}

#[test]
fn viv_vocabulary_is_the_three_probed_markers() {
    // vivado's vocabulary is the three probed line-start markers, in
    // probed order.
    assert_eq!(Tool::Vivado.name(), "vivado");
    assert_eq!(Tool::Vivado.vocabulary(), &["Fatal:", "Error:", "Failure:"]);
}
