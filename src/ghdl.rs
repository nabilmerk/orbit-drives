//! The ghdl target drive: the orbit blueprint in, a judged verdict out.
//!
//! The drive reads the blueprint orbit wrote for the target (the TSV plan:
//! one `fileset<TAB>library<TAB>filepath` line per collected file),
//! analyzes the VHDL entries in blueprint order with the deferral loop,
//! elaborates and runs every requested top, and judges each run log
//! against ghdl's probed failure vocabulary. The judge -- not ghdl's exit
//! code -- is the gate: ghdl's exit is honest for a `severity failure`
//! stop but lies for a violated `severity error` check (exit 0), and the
//! face in the log is the only witness.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::contracts::{self, Tool};

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

/// One entry of the blueprint: a fileset, a library, and the file's path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The 1-based blueprint line the entry came from.
    pub line: usize,
    /// The fileset orbit filed the file under (`VHDL`, `VLOG`, `SYSV`, or
    /// a custom fileset).
    pub fileset: String,
    /// The orbit library the file belongs to. Carried but not yet mapped:
    /// v1 analyzes every file into ghdl's `work` library.
    pub library: String,
    /// The file path, as the blueprint wrote it.
    pub path: PathBuf,
}

/// The error for a blueprint line that is not
/// `fileset<TAB>library<TAB>filepath`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MalformedLine {
    /// The 1-based blueprint line number.
    pub line: usize,
    /// The offending line's text, line terminator stripped.
    pub text: String,
}

impl fmt::Display for MalformedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "blueprint line {} is not fileset<TAB>library<TAB>filepath: {:?}",
            self.line, self.text
        )
    }
}

impl std::error::Error for MalformedLine {}

/// The error for a blueprint the drive could not read or parse.
#[derive(Debug)]
pub enum BlueprintError {
    /// The blueprint file could not be read.
    Read {
        /// The blueprint path, as the drive was handed it.
        path: PathBuf,
        /// The read failure.
        source: std::io::Error,
    },
    /// A line was not `fileset<TAB>library<TAB>filepath`.
    Parse(MalformedLine),
}

impl fmt::Display for BlueprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlueprintError::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            BlueprintError::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BlueprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BlueprintError::Read { source, .. } => Some(source),
            BlueprintError::Parse(e) => Some(e),
        }
    }
}

/// Parse the TSV blueprint text into entries, in file order.
///
/// Blank lines are skipped; anything else must carry exactly three
/// tab-separated fields.
pub fn parse_blueprint(text: &str) -> Result<Vec<Entry>, MalformedLine> {
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 3 {
            return Err(MalformedLine {
                line: index + 1,
                text: line.to_string(),
            });
        }
        entries.push(Entry {
            line: index + 1,
            fileset: fields[0].to_string(),
            library: fields[1].to_string(),
            path: PathBuf::from(fields[2]),
        });
    }
    Ok(entries)
}

/// The error for blueprint filesets the ghdl drive does not carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// Every non-VHDL entry, in blueprint order.
    pub entries: Vec<Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the ghdl drive carries the VHDL fileset only; not yet carried:"
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

/// The VHDL files of the blueprint, in blueprint order.
///
/// Every non-VHDL entry -- `VLOG`, `SYSV`, a custom fileset -- is rejected
/// loudly: mixed-language designs stay outside this drive's contract, and
/// a silent skip would leave files the user believes were simulated.
pub fn vhdl_only(entries: &[Entry]) -> Result<Vec<PathBuf>, NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| entry.fileset != "VHDL")
        .cloned()
        .collect();
    if foreign.is_empty() {
        Ok(entries.iter().map(|entry| entry.path.clone()).collect())
    } else {
        Err(NotCarried { entries: foreign })
    }
}

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
    /// The gate: `true` when the log carries no probed ghdl failure face.
    ///
    /// The judge is the gate, not the exit. ghdl exits honestly on a
    /// `severity failure` stop (1), but a violated `severity error` check
    /// exits 0 -- probed live on ghdl 6.0.0 -- so an exit gate alone would
    /// pass a failing run. The reverse hole (a nonzero exit with a clean
    /// log, a crash face) is a recorded v1 limit: the vocabulary carries
    /// only probed faces, and the exit stays secondary.
    pub fn clean(&self) -> bool {
        contracts::judge(&Tool::Ghdl, &self.log)
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
            DriveError::Blueprint(_) | DriveError::NotCarried(_) | DriveError::Spawn(_) => 2,
            DriveError::Analyze(_) | DriveError::Elaborate { .. } | DriveError::Findings(_) => 1,
        }
    }
}

