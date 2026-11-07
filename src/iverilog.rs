//! The iverilog target drive: the orbit blueprint in, a judged verdict
//! out.
//!
//! Two orbit targets share this drive, split by [`Mode`]: `iverilog-build`
//! (the one compile, exit honest, no simulation, nothing judged) and
//! `iverilog` (the same compile, then the judged `vvp` run). The drive
//! reads the blueprint orbit wrote for the target (the TSV plan: one
//! `fileset<TAB>library<TAB>filepath` line per collected file), compiles
//! the Verilog entries into ONE snapshot, and -- on the sim face -- runs
//! that snapshot once through `vvp` and judges the run log against
//! iverilog's probed failure vocabulary.
//!
//! There is NO deferral loop here, on probed grounds: iverilog's compile
//! is a single invocation over all files and it is order-free (a
//! consumer-before-provider pair compiled clean, probed live on Icarus
//! Verilog 12.0, 2026-11-07), so there is nothing to defer -- one
//! compile, no passes. The `-s` root flags ride the compile argv, and
//! iverilog validates every root name against the source at compile time
//! (`error: Unable to find the root module "nosuch_top" in the Verilog
//! source.`, exit 1, probed), so a top that names nothing fails the
//! compile loudly on both faces.
//!
//! The judge -- not vvp's exit code -- is the sim face's load-bearing
//! gate: vvp exits 0 on a `$error` call (probed live -- it prints the
//! `ERROR:` face, continues the simulation, and exits 0), exactly like
//! bare ghdl's severity-error exit; the face in the log is the only
//! witness on that failure. The compile needs no judge: iverilog exits
//! nonzero on a broken design (exit 2 on a parse error, exit 1 on an
//! unknown `-s` root, probed live), so the exit is honest there.

use std::fmt;
use std::fs;
use std::process::{Command, Output};

use crate::contracts::{self, Tool};
use crate::ghdl::{self, Drive, Mode};

/// The iverilog binary the drive spawns, resolved on PATH. The orbit
/// target contract carries no binary-path field.
pub const IVERILOG_BIN: &str = "iverilog";

/// The vvp runtime the sim face spawns, resolved on PATH. The snapshot
/// iverilog writes is vvp's input, and the run is vvp's.
pub const VVP_BIN: &str = "vvp";

/// The language generation pinned for every iverilog invocation
/// (SystemVerilog-2012), probed live on Icarus Verilog 12.0 (stable,
/// 2026-11-07). It is the generation where `$fatal` and `$error` are
/// first-class, and the generation under which the runtime faces were
/// probed; the runtime faces are only as probed as this pin. The
/// probed-optional fact is recorded too: iverilog 12.0's default
/// generation also accepts `$fatal` at compile time (exit 0), but the
/// drive pins `-g2012` so the judged faces cannot drift with the
/// default.
pub const GEN: &str = "-g2012";

/// The snapshot file iverilog writes (`-o`) and vvp runs. It is iverilog's
/// clock, rewritten by every compile into the working directory.
///
/// Orbit's output directory persists across runs, and the drive wipes the
/// stale snapshot first -- the same correctness-before-incrementality
/// stance as the work libraries. Probed honestly: the wipe is symmetry
/// here, not load-bearing -- a failed compile exits 1 before the sim face
/// can run anything, so a stale snapshot can never be run by this drive;
/// it exists so every drive starts from a known-clean clock.
pub const SNAPSHOT: &str = "orbit-snapshot.vvp";

/// The error for blueprint filesets the Verilog drives do not carry.
///
/// Unlike the VHDL filter -- which nvc borrows from ghdl, message and
/// all -- this one is shared by TWO drives (iverilog and verilator), so
/// the message names the filesets, not one drive: a verilator user reading
/// "the iverilog drive carries..." would be misdirected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// Every non-Verilog entry, in blueprint order.
    pub entries: Vec<ghdl::Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the Verilog drives carry the VLOG and SYSV filesets only; not yet carried:"
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

