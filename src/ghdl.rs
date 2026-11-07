//! The ghdl target drive: the orbit blueprint in, a judged verdict out.
//!
//! Two orbit targets share this drive, split by [`Mode`]: `ghdl-build`
//! (analyze + elaborate per top, exit honest, no simulation, nothing
//! judged) and `ghdl` (the same drive run to the end: elaborate, run,
//! judge). The drive reads the blueprint orbit wrote for the target (the
//! TSV plan: one `fileset<TAB>library<TAB>filepath` line per collected
//! file), analyzes the VHDL entries in blueprint order with the deferral
//! loop, and elaborates every requested top; the sim face then runs each
//! top and judges the run log against ghdl's probed failure vocabulary.
//! The judge -- not ghdl's exit code -- is the sim face's gate: ghdl's
//! exit is honest for a `severity failure` stop but lies for a violated
//! `severity error` check (exit 0), and the face in the log is the only
//! witness. The build face needs no judge: `ghdl -a` and `ghdl -e` exit
//! nonzero on a broken design (probed live), so the exit is honest there.
//!
//! The blueprint parse, the request view, and the fileset filter are the
//! shared machinery of [`crate::blueprint`]; what lives here is ghdl's
//! own process -- the deferral loop (shared with the nvc drive, which
//! still borrows it from this first drive that carried it), the library
//! wipe, the run, and the judge.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::blueprint::{
    parse_blueprint, plan_note, vhdl_only, BlueprintError, Drive, Mode, NotCarried,
};
use crate::contracts::{self, Tool};
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

/// The ghdl binary the drive spawns, resolved on PATH. The orbit target
/// contract carries no binary-path field.
pub const GHDL_BIN: &str = "ghdl";

/// The language standard pinned for every ghdl invocation (VHDL-2008).
/// ghdl's argv is command-first (`ghdl -a --std=08 <file>`): the command
/// flag leads and this option follows it, never the other way round.
pub const STD: &str = "--std=08";

/// The simulation option that makes a violated `severity error` stop the
/// run, probed live on ghdl 6.0.0 (2026-11-07). It is a SIMULATION option:
/// legal only after the toplevel unit -- placed before it, ghdl answers
/// `ghdl:error: unknown command option '--assert-level=error'`. It is
/// defense in depth only: the judge stays the gate, because the face must
/// land in the log either way.
pub const ASSERT_LEVEL: &str = "--assert-level=error";

/// The work library file ghdl writes into the working directory for
/// `--std=08` (ghdl names it after the standard: `-obj08`). It is ghdl's
/// clock.
///
/// Orbit's output directory persists across runs, and a stale library
/// turns this drive's own re-analysis against itself: in the
/// consumer-first order the deferral loop exists for, run 2 re-analyzes
/// the consumer BEFORE its provider -- the provider's re-analysis then
/// obsoletes the consumer's units, and `ghdl -e` refuses the unchanged
/// design (`error: architecture "rtl" of "top" is obsoleted by entity
/// "gate"`, probed live on ghdl 6.0.0, 2026-11-07). Correctness before
/// incrementality in v1: every drive starts from an empty library.
pub const WORK_LIBRARY_FILE: &str = "work-obj08.cf";

/// A file that never analyzed, with the stderr of its last attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Residual {
    /// The file, as the blueprint wrote it.
    pub path: PathBuf,
    /// The stderr of the last `ghdl -a` attempt, verbatim.
    pub stderr: String,
}

/// The outcome of the analyze deferral loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeReport {
    /// The files that analyzed, in the order they succeeded.
    pub analyzed: Vec<PathBuf>,
    /// The files that never analyzed, in blueprint order.
    pub residual: Vec<Residual>,
    /// The passes the loop ran.
    pub passes: usize,
}

