//! The cocotb target drive: the orbit blueprint in, a judged verdict
//! out -- the test-only drive driven through COCOTB'S OWN runner.
//!
//! ONE orbit target shares this drive: `cocotb` (the test face). There
//! is no build face and no compile step of this drive's own -- the
//! runner IS the compile: cocotb's python runner
//! (`cocotb_tools.runner.get_runner("ghdl")`, probed live on cocotb
//! 2.1.0) drives the backing simulator itself (`ghdl -i` to analyze,
//! `ghdl -m` to elaborate, `ghdl -r --vpi=...` to run), so the drive
//! speaks the runner, never ghdl's argv.
//!
//! The probed runner face (cocotb 2.1.0, 2026-11-07): the runner is NOT
//! a module invocation (`python -m cocotb.runner` does not exist; the
//! module was retired with the 2.0 split) and not the 1.x Makefile flow
//! (no `TOPLEVEL` / `SIM` env vars -- 2.x renames them
//! `COCOTB_TOPLEVEL` / `COCOTB_TEST_MODULES`, set by the runner itself).
//! It is a python API: `runner.build(sources=..., hdl_toplevel=...,
//! build_dir=...)` then `runner.test(test_module=..., hdl_toplevel=...)`
//! returning the results.xml path. The exit contract, probed: the
//! SIMULATOR process failing exits the runner nonzero (`Simulation
//! failed: <rc>`), but a FAILING TEST does not -- `ghdl -r` exits 0 on a
//! failed cocotb test, the finding landing only in results.xml -- so the
//! drive's generated runner script reads the results itself
//! (`cocotb_tools.check_results.get_results`) and exits honest: nonzero
//! on any failed test, and on the missing-results face
//! (`ERROR: Simulation terminated abnormally. Results file ... not
//! found.`) a module that fails to import or carries no tests produces.
//!
//! The judged log is judged by COCOTB'S OWN vocabulary
//! ([`crate::contracts::COCOTB_VOCABULARY`], probed): the space-padded
//! ` FAIL ` row column of the runner's regression summary table and the
//! missing-results face. The backing simulator's own markers are NOT
//! this vocabulary's -- a cocotb log is a ghdl log, and ghdl's faces
//! belong to the ghdl drive; cocotb aggregates.
//!
//! The python test module rides the argv, not the blueprint:
//! `--test-module <path.py>` is REQUIRED (loud exit-2 absent). Its file
//! stem names the module the runner imports (`COCOTB_TEST_MODULES`) and
//! its parent directory rides the spawned python's `PYTHONPATH`, because
//! the runner passes its own `sys.path` down to the simulation.
//!
//! The fileset door: the ghdl backend compiles VHDL ONLY -- the runner
//! itself raises `ValueError: Ghdl only supports VHDL. ...` on any
//! non-VHDL source (probed in the runner's own `_build_command`), so the
//! drive carries the `VHDL` fileset alone and every other entry is the
//! loud not-carried face.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::blueprint::{parse_blueprint, plan_note, BlueprintError, Drive};
use crate::contracts::{self, Tool};
use crate::proc::{capture, join_log, lossy, print_block};

/// The python binary the drive spawns, resolved on PATH. The runner
/// resolves the backing simulator itself (`shutil.which("ghdl")`, its
/// own loud `ERROR: ghdl executable not found!` when absent).
pub const PYTHON_BIN: &str = "python3";

/// The runner build directory: cocotb's runner defaults to `sim_build`
/// and the drive pins the same name, wiped before every drive -- the
/// correctness-before-incrementality stance (the runner recreates it;
/// the stale `results.xml` inside it cannot survive anyway, since the
/// runner unlinks the results file before each test, probed in the
/// runner's source).
pub const SIM_BUILD_DIR: &str = "sim_build";

/// The driver script prefix: per top the drive writes
/// `orbit-cocotb-<top>.py`, the generated python that calls the runner
/// API (the vivado-generated-synth-script precedent), spawns it once,
/// and judges the log.
pub const DRIVER_PREFIX: &str = "orbit-cocotb";