/// The Verilog files of the blueprint, in blueprint order.
///
/// Every non-Verilog entry -- `VHDL`, a custom fileset -- is rejected
/// loudly: iverilog and verilator are Verilog/SV-native, and a silent
/// skip would leave files the user believes were compiled. The blueprint
/// order is carried but not load-bearing: iverilog's single compile is
/// order-free (probed live on Icarus Verilog 12.0, 2026-11-07).
pub fn verilog_only(entries: &[ghdl::Entry]) -> Result<Vec<std::path::PathBuf>, NotCarried> {
    let foreign: Vec<ghdl::Entry> = entries
        .iter()
        .filter(|entry| entry.fileset != "VLOG" && entry.fileset != "SYSV")
        .cloned()
        .collect();
    if foreign.is_empty() {
        Ok(entries.iter().map(|entry| entry.path.clone()).collect())
    } else {
        Err(NotCarried { entries: foreign })
    }
}

/// The record of the snapshot's run: what vvp exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The roots that ran, joined with `+`: iverilog compiles every `-s`
    /// root into ONE snapshot and vvp runs that snapshot once, so one run
    /// can carry several roots (`tb+tb2`).
    pub top: String,
    /// vvp's exit code, recorded but secondary: it never flips the
    /// verdict -- and it cannot, for the probed `$error` call exits 0
    /// (the judge's reason to exist here). A `-1` records a run that
    /// died by signal rather than exiting.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed iverilog failure face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Iverilog, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face
    /// in the log AND vvp exited 0.
    ///
    /// Either witness convicts. The face catches what the exit cannot
    /// see: vvp exits 0 on a `$error` call (probed live on Icarus Verilog
    /// 12.0, 2026-11-07 -- it prints the `ERROR:` face and continues),
    /// and there the face is the only witness. The exit catches what the
    /// log cannot name: a killed or crashed runtime can exit nonzero
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
    NotCarried(NotCarried),
    /// iverilog or vvp could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale snapshot could not be wiped: usage-class, exit 2.
    Library(String),
    /// The compile failed: findings, exit 1 (iverilog's own exits are
    /// honest there: 2 on a parse error, 1 on an unknown `-s` root,
    /// probed live).
    Compile {
        /// The stderr of the failed compile, verbatim.
        stderr: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings iverilog or the judge
    /// reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_) => 2,
            DriveError::Compile { .. } | DriveError::Findings(_) => 1,
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
            DriveError::Compile { .. } => write!(f, "the iverilog compile failed"),
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
            "orbit-contracts: iverilog: note: $ORBIT_BLUEPRINT_PLAN is {plan:?}; the drive \
             carries the TSV plan only, reading the blueprint as TSV lines"
        )),
        _ => None,
    }
}