/// Analyze `files` in blueprint order with the exit-code-driven deferral
/// loop.
///
/// ghdl is order-sensitive: a file whose design units reference units of a
/// not-yet-analyzed file fails to analyze. A failing analyze therefore
/// defers its file to a retry after the others; the loop stops when every
/// file analyzed or when a pass makes no progress, whichever comes first.
/// Each pass either retires at least one file or ends the loop, so the
/// pass count is bounded by the file count.
///
/// The `analyze` closure is one `ghdl -a` attempt: `Ok` when ghdl accepted
/// the file, `Err` with the stderr when it did not.
pub fn analyze_with_deferral(
    files: &[PathBuf],
    mut analyze: impl FnMut(&Path) -> Result<(), String>,
) -> AnalyzeReport {
    let mut pending: Vec<(PathBuf, Option<String>)> =
        files.iter().map(|path| (path.clone(), None)).collect();
    let mut analyzed = Vec::new();
    let mut passes = 0;
    while !pending.is_empty() {
        passes += 1;
        let mut progress = false;
        let mut deferred = Vec::new();
        for (path, _) in std::mem::take(&mut pending) {
            match analyze(&path) {
                Ok(()) => {
                    analyzed.push(path);
                    progress = true;
                }
                Err(stderr) => deferred.push((path, Some(stderr))),
            }
        }
        pending = deferred;
        if !progress {
            break;
        }
    }
    AnalyzeReport {
        analyzed,
        passes,
        residual: pending
            .into_iter()
            .map(|(path, stderr)| Residual {
                path,
                stderr: stderr.unwrap_or_default(),
            })
            .collect(),
    }
}