/// Everything argv parsing produced: the drive's own view of the request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    /// The blueprint file: the `--blueprint` path or `$ORBIT_BLUEPRINT`.
    pub blueprint: PathBuf,
    /// The top units to elaborate and run, in the order given. The caller
    /// resolves the `$ORBIT_TB_NAME` default; the drive runs what it is
    /// handed.
    pub tops: Vec<String>,
    /// The extra tokens riding each `ghdl -r` argv: orbit appends the
    /// user's extras with the `--` separator stripped, and the argv
    /// parser passes through every token the drive does not own.
    pub extra: Vec<String>,
}

/// The note for a blueprint plan the drive does not carry: orbit exports
/// `$ORBIT_BLUEPRINT_PLAN` as the plan it wrote, and the drive speaks the
/// TSV plan only. Loud but non-fatal -- the file is still read as TSV
/// lines, and a non-TSV file fails the parse loudly right after.
fn plan_note(plan: Option<&str>) -> Option<String> {
    match plan {
        Some(plan) if plan != "tsv" => Some(format!(
            "orbit-contracts: ghdl: note: $ORBIT_BLUEPRINT_PLAN is {plan:?}; the drive \
             carries the TSV plan only, reading the blueprint as TSV lines"
        )),
        _ => None,
    }
}