/// The identity face: the python one-liner the drive spawns first, so a
/// missing cocotb (or the retired module layout of a 1.x install) is a
/// loud usage error instead of a driver-script failure. Probed: cocotb
/// 2.x ships the runner in the separate `cocotb_tools` distribution
/// package; a 1.x install answers this import with a nonzero exit.
pub const IDENTITY_PROBE: &str = "import cocotb, cocotb_tools.runner";

/// The record of one top's cocotb run: what the runner exited and what
/// it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top unit whose simulation ran.
    pub top: String,
    /// The generated runner script's exit code, recorded but secondary
    /// by ruling: it never flips the verdict alone. A `-1` records a run
    /// that died by signal rather than exiting. The script's exits are
    /// honest by construction -- it is this drive's own generated
    /// python, pinned to exit nonzero on any failed test or missing
    /// results file.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed cocotb failure face: the
    /// space-padded ` FAIL ` row column, or the missing-results face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Cocotb, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face
    /// in the log AND the runner script exited 0.
    ///
    /// Either witness convicts, and the exit here is the FIRST witness
    /// on test findings: the probed runner exit does not convict a
    /// failed test (`ghdl -r` exits 0), so the generated script's
    /// results.xml verdict is the load-bearing exit, and the face in the
    /// log is the second witness on the same failure.
    pub fn clean(&self) -> bool {
        !self.has_face() && self.exit == 0
    }
}

/// The error for a blueprint entry the drive does not carry: the ghdl
/// backend compiles VHDL only (the runner itself refuses everything
/// else, probed), so every non-VHDL entry -- `VLOG`, `SYSV`, a custom
/// fileset -- is rejected loudly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// Every non-VHDL entry, in blueprint order.
    pub entries: Vec<crate::blueprint::Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the cocotb drive carries the VHDL fileset only (the ghdl backend compiles \
             VHDL only, probed); not yet carried:"
        )?;
        for entry in &self.entries {
            write!(
                f,
                " {} (blueprint line {}, {})",
                entry.fileset,
                entry.line,
                entry.path.display()
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for NotCarried {}

/// How the drive turned out. Every variant is rendered on stderr by
/// [`drive`] before it is returned; the variant only picks the exit code.
#[derive(Debug)]
pub enum DriveError {
    /// The blueprint could not be read or parsed: usage-class, exit 2.
    Blueprint(BlueprintError),
    /// The blueprint carried filesets the drive does not speak:
    /// usage-class, exit 2.
    NotCarried(NotCarried),
    /// python (or the cocotb runner package) could not be spawned at
    /// all: usage-class, exit 2.
    Spawn(String),
    /// The stale build directory or driver script could not be reset:
    /// usage-class, exit 2.
    Library(String),
    /// No test module was named: usage-class, exit 2 (the CLI gate
    /// fires first; the drive carries the same door for direct drives).
    TestModule,
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings the judge or the generated
    /// runner script reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_)
            | DriveError::TestModule => 2,
            DriveError::Findings(_) => 1,
        }
    }
}

impl fmt::Display for DriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The drive renders every variant itself before returning it; this
        // Display exists so the error type is a std::error::Error and can
        // travel, and carries the same words the drive prints.
        match self {
            DriveError::Blueprint(e) => write!(f, "{e}"),
            DriveError::NotCarried(e) => write!(f, "{e}"),
            DriveError::Spawn(message) => write!(f, "{message}"),
            DriveError::Library(message) => write!(f, "{message}"),
            DriveError::TestModule => write!(f, "no test module to drive"),
            DriveError::Findings(findings) => {
                write!(f, "{} run(s) carried a finding", findings.len())
            }
        }
    }
}

impl std::error::Error for DriveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DriveError::Blueprint(e) => Some(e),
            DriveError::NotCarried(e) => Some(e),
            _ => None,
        }
    }
}

/// Escape a path into a python double-quoted string literal for the
/// generated driver: the driver is machine-written python, so the file
/// paths ride it EXACTLY (backslash and quote escaped), and the
/// whitespace-split v1 limit the other script faces document does not
/// exist here.
fn py_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The module import name and the directory that must ride PYTHONPATH
/// for the runner to import it: the file stem of `--test-module`, and
/// its parent directory (`.` for a bare filename).
pub fn module_name_and_dir(test_module: &str) -> (String, PathBuf) {
    let path = Path::new(test_module);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => PathBuf::from("."),
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::from("."),
    };
    (stem, dir)
}

