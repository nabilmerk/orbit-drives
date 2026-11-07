//! The probed ghdl vocabulary, ported from the aquila contract suite
//! (upstream: `github.com/chaseruskin/aquila`, `tests/test_contracts.py`):
//! the fixture logs are the live runs, byte-for-byte, and the judgments
//! are the contract. The retired Python gate counted `error):` and `ERROR`
//! substrings, matched nothing ghdl writes, and false-failed clean logs
//! that merely mentioned "ERROR"; these pins hold that door shut.

use std::str::FromStr;

use orbit_contracts::contracts::{self, Tool, UnknownTool};

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
    assert_eq!(contracts::tools(), &[Tool::Ghdl, Tool::Nvc]);
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
