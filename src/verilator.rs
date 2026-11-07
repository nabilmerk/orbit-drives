//! The verilator target drive: the orbit blueprint in, a judged verdict
//! out.
//!
//! Two orbit targets share this drive, split by [`Mode`]:
//! `verilator-build` (lint + the per-top model builds, exit honest, no
//! simulation, nothing judged) and `verilator` (the same lint and builds,
//! then the judged model runs). The drive reads the blueprint orbit wrote
//! for the target (the TSV plan: one `fileset<TAB>library<TAB>filepath`
//! line per collected file), lints the Verilog entries once, builds one
//! simulation model per requested top (`verilator --binary --build
//! --timing --top-module <top>`, landing `obj_dir/V<top>`), and -- on the
//! sim face -- runs each model and judges the run log against verilator's
//! probed failure vocabulary.
//!
//! There is NO deferral loop here, on probed grounds: verilator reads all
//! files into one elaboration and is order-free (a consumer-before-
//! provider pair linted and built clean, probed live on Verilator 5.032,
//! 2026-11-07), so there is nothing to defer -- one lint, then per-top
//! builds. The models are per-top because each testbench is its own
//! simulation; the model binaries coexist in the one `obj_dir`
//! (`Vtb` and `Vtb_err` probed side by side).
//!
//! The judge gates the sim face as second witness, not first: every model
//! failure probed dies by SIGABRT -- no exit code at all, the exit
//! witness always convicts -- and the `%Error:`/`%Fatal:` faces in the
//! log convict regardless. Either witness convicts, everywhere. The lint
//! and build faces need no judge: verilator exits nonzero on a broken
//! design (exit 1 on a lint error, on an unknown `--top-module`, and on
//! an enabled warning -- probed live), so the exits are honest there.

use std::fmt;
use std::fs;
use std::process::Command;

use crate::blueprint::{parse_blueprint, plan_note, verilog_only, BlueprintError, Drive, Mode};
use crate::contracts::{self, Tool};
use crate::iverilog::NotCarried;
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

/// The verilator binary the drive spawns, resolved on PATH. The orbit
/// target contract carries no binary-path field.
pub const VERILATOR_BIN: &str = "verilator";

/// The timing option pinned for every verilator invocation, probed live
/// on Verilator 5.032 (2026-11-07): without it, a delay-carrying file
/// fails the lint with `%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or
/// --no-timing to specify how delays should be handled` (exit 1). It
/// rides after `--lint-only` and before the files, and again on every
/// build argv.
pub const TIMING: &str = "--timing";

/// The model directory verilator builds into (`--Mdir`'s default,
/// probed). It is verilator's clock: the per-top model binaries
/// (`obj_dir/V<top>`) and their intermediates land there, and the
/// binaries of different tops coexist (probed side by side).
///
/// Orbit's output directory persists across runs, and v1's stance is
/// correctness before incrementality: every drive starts from an empty
/// model directory. Probed honestly: the wipe is symmetry here, not
/// load-bearing -- `--build` regenerates the intermediates, and a failed
/// build exits 1 before the sim face could run a stale model; it exists
/// so every drive starts from a known-clean clock.
pub const OBJ_DIR: &str = "obj_dir";

/// The record of one model's run: what it exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top module whose model ran.
    pub top: String,
    /// The model's exit code, recorded but secondary: it never flips the
    /// verdict. A `-1` records a run that died by signal rather than
    /// exiting -- the probed shape: the model aborts (SIGABRT) on every
    /// `$fatal`/`$error`/`$stop` failure, so the shell sees `Aborted`
    /// and a raw spawn sees no code; -1 convicts either way.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed verilator failure face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Verilator, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face
    /// in the log AND the model exited 0.
    ///
    /// Either witness convicts. For the built model the exit has probed
    /// merciless -- every `$fatal`/`$error`/`$stop` aborts the process
    /// (SIGABRT, no exit code) -- so the exit convicts on every failure
    /// seen so far, and the face is the second witness; the judge stays
    /// the gate because the exit cannot name the face, and a tool update
    /// that stops printing what it exits for must not silently pass.
    pub fn clean(&self) -> bool {
        !self.has_face() && self.exit == 0
    }
}