/// Generate the one runner script for a top: `get_runner("ghdl")`, the
/// build (every blueprint file, the top, the pinned build dir, always
/// rebuilt), the test (the module stem, the top, the extras riding
/// `test_args` -- the runner's documented extra-arguments channel to the
/// backing simulator, the last step of the drive), and the results read
/// whose verdict is the script's own honest exit -- the load-bearing
/// exit witness, probed into existence by the runner's own dishonesty
/// about failed tests (`ghdl -r` exits 0 on a failed test; the finding
/// lands only in results.xml).
pub fn runner_script(top: &str, module_stem: &str, files: &[PathBuf], extra: &[String]) -> String {
    let mut script = String::new();
    script.push_str("import sys\n");
    script.push_str("from pathlib import Path\n");
    script.push('\n');
    script.push_str("from cocotb_tools.check_results import get_results\n");
    script.push_str("from cocotb_tools.runner import get_runner\n");
    script.push('\n');
    script.push_str("runner = get_runner(\"ghdl\")\n");
    script.push_str("runner.build(\n");
    script.push_str("    sources=[");
    for (index, file) in files.iter().enumerate() {
        if index > 0 {
            script.push_str(", ");
        }
        script.push_str(&py_string(&file.to_string_lossy()));
    }
    script.push_str("],\n");
    script.push_str("    hdl_toplevel=");
    script.push_str(&py_string(top));
    script.push_str(",\n");
    script.push_str("    build_dir=");
    script.push_str(&py_string(SIM_BUILD_DIR));
    script.push_str(",\n");
    script.push_str("    always=True,\n");
    script.push_str(")\n");
    script.push_str("results = runner.test(\n");
    script.push_str("    test_module=");
    script.push_str(&py_string(module_stem));
    script.push_str(",\n");
    script.push_str("    hdl_toplevel=");
    script.push_str(&py_string(top));
    script.push_str(",\n");
    script.push_str("    build_dir=");
    script.push_str(&py_string(SIM_BUILD_DIR));
    script.push_str(",\n");
    script.push_str("    test_args=[");
    for (index, arg) in extra.iter().enumerate() {
        if index > 0 {
            script.push_str(", ");
        }
        script.push_str(&py_string(arg));
    }
    script.push_str("],\n");
    script.push_str(")\n");
    script.push_str("try:\n");
    script.push_str("    (num_tests, num_failed) = get_results(Path(results))\n");
    script.push_str("except RuntimeError as e:\n");
    script.push_str("    print(e.args[0], file=sys.stderr)\n");
    script.push_str("    sys.exit(1)\n");
    script.push_str("print(f\"cocotb summary: {num_tests} test(s), {num_failed} failed\")\n");
    script.push_str("sys.exit(0 if num_failed == 0 else 1)\n");
    script
}