/// Drive the iverilog target end to end: blueprint, one compile, and --
/// on the sim face -- one judged `vvp` run.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths, the
/// snapshot, and any waves the testbench writes land there. All progress
/// and diagnostics are printed here; the returned error only picks the
/// exit code. The mode picks how far the drive goes: `Build` stops after
/// the compile (no run, no log, nothing judged); `Sim` runs the snapshot
/// and judges it.
///
/// There is no deferral loop and no per-top loop: iverilog's compile is
/// order-free (probed live on Icarus Verilog 12.0, 2026-11-07), so the
/// files compile in blueprint order in ONE invocation; every requested
/// top rides a `-s` root flag on that compile, and vvp runs the single
/// snapshot once with all the roots.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    // iverilog must be spawnable before anything else, so a missing
    // binary is a loud usage error instead of a silent compile failure.
    capture(IVERILOG_BIN, Command::new(IVERILOG_BIN).arg("--version"))
        .map_err(DriveError::Spawn)?;

    // Run-scoped state: see `SNAPSHOT`. Every drive starts from a
    // known-clean clock (probed honestly: the wipe is symmetry here -- a
    // failed compile exits before the run could touch a stale snapshot).
    if let Err(e) = fs::remove_file(SNAPSHOT) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale snapshot {SNAPSHOT}: {e}");
            eprintln!("orbit-contracts: iverilog: error: {message}");
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
        eprintln!("orbit-contracts: iverilog: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = ghdl::parse_blueprint(&text).map_err(|e| {
        let e = ghdl::BlueprintError::Parse(e);
        eprintln!("orbit-contracts: iverilog: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let files = verilog_only(&entries).map_err(|e| {
        eprintln!("orbit-contracts: iverilog: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-contracts: iverilog: {label}: compiling {} Verilog file(s) into one snapshot \
         (the compile is order-free, probed live 2026-11-07)",
        files.len()
    );
    // The extras ride the drive's last step verbatim: the vvp argv on the
    // sim face, the compile argv on the build face (there is no run to
    // carry them, and silently dropping a user argument would be a
    // degradation). An option the compile does not speak is iverilog's
    // own loud rejection, surfaced as an exit-1 compile failure.
    let build_extra = if mode == Mode::Build {
        Some(request.extra.as_slice())
    } else {
        None
    };
    compile_once(&files, &request.tops, build_extra).map_err(|stderr| {
        eprint_block(&stderr);
        eprintln!("orbit-contracts: iverilog: error: the iverilog compile failed");
        DriveError::Compile { stderr }
    })?;
    println!("orbit-contracts: iverilog: {label}: {SNAPSHOT} compiled");

    if mode == Mode::Sim {
        let roots = request.tops.join("+");
        let outcome = run_snapshot(&roots, &request.extra, label)?;
        if outcome.clean() {
            println!("orbit-contracts: iverilog: {label}: {roots}: pass");
            return Ok(());
        }
        if outcome.has_face() {
            println!(
                "orbit-contracts: iverilog: {label}: {roots}: fail (the log carries a probed \
                 iverilog failure face)"
            );
        } else {
            println!(
                "orbit-contracts: iverilog: {label}: {roots}: fail (vvp exited {} without a \
                 judged face)",
                outcome.exit
            );
        }
        eprintln!("orbit-contracts: iverilog: error: 1 run(s) carried a finding");
        return Err(DriveError::Findings(vec![outcome]));
    }

    Ok(())
}

/// The one compile: `iverilog -g2012 -s <top>... -o <snapshot> <files>
/// <extras>`, every requested top a `-s` root. iverilog validates each
/// root name against the source at compile time (exit 1, probed), and its
/// compile exits are honest (exit 2 on a parse error), so the build face
/// needs no judge. The extras ride here only on the build face -- there
/// is no run to carry them, and silently dropping a user argument would
/// be a degradation; an option the compile does not speak is iverilog's
/// own loud rejection.
fn compile_once(
    files: &[std::path::PathBuf],
    tops: &[String],
    extra: Option<&[String]>,
) -> Result<(), String> {
    let mut compile = Command::new(IVERILOG_BIN);
    compile.arg(GEN);
    for top in tops {
        compile.arg("-s").arg(top);
    }
    compile.arg("-o").arg(SNAPSHOT).args(files);
    if let Some(extra) = extra {
        compile.args(extra);
    }
    let output = capture(IVERILOG_BIN, &mut compile)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(lossy(&output.stderr))
    }
}

