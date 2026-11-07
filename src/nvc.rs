//! The nvc target drive: the same shapes as the ghdl drive, nvc's probed
//! faces.
//!
//! The drive reads the blueprint orbit wrote for the target (the TSV plan:
//! one `fileset<TAB>library<TAB>filepath` line per collected file),
//! analyzes the VHDL entries in blueprint order with the deferral loop,
//! and elaborates every requested top; the sim face then runs each top and
//! judges the run log against nvc's probed failure vocabulary. Two orbit
//! targets share this drive, split by [`Mode`]: `nvc-build` (analyze +
//! elaborate per top, exit honest, no simulation, nothing judged) and
//! `nvc` (elaborate, run, judge).
//!
//! The TSV blueprint is orbit's plan format and the deferral loop is
//! order-independence machinery, not ghdl's: both live in [`crate::ghdl`]
//! with the first drive that carried them, and both drives share them.
//! Everything that is genuinely nvc's -- the argv shape, the library
//! state, the faces -- is probed and pinned here.
//!
//! The judge gates the sim face as second witness, not first: nvc's exits
//! probed honest across every failure face (a `severity error` run
//! continues but still exits 1, unlike bare ghdl's exit 0), and the face
//! in the log convicts regardless -- either witness convicts, everywhere.

use std::fmt;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::contracts::{self, Tool};
use crate::ghdl::{self, Drive, Mode};

/// The nvc binary the drive spawns, resolved on PATH. The orbit target
/// contract carries no binary-path field.
pub const NVC_BIN: &str = "nvc";

/// The language standard pinned for every nvc invocation (VHDL-2008). It
/// is a GLOBAL option, and nvc's argv is option-FIRST -- the opposite of
/// ghdl's command-first shape, probed live on nvc 1.23.0 (2026-11-07):
/// after the command, nvc answers `** Fatal: unrecognised analysis option
/// --std=2008` (exit 1), so this option always leads the argv and never
/// follows a command. Run options, by contrast, are legal after the
/// toplevel unit (`nvc --std=2008 -r top --stop-time=1ns` probed clean).
pub const STD: &str = "--std=2008";

/// The work library directory nvc analyzes into (`work/`, relative to the
/// working directory). It is a DIRECTORY, not ghdl's single clock file,
/// and nvc creates it even for an analysis that fails on its first
/// option; elaboration leaves nothing beside it (nvc 1.23 stores the
/// elaborated unit inside the library).
///
/// Orbit's output directory persists across runs, and v1's stance is
/// correctness before incrementality: every drive starts from an empty
/// library. Probed honestly: nvc, unlike ghdl, tolerates re-analysis in a
/// stale library without the obsoleted-design refusal (a second drive of
/// provider-then-consumer analysis, elaboration and run exited 0 across
/// the board on nvc 1.23.0, 2026-11-07), so this wipe is not load-bearing
/// here the way ghdl's is -- it exists so both drives keep one stance and
/// every drive starts from a known-clean clock.
pub const WORK_LIBRARY_DIR: &str = "work";

/// The record of one top's run: what nvc exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top unit that ran.
    pub top: String,
    /// nvc's exit code, recorded but secondary: it never flips the
    /// verdict -- and for nvc it has probed honest, so on every probed
    /// failure the two witnesses agree.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed nvc failure face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Nvc, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face in
    /// the log AND nvc exited 0.
    ///
    /// Either witness convicts. For nvc the face has a probed companion in
    /// the exit on every failure seen so far (severity error continues the
    /// simulation but still exits 1), but the judge stays the gate: the
    /// exit cannot name the face, and a tool update that stops printing
    /// what it exits for must not silently pass. The exit catches what the
    /// log cannot name: a killed or crashed simulation can exit nonzero
    /// without writing any probed face, and a clean log must not
    /// green-light it.
    pub fn clean(&self) -> bool {
        !self.has_face() && self.exit == 0
    }
}