/// How the drive turned out. Every variant is rendered on stderr by
/// [`drive`] before it is returned; the variant only picks the exit code.
#[derive(Debug)]
pub enum DriveError {
    /// The blueprint could not be read or parsed: usage-class, exit 2.
    Blueprint(BlueprintError),
    /// The blueprint carried filesets the drive does not speak yet:
    /// usage-class, exit 2.
    NotCarried(NotCarried),
    /// verilator could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale model directory could not be wiped: usage-class, exit 2.
    Library(String),
    /// The lint failed: findings, exit 1 (verilator's lint exit is
    /// honest: exit 1 on a lint error, on an unknown `--top-module`, and
    /// on an enabled warning, probed live).
    Lint {
        /// The stderr of the failed lint, verbatim.
        stderr: String,
    },
    /// A model build failed: findings, exit 1.
    Build {
        /// The top module verilator refused to build.
        top: String,
        /// The stderr of the failed build, verbatim.
        stderr: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings verilator or the judge
    /// reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_) => 2,
            DriveError::Lint { .. } | DriveError::Build { .. } | DriveError::Findings(_) => 1,
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
            DriveError::Lint { .. } => write!(f, "the verilator lint failed"),
            DriveError::Build { top, .. } => write!(f, "the verilator build failed for {top}"),
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

/// Drive the verilator target end to end: blueprint, one lint, per-top
/// model builds, and -- on the sim face -- judged model runs.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and
/// verilator's `obj_dir` land there. All progress and diagnostics are
/// printed here; the returned error only picks the exit code. The mode
/// picks how far the drive goes: `Build` stops after the model builds (no
/// run, no log, nothing judged); `Sim` runs each built model and judges
/// it.
///
/// There is no deferral loop: verilator's lint and build are order-free
/// (probed live on Verilator 5.032, 2026-11-07). The lint runs ONCE over
/// all files with every requested top a `--top-module` (repeated flags
/// probed legal, but verilator honors only the LAST one -- probed: an
/// unknown non-last top lints clean, exit 0); each top then gets its own
/// model build, which validates it (an unknown top fails its build with
/// exit 1), and -- on the sim face -- its own judged run.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    // verilator must be spawnable before anything else, so a missing
    // binary is a loud usage error instead of a silent lint failure.
    capture(VERILATOR_BIN, Command::new(VERILATOR_BIN).arg("--version"))
        .map_err(DriveError::Spawn)?;

    // Run-scoped state: see `OBJ_DIR`. Every drive starts from an empty
    // model directory (probed honestly: the wipe is symmetry here -- a
    // failed build exits before the run could touch a stale model).
    if let Err(e) = fs::remove_dir_all(OBJ_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale model directory {OBJ_DIR}/: {e}");
            eprintln!("orbit-drives: verilator: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    if let Some(note) = plan_note(
        "verilator",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: verilator: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: verilator: error: {e}");
        DriveError::Blueprint(e)
    })?;
    // The fileset door is the Verilog drives' shared one: iverilog and
    // verilator carry the same VLOG and SYSV filesets -- the filter lives
    // with the blueprint machinery, its message with the first Verilog
    // drive.
    let files = verilog_only(&entries).map_err(|e| {
        eprintln!("orbit-drives: verilator: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-drives: verilator: {label}: linting {} Verilog file(s) with --timing \
         (order-free, probed live 2026-11-07)",
        files.len()
    );
    lint_once(&files, &request.tops).map_err(|stderr| {
        eprint_block(&stderr);
        eprintln!("orbit-drives: verilator: error: the verilator lint failed");
        DriveError::Lint { stderr }
    })?;
    println!("orbit-drives: verilator: {label}: lint clean");

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-drives: verilator: {label}: building {top} into {OBJ_DIR}");
        // The extras ride the drive's last step verbatim: the model run
        // argv on the sim face, the build argv on the build face (there
        // is no run to carry them, and silently dropping a user argument
        // would be a degradation). An option the build does not speak is
        // verilator's own loud rejection, surfaced as an exit-1 build
        // failure.
        let build_extra = if mode == Mode::Build {
            Some(request.extra.as_slice())
        } else {
            None
        };
        build_top(top, &files, build_extra).map_err(|stderr| {
            eprint_block(&stderr);
            eprintln!("orbit-drives: verilator: error: the verilator build failed for {top}");
            DriveError::Build {
                top: top.clone(),
                stderr,
            }
        })?;

        match mode {
            Mode::Build => println!("orbit-drives: verilator: {label}: {top}: built"),
            Mode::Sim => {
                let outcome = run_model(top, &request.extra, label)?;
                if outcome.clean() {
                    println!("orbit-drives: verilator: {label}: {top}: pass");
                } else if outcome.has_face() {
                    println!(
                        "orbit-drives: verilator: {label}: {top}: fail (the log carries a \
                         probed verilator failure face)"
                    );
                    findings.push(outcome);
                } else {
                    println!(
                        "orbit-drives: verilator: {label}: {top}: fail (the model exited {} \
                         without a judged face)",
                        outcome.exit
                    );
                    findings.push(outcome);
                }
            }
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "orbit-drives: verilator: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// The one lint: `verilator --lint-only --timing --top-module <top>...
/// <files>`, every requested top a root. Verilator honors only the LAST
/// `--top-module` (probed: an unknown non-last top lints clean, exit 0);
/// every top is validated by its own per-top build on both faces
/// (`%Error: Specified --top-module 'nosuch' was not found in design.`,
/// exit 1, probed at the build), and the lint and build exits are
/// honest, so both faces gate on the exit alone.
fn lint_once(files: &[std::path::PathBuf], tops: &[String]) -> Result<(), String> {
    let mut lint = Command::new(VERILATOR_BIN);
    lint.arg("--lint-only").arg(TIMING);
    for top in tops {
        lint.arg("--top-module").arg(top);
    }
    lint.args(files);
    let output = capture(VERILATOR_BIN, &mut lint)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(lossy(&output.stderr))
    }
}

/// One model build: `verilator --binary --build --timing --top-module
/// <top> <files> <extras>`. The model lands at `obj_dir/V<top>` (probed);
/// the models of different tops coexist in the one directory.
fn build_top(
    top: &str,
    files: &[std::path::PathBuf],
    extra: Option<&[String]>,
) -> Result<(), String> {
    let mut build = Command::new(VERILATOR_BIN);
    build
        .arg("--binary")
        .arg("--build")
        .arg(TIMING)
        .arg("--top-module")
        .arg(top)
        .args(files);
    if let Some(extra) = extra {
        build.args(extra);
    }
    let output = capture(VERILATOR_BIN, &mut build)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(lossy(&output.stderr))
    }
}

/// Run one built model and capture its log: `./obj_dir/V<top> <extras>`.
/// The probed failure faces (`%Fatal:`, `%Error:`) land on stdout; the
/// log joins both streams, so it reads either way. A model killed by a
/// signal -- the probed SIGABRT on every `$fatal`/`$error`/`$stop` --
/// has no exit code, and [`RunOutcome::exit`] records -1.
fn run_model(top: &str, extra: &[String], label: &str) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: verilator: {label}: running {top}");
    let model = format!("./{OBJ_DIR}/V{top}");
    let mut run = Command::new(&model);
    run.args(extra);
    let output = capture(&model, &mut run).map_err(DriveError::Spawn)?;
    let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
    print_block(&log);
    Ok(RunOutcome {
        top: top.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::MalformedLine;

    // The probed $fatal fixture (verilator 5.032, 2026-11-07): the model
    // prints the %Fatal face, the %Error $stop line, and `Aborting...`,
    // then dies by SIGABRT -- no exit code, recorded as -1. Byte-for-byte
    // as recorded live; the faces land on stdout.
    const FATAL_LOG: &str = "\
[1000] %Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is wrong
%Error: tb_fatal.sv:8: Verilog $stop
Aborting...
";

    // The probed $error fixture (verilator 5.032, 2026-11-07): the same
    // %Error face and the same SIGABRT -- the built model, unlike vvp,
    // has no interactive continue.
    const ERROR_LOG: &str = "\
[1000] %Error: tb_err.sv:8: Assertion failed in tb_err: check failed
%Error: tb_err.sv:8: Verilog $stop
Aborting...
";

    // The probed lint face (verilator --lint-only, exit 1): the %Error
    // line above its caret block, closing with the Exiting line.
    const LINT_ERROR_LOG: &str = "\
%Error: broken.v:2:3: syntax error, unexpected input, expecting ';'
    2 |   input a;
      |   ^~~~~
%Error: Exiting due to 1 error(s)
";

    // The probed lint-time refusal of an unknown top (exit 1).
    const UNKNOWN_TOP_LOG: &str = "\
%Error: Specified --top-module 'nosuch' was not found in design.
%Error: Exiting due to 1 error(s)
";

    // The probed gentle faces on a continuing run that exits 0: the
    // model's $warning face, the testbench's own $display, the $finish
    // line, and the Simulation Report banner -- continuations, not
    // failures.
    const WARNING_LOG: &str = "\
[1000] %Warning: tb_vwarn.sv:8: tb_vwarn: check flagged
all quiet
- tb_vwarn.sv:10: Verilog $finish
- S i m u l a t i o n   R e p o r t: Verilator 5.032 2025-01-01
- Verilator: $finish at 1ns; walltime 0.001 s; speed 674.149 ns/s
";

    #[test]
    fn a_fatal_run_is_a_finding_on_both_witnesses() {
        // The probed stop: the face AND the SIGABRT death (-1). Either
        // witness convicts; here both do.
        let outcome = RunOutcome {
            top: "tb_fatal".to_string(),
            exit: -1,
            log: FATAL_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_fatal_run_convicts_on_its_signal_death_alone() {
        // Pin the exit witness on its own: the model died by SIGABRT and
        // the drive recorded -1 (see `RunOutcome.exit`). A future
        // verilator that stopped printing the faces would still be
        // convicted here.
        let outcome = RunOutcome {
            top: "tb_fatal".to_string(),
            exit: -1,
            log: String::new(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn an_error_run_convicts_on_both_witnesses() {
        // The model's $error: the face AND the SIGABRT death. Unlike
        // vvp's $error (which continues and exits 0), the built model
        // aborts -- probed live on verilator 5.032 (2026-11-07).
        let outcome = RunOutcome {
            top: "tb_err".to_string(),
            exit: -1,
            log: ERROR_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn the_lint_and_top_faces_judge_fail() {
        // The probed lint faces judge FAIL. Recorded for the vocabulary's
        // completeness; the lint step gates on verilator's honest exit
        // and is never judged.
        assert!(!contracts::judge(&Tool::Verilator, LINT_ERROR_LOG));
        assert!(!contracts::judge(&Tool::Verilator, UNKNOWN_TOP_LOG));
    }

    #[test]
    fn the_coded_error_face_subsumes_the_lint_gate() {
        // The bare `%Error` face subsumes the coded form
        // (`%Error-NEEDTIMINGOPT: ...` shares no shape with `%Error:`) --
        // the probed --timing gate, pinned at the vocabulary level.
        let log = "%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or --no-timing to specify \
                   how delays should be handled\n";
        assert!(!contracts::judge(&Tool::Verilator, log));
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a killed or crashed model can exit nonzero
        // without writing any probed face, and a clean log must not
        // green-light it. Either witness convicts.
        let outcome = RunOutcome {
            top: "tb_crash".to_string(),
            exit: 137,
            log: "tb quiet\n".to_string(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_warning_run_passes_the_gate() {
        // The probed gentle faces (`%Warning:` and the report banner) on
        // a continuing run that exits 0: continuations, not failures.
        let outcome = RunOutcome {
            top: "tb_vwarn".to_string(),
            exit: 0,
            log: WARNING_LOG.to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn drive_errors_map_to_honest_exit_codes() {
        assert_eq!(
            DriveError::Blueprint(BlueprintError::Read {
                path: std::path::Path::new("nope").to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "nope"),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Blueprint(BlueprintError::Parse(MalformedLine {
                line: 1,
                text: "VLOG\twork".to_string(),
            }))
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::NotCarried(NotCarried {
                entries: Vec::new()
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Spawn("verilator: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Lint {
                stderr: String::new()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            DriveError::Build {
                top: "tb".to_string(),
                stderr: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(DriveError::Findings(Vec::new()).exit_code(), 1);
    }

    #[test]
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("verilator", Some("tsv")).is_none());
        assert!(plan_note("verilator", None).is_none());
        let note = plan_note("verilator", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: verilator: note:"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Lint {
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "the verilator lint failed");
        let e = DriveError::Build {
            top: "tb".to_string(),
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "the verilator build failed for tb");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
    }
}