/// Drive the ghdl target end to end: blueprint, analyze, elaborate, run,
/// judge.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and
/// ghdl's library files land there.
///
/// All progress and diagnostics are printed here; the returned error only
/// picks the exit code.
pub fn drive(request: &Drive) -> Result<(), DriveError> {
    // ghdl must be spawnable before anything else, so a missing binary is
    // a loud usage error instead of a deferral-loop residual.
    capture(Command::new(GHDL_BIN).arg("--version"))?;

    if let Some(note) = plan_note(std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref()) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-contracts: ghdl: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-contracts: ghdl: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let files = vhdl_only(&entries).map_err(|e| {
        eprintln!("orbit-contracts: ghdl: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-contracts: ghdl: analyzing {} VHDL file(s) in blueprint order",
        files.len()
    );
    let report = analyze_with_deferral(&files, analyze_once);
    println!(
        "orbit-contracts: ghdl: {} file(s) analyzed in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if !report.residual.is_empty() {
        eprintln!(
            "orbit-contracts: ghdl: error: {} file(s) never analyzed after {} pass(es)",
            report.residual.len(),
            report.passes
        );
        for residual in &report.residual {
            eprintln!(
                "orbit-contracts: ghdl: error: analyze failed: {}",
                residual.path.display()
            );
            eprint_block(&residual.stderr);
        }
        return Err(DriveError::Analyze(report));
    }

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-contracts: ghdl: elaborating {top}");
        let mut elaborate = Command::new(GHDL_BIN);
        elaborate.args(["-e", STD]).arg(top);
        let output = capture(&mut elaborate)?;
        if !output.status.success() {
            let stderr = lossy(&output.stderr);
            eprint_block(&stderr);
            eprintln!("orbit-contracts: ghdl: error: ghdl -e failed for {top}");
            return Err(DriveError::Elaborate {
                top: top.clone(),
                stderr,
            });
        }

        println!("orbit-contracts: ghdl: running {top}");
        // `--assert-level=error` rides AFTER the top unit: it is a
        // simulation option, and ghdl rejects it anywhere earlier.
        let mut run = Command::new(GHDL_BIN);
        run.args(["-r", STD])
            .arg(top)
            .arg(ASSERT_LEVEL)
            .args(&request.extra);
        let output = capture(&mut run)?;
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

        let outcome = RunOutcome {
            top: top.clone(),
            exit: output.status.code().unwrap_or(-1),
            log,
        };
        if outcome.clean() {
            println!("orbit-contracts: ghdl: {top}: pass");
            if outcome.exit != 0 {
                eprintln!(
                    "orbit-contracts: ghdl: note: ghdl exited {} for {top} but its log judges clean; the exit stays secondary",
                    outcome.exit
                );
            }
        } else {
            println!(
                "orbit-contracts: ghdl: {top}: fail (the log carries a probed ghdl failure face)"
            );
            findings.push(outcome);
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "orbit-contracts: ghdl: error: {} run(s) carried a ghdl failure face",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// One `ghdl -a` attempt: `Ok` when ghdl accepted the file, `Err` with the
/// stderr when it did not.
fn analyze_once(file: &Path) -> Result<(), String> {
    let mut command = Command::new(GHDL_BIN);
    command.args(["-a", STD]).arg(file);
    match capture(&mut command) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(lossy(&output.stderr)),
        Err(DriveError::Spawn(message)) => Err(message),
        Err(_) => Err(format!("{GHDL_BIN}: analyze failed")),
    }
}

/// Spawn a command and capture its output, mapping a spawn failure to the
/// usage-class error.
fn capture(command: &mut Command) -> Result<Output, DriveError> {
    command
        .output()
        .map_err(|e| DriveError::Spawn(format!("{GHDL_BIN}: {e}")))
}

/// The captured bytes as text: ghdl writes UTF-8, and the verdict must not
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

    // The controller-probed severity-error fixture (ghdl 6.0.0): the face
    // lands and ghdl exits 0 anyway. Byte-for-byte as recorded.
    const SEVERITY_ERROR_LOG: &str = "\
tb_err.vhd:8:5:@0ms:(assertion error): check failed
tb_err.vhd:9:5:@0ms:(report note): simulation continued
";

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("orbit-contracts-{name}-{}", std::process::id()));
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
    fn blueprint_parses_the_tsv_plan_in_file_order() {
        let text = "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\n";
        let entries = parse_blueprint(text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].line, 1);
        assert_eq!(entries[0].fileset, "VHDL");
        assert_eq!(entries[0].library, "work");
        assert_eq!(entries[0].path, PathBuf::from("./gate.vhd"));
        assert_eq!(entries[1].line, 2);
        assert_eq!(entries[1].path, PathBuf::from("./top.vhd"));
    }

    #[test]
    fn blueprint_skips_blank_lines_and_strips_carriage_returns() {
        let text = "VHDL\twork\t./a.vhd\r\n\nVHDL\twork\t./b.vhd\n";
        let entries = parse_blueprint(text).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("./a.vhd"));
        assert_eq!(entries[1].line, 3);
    }

    #[test]
    fn blueprint_rejects_a_short_line_loudly() {
        let err = parse_blueprint("VHDL\twork\t./a.vhd\nVHDL\twork\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert_eq!(err.text, "VHDL\twork");
    }

    #[test]
    fn blueprint_rejects_an_extra_field_loudly() {
        let err = parse_blueprint("VHDL\twork\t./a.vhd\textra\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn the_drive_carries_vhdl_only_and_names_the_rest() {
        let entries =
            parse_blueprint("VHDL\twork\t./a.vhd\nVLOG\twork\t./b.v\nSYSV\twork\t./c.sv\n")
                .unwrap();
        let err = vhdl_only(&entries).unwrap_err();
        assert_eq!(err.entries.len(), 2);
        assert_eq!(err.entries[0].fileset, "VLOG");
        assert_eq!(err.entries[1].fileset, "SYSV");
        let carried = parse_blueprint("VHDL\twork\t./a.vhd\n").unwrap();
        assert_eq!(vhdl_only(&carried).unwrap(), vec![PathBuf::from("./a.vhd")]);
    }

    #[test]
    fn the_judge_is_the_gate_on_a_severity_error_run() {
        // Probed live on ghdl 6.0.0 (2026-11-07): the violated check
        // exits 0 -- the honest exit would pass it. The verdict reads the
        // log, where the face lands.
        let outcome = RunOutcome {
            top: "tb_err".to_string(),
            exit: 0,
            log: SEVERITY_ERROR_LOG.to_string(),
        };
        assert!(!outcome.clean());
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