/// How the drive turned out. Every variant is rendered on stderr by
/// [`drive`] before it is returned; the variant only picks the exit code.
#[derive(Debug)]
pub enum DriveError {
    /// The blueprint could not be read or parsed: usage-class, exit 2.
    Blueprint(ghdl::BlueprintError),
    /// The blueprint carried filesets the drive does not speak yet:
    /// usage-class, exit 2.
    NotCarried(ghdl::NotCarried),
    /// nvc could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale work library could not be wiped: usage-class, exit 2.
    Library(String),
    /// Files never analyzed after the deferral loop exhausted: findings,
    /// exit 1.
    Analyze(ghdl::AnalyzeReport),
    /// A top failed to elaborate: findings, exit 1.
    Elaborate {
        /// The top unit nvc refused.
        top: String,
        /// The stderr of the failed `nvc -e`, verbatim.
        stderr: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings nvc or the judge reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_) => 2,
            DriveError::Analyze(_) | DriveError::Elaborate { .. } | DriveError::Findings(_) => 1,
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
            DriveError::Analyze(report) => write!(
                f,
                "{} file(s) never analyzed after {} pass(es)",
                report.residual.len(),
                report.passes
            ),
            DriveError::Elaborate { top, .. } => write!(f, "nvc -e failed for {top}"),
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

/// The note for a blueprint plan the drive does not carry: orbit exports
/// `$ORBIT_BLUEPRINT_PLAN` as the plan it wrote, and the drive speaks the
/// TSV plan only. Loud but non-fatal -- the file is still read as TSV
/// lines, and a non-TSV file fails the parse loudly right after.
fn plan_note(plan: Option<&str>) -> Option<String> {
    match plan {
        Some(plan) if plan != "tsv" => Some(format!(
            "orbit-contracts: nvc: note: $ORBIT_BLUEPRINT_PLAN is {plan:?}; the drive \
             carries the TSV plan only, reading the blueprint as TSV lines"
        )),
        _ => None,
    }
}

/// Drive the nvc target end to end: blueprint, analyze, elaborate, and --
/// on the sim face -- run and judge.
///
/// The request is the shared argv-level view every drive takes ([`Drive`];
/// the blueprint is orbit's format, the mode is the shared face split);
/// everything past it is nvc's own probed shape.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and nvc's
/// work library land there. All progress and diagnostics are printed
/// here; the returned error only picks the exit code. The mode picks how
/// far the drive goes: `Build` stops after the elaborations (no run, no
/// log, nothing judged); `Sim` runs each elaborated top and judges it.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    // nvc must be spawnable before anything else, so a missing binary is
    // a loud usage error instead of a deferral-loop residual.
    capture(Command::new(NVC_BIN).arg("--version")).map_err(DriveError::Spawn)?;