/// Run the snapshot once and capture its log: `vvp <snapshot> <extras>`.
/// The probed failure faces (`FATAL:`, `ERROR:`) land on stdout; the log
/// joins both streams, so it reads either way.
fn run_snapshot(roots: &str, extra: &[String], label: &str) -> Result<RunOutcome, DriveError> {
    println!("orbit-contracts: iverilog: {label}: running {roots}");
    let mut run = Command::new(VVP_BIN);
    run.arg(SNAPSHOT).args(extra);
    let output = capture(VVP_BIN, &mut run).map_err(DriveError::Spawn)?;
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
        top: roots.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

/// Spawn a command and capture its output, mapping a spawn failure to the
/// usage-class error.
fn capture(bin: &str, command: &mut Command) -> Result<Output, String> {
    command.output().map_err(|e| format!("{bin}: {e}"))
}

/// The captured bytes as text: iverilog and vvp write UTF-8, and the
/// verdict must not panic on anything less.
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

    // The probed $fatal fixture (vvp 12.0, 2026-11-07): the run stops,
    // exit 1. Byte-for-byte as recorded live; the faces land on stdout.
    const FATAL_LOG: &str = "\
FATAL: tb_fatal.v:4: the design is wrong
       Time: 1  Scope: tb_fatal
";

    // The probed $error fixture (vvp 12.0, 2026-11-07): vvp prints the
    // face, CONTINUES the simulation, and EXITS 0 -- the same hole bare
    // ghdl has, and the reason the judge is this drive's load-bearing
    // gate. The face convicts on its own; an exit of 0 must never pass it.
    const ERROR_LOG: &str = "\
ERROR: tb_error.v:4: check failed
       Time: 1  Scope: tb_error
simulation continued
tb_error.v:6: $finish called at 1 (1s)
";

    // The probed gentle faces (`WARNING:`, probed on a continuing run
    // that exits 0) plus a `$stop`'s `** VVP Stop(0) **` banner (also
    // exit 0): continuations and legitimate stops, not failures.
    const WARNING_LOG: &str = "\
WARNING: tb_warn.v:4: check flagged
         Time: 1  Scope: tb_warn
all quiet
tb_warn.v:6: $finish called at 1 (1s)
** VVP Stop(0) **
";

    #[test]
    fn a_fatal_run_is_a_finding_on_both_witnesses() {
        // The probed stop: face AND exit 1. Either witness convicts; here
        // both do.
        let outcome = RunOutcome {
            top: "tb_fatal".to_string(),
            exit: 1,
            log: FATAL_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn an_error_run_convicts_on_the_face_alone() {
        // vvp continues past a $error call and exits 0 anyway, probed
        // live on vvp 12.0 (2026-11-07) -- the same hole bare ghdl has.
        // Pin the face witness on its own: the exit of 0 must not pass
        // this run.
        let outcome = RunOutcome {
            top: "tb_error".to_string(),
            exit: 0,
            log: ERROR_LOG.to_string(),
        };
        assert_eq!(outcome.exit, 0);
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a killed or crashed runtime can exit nonzero
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
    fn a_signal_death_is_a_finding_on_its_minus_one_exit() {
        // A run killed by a signal has no exit code; the drive records
        // -1 (see `RunOutcome.exit`), and -1 convicts.
        let outcome = RunOutcome {
            top: "tb_sig".to_string(),
            exit: -1,
            log: String::new(),
        };
        assert!(!outcome.clean());
    }

    #[test]
    fn a_warning_run_passes_the_gate() {
        // The probed gentle faces (`WARNING:`, `** VVP Stop(0) **`) on
        // continuing runs that exit 0: continuations and legitimate
        // stops, not failures.
        let outcome = RunOutcome {
            top: "tb_warn".to_string(),
            exit: 0,
            log: WARNING_LOG.to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn drive_errors_map_to_honest_exit_codes() {
        assert_eq!(
            DriveError::Blueprint(ghdl::BlueprintError::Read {
                path: std::path::Path::new("nope").to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "nope"),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Blueprint(ghdl::BlueprintError::Parse(ghdl::MalformedLine {
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
            DriveError::Spawn("iverilog: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Compile {
                stderr: String::new()
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
        assert!(note.starts_with("orbit-contracts: iverilog: note:"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Compile {
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "the iverilog compile failed");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
    }
}