/// Drive the cocotb target end to end: blueprint in, per-top judged runs
/// out, through cocotb's own runner.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths, the
/// generated runner scripts, and `sim_build/` land there. All progress
/// and diagnostics are printed here; the returned error only picks the
/// exit code. There is no build face: every run builds, tests, and
/// judges, because the runner IS the compile.
pub fn drive(request: &Drive) -> Result<(), DriveError> {
    // python + the cocotb runner must be spawnable before anything else,
    // so a missing cocotb is a loud usage error instead of a
    // driver-script failure (a 1.x install answers the same probe with
    // a nonzero exit: the runner lives in cocotb_tools since 2.0).
    let mut probe = Command::new(PYTHON_BIN);
    probe.arg("-c").arg(IDENTITY_PROBE);
    capture(PYTHON_BIN, &mut probe).map_err(DriveError::Spawn)?;

    // The test module is the drive's own requirement: the CLI gate
    // fires first (exit 2 naming the flag); this door keeps the drive
    // honest for direct library callers.
    let Some(test_module) = request.test_module.as_deref().filter(|m| !m.is_empty()) else {
        let e = DriveError::TestModule;
        eprintln!("orbit-drives: cocotb: error: {e}: pass --test-module <path.py>");
        return Err(e);
    };
    let (module_stem, module_dir) = module_name_and_dir(test_module);

    // Run-scoped build state: see `SIM_BUILD_DIR`. Every drive starts
    // from an empty build directory (probed honestly: the wipe is
    // symmetry -- the runner unlinks the results file before each test
    // and always=True rebuilds).
    if let Err(e) = fs::remove_dir_all(SIM_BUILD_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale build directory {SIM_BUILD_DIR}/: {e}");
            eprintln!("orbit-drives: cocotb: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    if let Some(note) = plan_note(
        "cocotb",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: cocotb: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: cocotb: error: {e}");
        DriveError::Blueprint(e)
    })?;
    // The fileset door: the ghdl backend compiles VHDL only (the runner
    // refuses everything else, probed), so every non-VHDL entry is the
    // loud not-carried face, in blueprint order.
    let foreign: Vec<crate::blueprint::Entry> = entries
        .iter()
        .filter(|entry| entry.fileset != "VHDL")
        .cloned()
        .collect();
    if !foreign.is_empty() {
        let e = NotCarried { entries: foreign };
        eprintln!("orbit-drives: cocotb: error: {e}");
        return Err(DriveError::NotCarried(e));
    }
    let files: Vec<PathBuf> = entries.iter().map(|entry| entry.path.clone()).collect();

    println!(
        "orbit-drives: cocotb: test: carrying {} VHDL file(s) into the cocotb runner (the \
         runner IS the compile; test module {module_stem} from {})",
        files.len(),
        module_dir.display()
    );

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-drives: cocotb: test: driving {top} through the cocotb runner");
        let script = runner_script(top, &module_stem, &files, &request.extra);
        let script_path = format!("{DRIVER_PREFIX}-{top}.py");
        fs::write(&script_path, &script).map_err(|e| {
            let message = format!("cannot write the runner script {script_path}: {e}");
            eprintln!("orbit-drives: cocotb: error: {message}");
            DriveError::Library(message)
        })?;
        // The generated script spawns with the test module's directory
        // APPENDED to PYTHONPATH: the runner passes its own sys.path
        // down to the simulation, and the module must be importable
        // from there (probed shape). The extras ride the script's
        // `test_args` (the runner's documented channel to the backing
        // simulator's argv), never dropped.
        let mut run = Command::new(PYTHON_BIN);
        run.arg(&script_path);
        if let Ok(existing) = std::env::var("PYTHONPATH") {
            if !existing.is_empty() {
                run.env("PYTHONPATH", format!("{existing}:{}", module_dir.display()));
            } else {
                run.env("PYTHONPATH", module_dir.as_os_str());
            }
        } else {
            run.env("PYTHONPATH", module_dir.as_os_str());
        }
        let output = capture(PYTHON_BIN, &mut run).map_err(DriveError::Spawn)?;
        let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
        print_block(&log);
        let outcome = RunOutcome {
            top: top.to_string(),
            exit: output.status.code().unwrap_or(-1),
            log,
        };
        if outcome.clean() {
            println!("orbit-drives: cocotb: test: {top}: pass");
        } else if outcome.has_face() {
            println!(
                "orbit-drives: cocotb: test: {top}: fail (the log carries a probed cocotb \
                 failure face)"
            );
            findings.push(outcome);
        } else {
            println!(
                "orbit-drives: cocotb: test: {top}: fail (the runner exited {} without a \
                 judged face)",
                outcome.exit
            );
            findings.push(outcome);
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "orbit-drives: cocotb: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::MalformedLine;

    // The probed clean pass (cocotb 2.1.0 + ghdl 6.0.0, 2026-11-07,
    // /tmp/cocotb-probe): the runner's build lines, the regression
    // table with the PASS row, the clean summary, and the generated
    // script's own summary line. Byte-for-byte as recorded live.
    const CLEAN_LOG: &str = "\
analyze /tmp/cocotb-probe/counter.vhd
elaborate counter
     6.00ns INFO     cocotb.regression                  test_counter.reads_the_first_count passed
     6.00ns INFO     cocotb.regression                  ********************************************************************************************
                                                        ** TEST                                STATUS  SIM TIME (ns)  REAL TIME (s)  RATIO (ns/s) **
                                                        ********************************************************************************************
                                                        ** test_counter.reads_the_first_count   PASS           6.00           0.00       3747.55  **
                                                        ********************************************************************************************
                                                        ** TESTS=1 PASS=1 FAIL=0 SKIP=0                        6.00           0.00       2748.09  **
                                                        ********************************************************************************************
cocotb summary: 1 test(s), 0 failed
";

    // The probed failing run: the AssertionError traceback rides the
    // log, the row column reads FAIL, and the summary counts the
    // failure -- `FAIL=1` shares no shape with the judged face, the
    // space-padded row column does.
    const FAILING_LOG: &str = "\
     6.00ns WARNING  cocotb.regression                  test_counter_fail.expects_the_wrong_count failed
                                                        AssertionError: the design is wrong
                                                        assert 3 == 99
     6.00ns INFO     cocotb.regression                  ***************************************************************************************************
                                                        ** TEST                                       STATUS  SIM TIME (ns)  REAL TIME (s)  RATIO (ns/s) **
                                                        ***************************************************************************************************
                                                        ** test_counter_fail.expects_the_wrong_count   FAIL           6.00           0.00       3794.54  **
                                                        ***************************************************************************************************
                                                        ** TESTS=1 PASS=0 FAIL=1 SKIP=0                               6.00           0.00       1483.59  **
                                                        ***************************************************************************************************
cocotb summary: 1 test(s), 1 failed
";

    // The probed missing-results face (a test module that fails to
    // import, or carries no tests): the runner script relays
    // get_results' RuntimeError message and exits 1.
    const MISSING_RESULTS_LOG: &str = "\
Traceback (most recent call last):
  File \"<frozen importlib._bootstrap>\", line 241, in _find_and_load_unlocked
ModuleNotFoundError: No module named 'nonexistent_module_xyz'
ERROR: Simulation terminated abnormally. Results file /tmp/x/sim_build/results.xml not found.
";

    // The probed near-miss: the clean summary's own failure counter.
    // `FAIL=0` shares no shape with ` FAIL ` -- the space padding is
    // load-bearing, this line alone never judges.
    const SUMMARY_ONLY_LOG: &str = "\
** TESTS=3 PASS=3 FAIL=0 SKIP=0
";

    #[test]
    fn a_clean_run_passes_the_gate() {
        // The probed clean pass: exit 0, the summary counts zero
        // failures, no face.
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 0,
            log: CLEAN_LOG.to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn a_failing_test_convicts_on_both_witnesses() {
        // The probed failing run: the row column carries ` FAIL ` AND
        // the generated script exited 1 (the load-bearing exit -- the
        // probed runner exit lies, `ghdl -r` exits 0 on a failed test).
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 1,
            log: FAILING_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_face_convicts_an_exit_of_zero() {
        // The judge's reason to exist as the second witness: a runner
        // update that printed the face but exited 0 must not pass.
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 0,
            log: FAILING_LOG.to_string(),
        };
        assert_eq!(outcome.exit, 0);
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn the_missing_results_face_judges_fail() {
        // The probed missing-results face (import failure, no-tests
        // module, crashed sim): the relayed ERROR line judges, and the
        // generated script exits 1 behind it.
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 1,
            log: MISSING_RESULTS_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn the_summary_counter_never_judges_a_clean_run() {
        // The load-bearing padding: `FAIL=0` (and any `FAIL=` counter)
        // shares no shape with the space-padded ` FAIL ` row face, so
        // the summary line alone never judges -- pinned on the probed
        // clean summary.
        assert!(contracts::judge(&Tool::Cocotb, SUMMARY_ONLY_LOG));
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 0,
            log: SUMMARY_ONLY_LOG.to_string(),
        };
        assert!(!outcome.has_face());
        assert!(outcome.clean());
    }

    #[test]
    fn a_lowercase_drift_never_judges() {
        // cocotb's row column is uppercase FAIL, case-exactly: a
        // lowercase drift is not one of its faces and never judges a
        // log.
        assert!(contracts::judge(
            &Tool::Cocotb,
            "** tb.reads_the_first_count    fail    6.00\n"
        ));
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a killed runner can exit nonzero without
        // writing any probed face, and a clean log must not green-light
        // it. Either witness convicts.
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: 137,
            log: CLEAN_LOG.to_string(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_signal_death_is_a_finding_on_its_minus_one_exit() {
        // A run killed by a signal has no exit code; the drive records
        // -1 (see `RunOutcome.exit`), and -1 convicts.
        let outcome = RunOutcome {
            top: "counter".to_string(),
            exit: -1,
            log: String::new(),
        };
        assert!(!outcome.clean());
    }

    #[test]
    fn each_face_alone_judges_fail() {
        // Each of cocotb's faces judges FAIL on its own, in an
        // otherwise clean log.
        for face in Tool::Cocotb.vocabulary() {
            let log = format!("cocotb summary: 1 test(s), 0 failed\nnoise {face} tail\n");
            assert!(
                !contracts::judge(&Tool::Cocotb, &log),
                "face {face:?} did not judge its log"
            );
        }
    }

    #[test]
    fn the_runner_script_reads_the_probed_shape() {
        // The generated script shape, probed: get_runner("ghdl"), the
        // build (every file, the top, sim_build, always=True), the test
        // (module stem, top, the extras on test_args), the results
        // read, and the honest exit.
        let script = runner_script(
            "counter",
            "test_counter",
            &[PathBuf::from("./counter.vhd"), PathBuf::from("./top.vhd")],
            &["--vcd=w.vcd".to_string()],
        );
        assert!(script.contains("from cocotb_tools.runner import get_runner\n"));
        assert!(script.contains("runner = get_runner(\"ghdl\")\n"));
        assert!(script.contains("    sources=[\"./counter.vhd\", \"./top.vhd\"],\n"));
        assert!(script.contains("    hdl_toplevel=\"counter\",\n"));
        assert!(script.contains("    build_dir=\"sim_build\",\n"));
        assert!(script.contains("    always=True,\n"));
        assert!(script.contains("    test_module=\"test_counter\",\n"));
        assert!(script.contains("    test_args=[\"--vcd=w.vcd\"],\n"));
        assert!(script.contains("    (num_tests, num_failed) = get_results(Path(results))\n"));
        assert!(script.ends_with("sys.exit(0 if num_failed == 0 else 1)\n"));
    }

    #[test]
    fn the_runner_script_escapes_the_paths_exactly() {
        // The driver is machine-written python, so the paths ride it
        // EXACTLY: a quote and a backslash are escaped, no
        // whitespace-split limit on this face.
        let script = runner_script(
            "top",
            "tb",
            &[PathBuf::from("./my design/gate\"1.vhd")],
            &[],
        );
        assert!(script.contains("    sources=[\"./my design/gate\\\"1.vhd\"],\n"));
        assert!(script.contains("    test_args=[],\n"));
    }

    #[test]
    fn the_module_resolves_to_stem_and_dir() {
        // The import name is the file stem; the directory rides
        // PYTHONPATH. A bare filename's parent is the current
        // directory.
        assert_eq!(
            module_name_and_dir("/tmp/probe/test_counter.py"),
            ("test_counter".to_string(), PathBuf::from("/tmp/probe"))
        );
        assert_eq!(
            module_name_and_dir("tb_counter.py"),
            ("tb_counter".to_string(), PathBuf::from("."))
        );
    }

    #[test]
    fn drive_errors_map_to_honest_exit_codes() {
        assert_eq!(
            DriveError::Blueprint(BlueprintError::Read {
                path: PathBuf::from("nope"),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "nope"),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Blueprint(BlueprintError::Parse(MalformedLine {
                line: 1,
                text: "VHDL\twork".to_string(),
            }))
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::NotCarried(NotCarried {
                entries: Vec::new(),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Spawn("python3: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(DriveError::TestModule.exit_code(), 2);
        assert_eq!(DriveError::Findings(Vec::new()).exit_code(), 1);
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::TestModule;
        assert_eq!(e.to_string(), "no test module to drive");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
        let e = DriveError::NotCarried(NotCarried {
            entries: Vec::new(),
        });
        assert!(e
            .to_string()
            .starts_with("the cocotb drive carries the VHDL fileset only"));
    }

    #[test]
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("cocotb", Some("tsv")).is_none());
        assert!(plan_note("cocotb", None).is_none());
        let note = plan_note("cocotb", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: cocotb: note:"));
    }
}