    // Run-scoped library state: see `WORK_LIBRARY_DIR`. Every drive
    // starts from an empty library, so a second run in the same
    // persistent output directory starts from a known-clean clock.
    if let Err(e) = fs::remove_dir_all(WORK_LIBRARY_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale work library {WORK_LIBRARY_DIR}/: {e}");
            eprintln!("orbit-contracts: nvc: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    if let Some(note) = plan_note(std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref()) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = ghdl::BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-contracts: nvc: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = ghdl::parse_blueprint(&text).map_err(|e| {
        let e = ghdl::BlueprintError::Parse(e);
        eprintln!("orbit-contracts: nvc: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let files = ghdl::vhdl_only(&entries).map_err(|e| {
        eprintln!("orbit-contracts: nvc: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-contracts: nvc: {label}: analyzing {} VHDL file(s) in blueprint order",
        files.len()
    );
    let report = ghdl::analyze_with_deferral(&files, analyze_once);
    println!(
        "orbit-contracts: nvc: {label}: {} file(s) analyzed in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if !report.residual.is_empty() {
        eprintln!(
            "orbit-contracts: nvc: error: {} file(s) never analyzed after {} pass(es)",
            report.residual.len(),
            report.passes
        );
        for residual in &report.residual {
            eprintln!(
                "orbit-contracts: nvc: error: analyze failed: {}",
                residual.path.display()
            );
            eprint_block(&residual.stderr);
        }
        return Err(DriveError::Analyze(report));
    }

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-contracts: nvc: {label}: elaborating {top}");
        let mut elaborate = Command::new(NVC_BIN);
        // nvc's argv is option-first: the global `--std` leads, the
        // command follows (`nvc --std=2008 -e <top>`); after the command
        // it is rejected (`** Fatal: unrecognised elaboration option`,
        // probed live).
        elaborate.arg(STD).arg("-e").arg(top);
        // The extras ride the drive's last step verbatim: the run argv on
        // the sim face, the elaboration argv on the build face (there is
        // no run to carry them, and silently dropping a user argument
        // would be a degradation). An option the elaboration does not
        // speak is nvc's own loud rejection, surfaced as an exit-1
        // elaborate failure.
        if mode == Mode::Build {
            elaborate.args(&request.extra);
        }
        let output = capture(&mut elaborate).map_err(DriveError::Spawn)?;
        if !output.status.success() {
            let stderr = lossy(&output.stderr);
            eprint_block(&stderr);
            eprintln!("orbit-contracts: nvc: error: nvc -e failed for {top}");
            return Err(DriveError::Elaborate {
                top: top.clone(),
                stderr,
            });
        }

        match mode {
            Mode::Build => println!("orbit-contracts: nvc: {label}: {top}: elaborated"),
            Mode::Sim => {
                let outcome = run_top(top, &request.extra, label)?;
                if outcome.clean() {
                    println!("orbit-contracts: nvc: {label}: {top}: pass");
                } else if outcome.has_face() {
                    println!(
                        "orbit-contracts: nvc: {label}: {top}: fail (the log carries a probed nvc failure face)"
                    );
                    findings.push(outcome);
                } else {
                    println!(
                        "orbit-contracts: nvc: {label}: {top}: fail (nvc exited {} without a judged face)",
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
            "orbit-contracts: nvc: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// Run one elaborated top and capture its log: `nvc --std=2008 -r <top>
/// <extras>`. The run options are legal after the toplevel unit (probed
/// clean with `--stop-time=1ns`); there is no `--assert-level` analogue
/// pinned for nvc because none is needed -- nvc's run exit probed honest
/// on every failure face, and the judge gates the log regardless.
fn run_top(top: &str, extra: &[String], label: &str) -> Result<RunOutcome, DriveError> {
    println!("orbit-contracts: nvc: {label}: running {top}");
    let mut run = Command::new(NVC_BIN);
    run.arg(STD).arg("-r").arg(top).args(extra);
    let output = capture(&mut run).map_err(DriveError::Spawn)?;
    // The log is both streams joined on their boundary; an empty side
    // contributes no line, so a quiet run relays no stray blank line.
    let stdout = lossy(&output.stdout);
    let stderr = lossy(&output.stderr);
    let log = match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (true, false) => stderr,
        (false, true) => stdout,
        (false, false) => format!("{stdout}\n{stderr}"),
    };
    print_block(&log);
    Ok(RunOutcome {
        top: top.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

/// One `nvc -a` attempt: `Ok` when nvc accepted the file, `Err` with the
/// stderr when it did not. The global `--std` leads the command, per the
/// probed option-first shape.
fn analyze_once(file: &Path) -> Result<(), String> {
    let mut command = Command::new(NVC_BIN);
    command.arg(STD).arg("-a").arg(file);
    match capture(&mut command) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(lossy(&output.stderr)),
        Err(message) => Err(message),
    }
}

/// Spawn a command and capture its output, mapping a spawn failure to the
/// usage-class error.
fn capture(command: &mut Command) -> Result<Output, String> {
    command.output().map_err(|e| format!("{NVC_BIN}: {e}"))
}

/// The captured bytes as text: nvc writes UTF-8, and the verdict must not
/// panic on anything less.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Print a captured block verbatim, guaranteeing one final newline.
fn print_block(text: &str) {
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
fn eprint_block(text: &str) {
    if text.is_empty() {
        return;
    }
    if text.ends_with('\n') {
        eprint!("{text}");
    } else {
        eprintln!("{text}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The probed severity-failure fixture (nvc 1.23.0, 2026-11-07): the
    // run stops, exit 1. Byte-for-byte as recorded live.
    const SEVERITY_FAILURE_LOG: &str = "\
** Failure: 0ms+0: the design is wrong
   Process :tb_fail:_p0 at tb_fail.vhd:5
";

    // The probed severity-error fixture (nvc 1.23.0, 2026-11-07): nvc
    // continues the simulation but STILL exits 1 -- unlike bare ghdl,
    // whose severity-error run exits 0, nvc's exit convicts on its own,
    // and the face is the second witness.
    const SEVERITY_ERROR_LOG: &str = "\
** Error: 0ms+0: check failed
   Process :tb_err:_p0 at tb_err.vhd:5
** Note: 0ms+0: simulation continued
   Process :tb_err:_p0 at tb_err.vhd:5
";

    #[test]
    fn a_severity_failure_run_is_a_finding_on_both_witnesses() {
        // The probed stop: face AND exit 1. Either witness convicts; here
        // both do.
        let outcome = RunOutcome {
            top: "tb_fail".to_string(),
            exit: 1,
            log: SEVERITY_FAILURE_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_severity_error_run_convicts_on_the_face_alone() {
        // nvc continues past a severity error and exits 1 anyway, probed
        // live on nvc 1.23.0 (2026-11-07) -- so this run convicts twice.
        // Pin the face witness on its own: an exit of 0 (a future nvc
        // that goes quiet) would still be a finding here.
        let outcome = RunOutcome {
            top: "tb_err".to_string(),
            exit: 0,
            log: SEVERITY_ERROR_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a killed or crashed sim can exit nonzero
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
    fn a_clean_run_log_passes_the_gate() {
        // The probed gentle faces (`** Warning:`, `** Note:`) on a
        // continuing run that exits 0: continuations, not failures.
        let outcome = RunOutcome {
            top: "tb_warn".to_string(),
            exit: 0,
            log: "\
** Warning: 0ms+0: check flagged
   Process :tb_warn:_p0 at tb_warn.vhd:5
** Note: 0ms+0: all quiet
   Process :tb_warn:_p0 at tb_warn.vhd:5
"
            .to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn drive_errors_map_to_honest_exit_codes() {
        assert_eq!(
            DriveError::Blueprint(ghdl::BlueprintError::Read {
                path: Path::new("nope").to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "nope"),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Blueprint(ghdl::BlueprintError::Parse(ghdl::MalformedLine {
                line: 1,
                text: "VHDL\twork".to_string(),
            }))
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::NotCarried(ghdl::NotCarried {
                entries: Vec::new()
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Spawn("nvc: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Analyze(ghdl::AnalyzeReport {
                analyzed: Vec::new(),
                residual: Vec::new(),
                passes: 0,
            })
            .exit_code(),
            1
        );
        assert_eq!(
            DriveError::Elaborate {
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
        assert!(plan_note(Some("tsv")).is_none());
        assert!(plan_note(None).is_none());
        let note = plan_note(Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-contracts: nvc: note:"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Elaborate {
            top: "tb".to_string(),
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "nvc -e failed for tb");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
    }
}