/// The record of one top's run: what ghdl exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top unit that ran.
    pub top: String,
    /// ghdl's exit code, recorded but secondary by ruling: it never flips
    /// the verdict.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed ghdl failure face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Ghdl, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face in
    /// the log AND ghdl exited 0.
    ///
    /// Either witness convicts. The face catches what the exit cannot see:
    /// bare ghdl exits 0 on a violated `severity error` check (probed live
    /// on ghdl 6.0.0), and there the face is the only witness. The exit
    /// catches what the log cannot name: a killed or crashed simulation
    /// can exit nonzero without writing any probed face, and a clean log
    /// must not green-light it. The drive also pins
    /// `--assert-level=error` onto the run argv, so a severity-error stop
    /// lands with a nonzero exit too -- the two witnesses agree there.
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
    /// ghdl could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale work library could not be wiped: usage-class, exit 2.
    Library(String),
    /// Files never analyzed after the deferral loop exhausted: findings,
    /// exit 1.
    Analyze(AnalyzeReport),
    /// A top failed to elaborate: findings, exit 1.
    Elaborate {
        /// The top unit ghdl refused.
        top: String,
        /// The stderr of the failed `ghdl -e`, verbatim.
        stderr: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings ghdl or the judge reported.
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

/// Drive the ghdl target end to end: blueprint, analyze, elaborate, and
/// -- on the sim face -- run and judge.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and
/// ghdl's library files land there.
///
/// All progress and diagnostics are printed here; the returned error only
/// picks the exit code. The mode picks how far the drive goes: `Build`
/// stops after the elaborations (no run, no log, nothing judged); `Sim`
/// runs each elaborated top and judges it.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    // ghdl must be spawnable before anything else, so a missing binary is
    // a loud usage error instead of a deferral-loop residual.
    capture(GHDL_BIN, Command::new(GHDL_BIN).arg("--version")).map_err(DriveError::Spawn)?;

    // Run-scoped library state: see `WORK_LIBRARY_FILE`. Every drive
    // starts from an empty library, so a second run in the same
    // persistent output directory cannot obsolete its own design.
    if let Err(e) = fs::remove_file(WORK_LIBRARY_FILE) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale work library {WORK_LIBRARY_FILE}: {e}");
            eprintln!("orbit-drives: ghdl: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    if let Some(note) = plan_note(
        "ghdl",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: ghdl: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: ghdl: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let files = vhdl_only(&entries).map_err(|e| {
        eprintln!("orbit-drives: ghdl: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-drives: ghdl: {label}: analyzing {} VHDL file(s) in blueprint order",
        files.len()
    );
    let report = analyze_with_deferral(&files, analyze_once);
    println!(
        "orbit-drives: ghdl: {label}: {} file(s) analyzed in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if !report.residual.is_empty() {
        eprintln!(
            "orbit-drives: ghdl: error: {} file(s) never analyzed after {} pass(es)",
            report.residual.len(),
            report.passes
        );
        for residual in &report.residual {
            eprintln!(
                "orbit-drives: ghdl: error: analyze failed: {}",
                residual.path.display()
            );
            eprint_block(&residual.stderr);
        }
        return Err(DriveError::Analyze(report));
    }

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-drives: ghdl: {label}: elaborating {top}");
        let mut elaborate = Command::new(GHDL_BIN);
        elaborate.args(["-e", STD]).arg(top);
        // The extras ride the drive's last step verbatim: the run argv on
        // the sim face, the elaboration argv on the build face (there is
        // no run to carry them, and silently dropping a user argument
        // would be a degradation). An option the elaboration does not
        // speak is ghdl's own loud rejection, surfaced as an exit-1
        // elaborate failure.
        if mode == Mode::Build {
            elaborate.args(&request.extra);
        }
        let output = capture(GHDL_BIN, &mut elaborate).map_err(DriveError::Spawn)?;
        if !output.status.success() {
            let stderr = lossy(&output.stderr);
            eprint_block(&stderr);
            eprintln!("orbit-drives: ghdl: error: ghdl -e failed for {top}");
            return Err(DriveError::Elaborate {
                top: top.clone(),
                stderr,
            });
        }

        match mode {
            Mode::Build => println!("orbit-drives: ghdl: {label}: {top}: elaborated"),
            Mode::Sim => {
                let outcome = run_top(top, &request.extra, label)?;
                if outcome.clean() {
                    println!("orbit-drives: ghdl: {label}: {top}: pass");
                } else if outcome.has_face() {
                    println!(
                        "orbit-drives: ghdl: {label}: {top}: fail (the log carries a probed ghdl failure face)"
                    );
                    findings.push(outcome);
                } else {
                    println!(
                        "orbit-drives: ghdl: {label}: {top}: fail (ghdl exited {} without a judged face)",
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
            "orbit-drives: ghdl: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// Run one elaborated top and capture its log: `ghdl -r --std=08 <top>
/// --assert-level=error <extras>`.
fn run_top(top: &str, extra: &[String], label: &str) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: ghdl: {label}: running {top}");
    // `--assert-level=error` rides AFTER the top unit: it is a
    // simulation option, and ghdl rejects it anywhere earlier.
    let mut run = Command::new(GHDL_BIN);
    run.args(["-r", STD]).arg(top).arg(ASSERT_LEVEL).args(extra);
    let output = capture(GHDL_BIN, &mut run).map_err(DriveError::Spawn)?;
    let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
    print_block(&log);
    Ok(RunOutcome {
        top: top.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

/// One `ghdl -a` attempt: `Ok` when ghdl accepted the file, `Err` with the
/// stderr when it did not.
fn analyze_once(file: &Path) -> Result<(), String> {
    let mut command = Command::new(GHDL_BIN);
    command.args(["-a", STD]).arg(file);
    match capture(GHDL_BIN, &mut command) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(lossy(&output.stderr)),
        Err(message) => Err(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::MalformedLine;

    // The provider and consumer of the order-dependence fixture: the
    // consumer references the provider's unit, so ghdl cannot analyze the
    // consumer until the provider is done.
    const PROVIDER_VHDL: &str = "\
entity gate is
  port (a : in bit; y : out bit);
end gate;
architecture rtl of gate is
begin
  y <= not a;
end rtl;
";

    const CONSUMER_VHDL: &str = "\
entity top is
  port (a : in bit; y : out bit);
end top;
architecture rtl of top is
begin
  u : entity work.gate port map (a, y);
end rtl;
";

    // The probed severity-error fixture (ghdl 6.0.0, 2026-11-07): bare
    // ghdl prints the face and exits 0 anyway. Byte-for-byte as recorded
    // in the aquila contract suite (github.com/chaseruskin/aquila).
    const SEVERITY_ERROR_LOG: &str = "\
tb_err.vhd:8:5:@0ms:(assertion error): check failed
tb_err.vhd:9:5:@0ms:(report note): simulation continued
";

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orbit-drives-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn deferral_retries_the_consumer_after_its_provider() {
        // Two tiny VHDL files in a temp dir, consumer listed FIRST -- the
        // order ghdl cannot forgive. The fake runner fails the consumer
        // until the provider has analyzed; the loop must defer the
        // failure and retry the consumer after the provider.
        let dir = temp_dir("deferral-retry");
        let provider = dir.join("gate.vhd");
        let consumer = dir.join("top.vhd");
        fs::write(&provider, PROVIDER_VHDL).unwrap();
        fs::write(&consumer, CONSUMER_VHDL).unwrap();

        let mut provider_done = false;
        let report = analyze_with_deferral(&[consumer.clone(), provider.clone()], |file| {
            if file == provider.as_path() {
                provider_done = true;
                Ok(())
            } else if file == consumer.as_path() && !provider_done {
                Err(format!(
                    "{}:6:9: unit 'gate' is not visible: analyze {} first",
                    consumer.display(),
                    provider.display()
                ))
            } else {
                Ok(())
            }
        });

        assert_eq!(report.passes, 2);
        assert_eq!(report.analyzed, vec![provider.clone(), consumer.clone()]);
        assert!(report.residual.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deferral_stops_on_a_no_progress_pass_and_surfaces_the_residual() {
        let dir = temp_dir("deferral-residual");
        let good = dir.join("good.vhd");
        let bad = dir.join("bad.vhd");
        fs::write(&good, PROVIDER_VHDL).unwrap();
        fs::write(&bad, "entity broken is\nend broken;\n").unwrap();

        // Pass 1 retires the good file and defers the failure; pass 2
        // makes no progress and stops the loop, with the residual
        // surfaced.
        let stderr = format!("{}:1:19: 'end broken' expected a ';'", bad.display());
        let report = analyze_with_deferral(&[good.clone(), bad.clone()], |file| {
            if file == bad.as_path() {
                Err(stderr.clone())
            } else {
                Ok(())
            }
        });

        assert_eq!(report.passes, 2);
        assert_eq!(report.analyzed, vec![good.clone()]);
        assert_eq!(
            report.residual,
            vec![Residual {
                path: bad.clone(),
                stderr: stderr.clone(),
            }]
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deferral_runs_zero_passes_on_an_empty_blueprint() {
        let report = analyze_with_deferral(&[], |_file| Ok(()));
        assert_eq!(report.passes, 0);
        assert!(report.analyzed.is_empty());
        assert!(report.residual.is_empty());
    }

    #[test]
    fn a_clean_run_log_passes_the_gate() {
        let outcome = RunOutcome {
            top: "tb_pass".to_string(),
            exit: 0,
            log: "tb_pass.vhd:7:5:@0ms:(report note): all checks passed\n".to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a killed or crashed sim can exit nonzero without
        // writing any probed face, and a clean log must not green-light
        // it. Either witness convicts.
        let outcome = RunOutcome {
            top: "tb_crash".to_string(),
            exit: 137,
            log: "tb_crash.vhd:7:5:@0ms:(report note): sim done\n".to_string(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_face_convicts_an_exit_zero_run() {
        // The judge catches what the exit cannot see: bare ghdl exits 0
        // on a violated `severity error` check, probed live on ghdl 6.0.0
        // (2026-11-07). The verdict reads the log, where the face lands.
        let outcome = RunOutcome {
            top: "tb_err".to_string(),
            exit: 0,
            log: SEVERITY_ERROR_LOG.to_string(),
        };
        assert_eq!(outcome.exit, 0);
        assert!(outcome.has_face());
        assert!(!outcome.clean());
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
                entries: Vec::new()
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Spawn("ghdl: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Analyze(AnalyzeReport {
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
}
