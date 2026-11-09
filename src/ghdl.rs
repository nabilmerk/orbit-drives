//! The ghdl target drive: the orbit blueprint in, a judged verdict out.
//!
//! Two orbit targets share this drive, split by [`Mode`]: `ghdl-build`
//! (analyze + elaborate per top, exit honest, no simulation, nothing
//! judged) and `ghdl` (the same drive run to the end: elaborate, run,
//! judge). The drive reads the blueprint orbit wrote for the target (the
//! TSV plan: one `fileset<TAB>library<TAB>filepath` line per collected
//! file), analyzes the VHDL entries in blueprint order with the deferral
//! loop -- each file into the library its own row declares
//! (`--work=<entry.library>`, the §25 multi-library face, probed live on
//! ghdl 6.0.0) -- and elaborates every requested top; the sim face then
//! runs each top and judges the run log against ghdl's probed failure
//! vocabulary. ghdl carries no search-path flag and needs none: a
//! consumer's `library <other>;` clause resolves against the sibling
//! `<other>-obj93.cf` file in the same working directory, automatically
//! -- ghdl's library search is the cwd alone (probed: move the sibling
//! into a subdirectory and the resolution dies). The elaborate and run
//! argv pin the top's library, because ghdl's bare-top commands search
//! the work library only. The judge -- not ghdl's exit code -- is the
//! sim face's gate: ghdl's exit is honest for a `severity failure` stop
//! but lies for a violated `severity error` check (exit 0), and the face
//! in the log is the only witness. The build face needs no judge:
//! `ghdl -a` and `ghdl -e` exit nonzero on a broken design (probed
//! live), so the exit is honest there.
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
    parse_blueprint, plan_note, vhdl_entries, BlueprintError, Drive, Entry, Mode, NotCarried,
};
use crate::contracts::{self, Tool};
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

/// The ghdl binary the drive spawns, resolved on PATH. The orbit target
/// contract carries no binary-path field.
pub const GHDL_BIN: &str = "ghdl";

/// The simulation option that makes a violated `severity error` stop the
/// run, probed live on ghdl 6.0.0 (2026-11-07). It is a SIMULATION option:
/// legal only after the toplevel unit -- placed before it, ghdl answers
/// `ghdl:error: unknown command option '--assert-level=error'`. It is
/// defense in depth only: the judge stays the gate, because the face must
/// land in the log either way. The language standard, by contrast, is NOT
/// pinned: the drive rides ghdl's own default (VHDL-93 on ghdl 6.0.0,
/// probed) -- the hardcoded `--std=08` pin walled 93-written corpora
/// (the GRLIB stdio `redeclaration of procedure "HRead"` homograph face,
/// probed live 2026-11-08: the file compiles clean under the default and
/// dies under `--std=08`), while GRLIB's own ghdl flow passes no std flag
/// at all.
pub const ASSERT_LEVEL: &str = "--assert-level=error";

/// The env override for the analysis flags every spawn carries
/// (`ORBIT_DRIVES_GHDL_FLAGS`): the value is whitespace-split and rides
/// each spawn's argv verbatim, right after the command. Unset or empty:
/// nothing rides.
///
/// WHY the drive carries this face: vendor simulation libraries pin
/// their own analysis flags, and ghdl re-checks flag consistency at
/// every LOAD of an analyzed unit -- a unit compiled under `-fsynopsys`
/// reparses from source on a flagless load and dies on the very face
/// the flag exists to allow (`use of synopsys package "std_logic_signed"
/// needs the -fsynopsys option`, probed live on ghdl 6.0.0, 2026-11-08,
/// the Xilinx unisim tree -- DCM, BUFGMUX and the RAM wrappers all pin
/// the Synopsys packages). The probed corpus case: the GRLIB leon3
/// zedboard workspace against Vivado's unisim -- `orbit-drives ghdl`
/// with `ORBIT_DRIVES_GHDL_FLAGS="-fpsl -frelaxed -fsynopsys
/// -fexplicit"` and the unisim tree pre-analyzed under the same flags
/// (see the workspace's lift record) carries the whole design.
/// Upstream there is no config or target flag face at all (chaseruskin/
/// orbit#25's class) -- until one lands, the env is the drive's honest
/// interim: the user names the corpus's flags, the drive carries them
/// on every spawn.
pub const FLAGS_ENV: &str = "ORBIT_DRIVES_GHDL_FLAGS";

/// The resolver behind the flag ride, pure so the tests can pin both
/// arms without touching the process env. Whitespace-split -- ghdl's
/// own multi-flag spellings ride as separate argv entries.
fn flags_from(value: Option<&str>) -> Vec<String> {
    match value {
        Some(v) => v.split_whitespace().map(str::to_string).collect(),
        None => Vec::new(),
    }
}

fn extra_flags() -> Vec<String> {
    flags_from(std::env::var(FLAGS_ENV).ok().as_deref())
}

/// The env override for the run-time assertion level
/// (`ORBIT_DRIVES_GHDL_ASSERT_LEVEL`): the value rides as
/// `--assert-level=<value>` in the run argv's post-top position (ghdl
/// rejects the option anywhere earlier; probed live). Unset or empty:
/// the default pin, `error`.
///
/// WHY the drive carries this face: the pin makes ghdl's lying exit
/// honest for typical corpora -- ghdl exits 0 on violated severity
/// error checks, so the drive raises the level to make ANY error
/// assertion halt the sim and convict the exit. But the LEVEL is a
/// corpus property: a corpus whose tolerated infrastructure notes are
/// severity-error assertions needs the level at `failure` -- the probed
/// case (2026-11-08, the GRLIB leon3 zedboard systest): the AHB-to-AXI
/// bridge prints a severity-error note at init for a translation path
/// the design never exercises, and the level-error pin halted the sim
/// at 0 ms before the systest ran. ghdl's own duplicate-option
/// precedence is last-wins (probed: `--assert-level=failure
/// --assert-level=error` halts at the error), so this face cannot ride
/// the earlier [`FLAGS_ENV`] position -- it owns its slot.
pub const ASSERT_LEVEL_ENV: &str = "ORBIT_DRIVES_GHDL_ASSERT_LEVEL";

/// The resolver behind the assert-level override, pure so the tests can
/// pin both arms without touching the process env.
fn assert_level_from(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => format!("--assert-level={v}"),
        _ => ASSERT_LEVEL.to_string(),
    }
}

fn assert_level_arg() -> String {
    assert_level_from(std::env::var(ASSERT_LEVEL_ENV).ok().as_deref())
}

/// Whether a file name in the working directory is one of ghdl's library
/// state files: the `<library>-obj<STD>.cf` family -- ghdl tags the file
/// with the standard it analyzed under (`-obj08` under `--std=08`,
/// `-obj93` under the tool default the drive now rides, probed live on
/// ghdl 6.0.0, 2026-11-08), one file per analyzed library,
/// `work-obj93.cf` beside `grlib-obj93.cf` on a multi-library blueprint.
/// It is ghdl's clock, one clock per library.
///
/// Probed live on ghdl 6.0.0 (2026-11-07): the library name lands
/// LOWERCASED in the file name -- `--work=MyLib` writes `mylib-obj93.cf`
/// -- while the blueprint's `library MyLib;` clauses still resolve
/// against it, so the drive's wipe never needs the name's case.
///
/// Orbit's output directory persists across runs, and a stale library
/// file turns this drive's own re-analysis against itself: in the
/// consumer-first order the deferral loop exists for, run 2 re-analyzes
/// the consumer BEFORE its provider -- the provider's re-analysis then
/// obsoletes the consumer's units, and `ghdl -e` refuses the unchanged
/// design (`error: architecture "rtl" of "top" is obsoleted by entity
/// "gate"`, probed live on ghdl 6.0.0, 2026-11-07). Correctness before
/// incrementality in v1: every drive starts from empty libraries -- and
/// the wipe takes EVERY `<library>-obj<STD>.cf` in the working directory
/// -- the whole FAMILY, every std tag -- not just work's, because a
/// library a PREVIOUS blueprint analyzed but this one no longer names
/// must die too, or a `library <other>;` clause would silently resolve
/// against its stale units (the same stance as the nvc drive's marker
/// scan). The family key is what keeps the no-pin face honest across a
/// std drift: a `-obj08.cf` left by a pre-ruling drive is ghdl's state
/// all the same and dies with the rest.
/// ghdl's default work library name (`work`), and the drive's last resort
/// when it must name the library that holds the top: see [`top_library`].
pub const DEFAULT_LIBRARY: &str = "work";

/// The env var orbit sets on every spawned target
/// (`ORBIT_PROJECT_LIBRARY`): the working project's HDL library name --
/// the same name orbit's planner writes into the blueprint's library
/// column for the project's own files, which is where the designated
/// testbench lives. The planner's voice, not the drive's guess: orbit
/// defaults a project's library to the project's NAME, so under orbit
/// this is generally NOT ghdl's `work`.
pub const PROJECT_LIBRARY_ENV: &str = "ORBIT_PROJECT_LIBRARY";

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
    /// The stale library state files could not be wiped: usage-class,
    /// exit 2.
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

    // Run-scoped library state: see the `<library>-obj<STD>.cf` family
    // (the `is_ghdl_library_file` scan). Every drive
    // starts from empty libraries, so a second run in the same persistent
    // output directory cannot obsolete its own design, and no consumer
    // resolves a library this blueprint no longer analyzes.
    wipe_libraries(Path::new(".")).map_err(|message| {
        eprintln!("orbit-drives: ghdl: error: {message}");
        DriveError::Library(message)
    })?;

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
    let carried = vhdl_entries(&entries).map_err(|e| {
        eprintln!("orbit-drives: ghdl: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-drives: ghdl: {label}: analyzing {} VHDL file(s) in blueprint order",
        carried.len()
    );
    let paths: Vec<PathBuf> = carried.iter().map(|entry| entry.path.clone()).collect();
    let report = analyze_with_deferral(&paths, |file| {
        // Each file analyzes into the library its own blueprint row
        // declares -- the §25 face, the column consumed verbatim. The
        // deferral loop hands back exactly the paths it was given, and
        // those came from these entries: the lookup cannot miss.
        let library = carried
            .iter()
            .find(|entry| entry.path == file)
            .expect("the deferral loop hands back the blueprint's own paths")
            .library
            .clone();
        analyze_once(file, &library)
    });
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

    // The library that holds the tops: see `top_library`. Resolved once
    // -- it does not vary per top.
    let top_library = top_library(std::env::var(PROJECT_LIBRARY_ENV).ok().as_deref(), &carried);

    let mut findings = Vec::new();
    for top in &request.tops {
        println!("orbit-drives: ghdl: {label}: elaborating {top}");
        let mut elaborate = Command::new(GHDL_BIN);
        // The top's library rides every command (`--work=<library>`):
        // ghdl's bare-top commands search the work library only, so the
        // pin is load-bearing -- see `top_library`.
        elaborate.args(command_args("-e", &top_library)).arg(top);
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
                let outcome = run_top(top, &top_library, &request.extra, label)?;
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

/// Run one elaborated top and capture its log: `ghdl -r
/// --work=<the top's library> <top> --assert-level=error <extras>` (no
/// std pin: ghdl's own default rides, see [`ASSERT_LEVEL`]).
fn run_top(
    top: &str,
    library: &str,
    extra: &[String],
    label: &str,
) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: ghdl: {label}: running {top}");
    // `--assert-level=error` rides AFTER the top unit: it is a
    // simulation option, and ghdl rejects it anywhere earlier.
    let mut run = Command::new(GHDL_BIN);
    run.args(command_args("-r", library))
        .arg(top)
        .arg(assert_level_arg())
        .args(extra);
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
/// stderr when it did not. The command flag leads ghdl's command-first
/// argv (`-a`, then the global pins), and the work library is what makes
/// the attempt multi-library: the file lands in the library file of the
/// library its blueprint row declares.
fn analyze_once(file: &Path, library: &str) -> Result<(), String> {
    let mut command = Command::new(GHDL_BIN);
    command.args(command_args("-a", library)).arg(file);
    match capture(GHDL_BIN, &mut command) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(lossy(&output.stderr)),
        Err(message) => Err(message),
    }
}

/// The leading block every ghdl argv carries: the command flag first
/// (`-a` / `-e` / `-r` -- ghdl's argv is command-first), then the work
/// library pin. NO std flag: the drive rides ghdl's own default
/// (VHDL-93 on ghdl 6.0.0, probed) -- the hardcoded `--std=08` pin walled
/// 93-written corpora (the GRLIB stdio homograph face, probed live
/// 2026-11-08: `redeclaration of procedure "HRead"`, exit 1, on a file
/// the default compiles clean), while GRLIB's own ghdl flow passes no
/// std flag at all.
///
/// The `--work` value is consumed verbatim from the blueprint's library
/// column on the analyze face (the §25 ruling: the library name is the
/// project's declared name, orbit's planner is its source of truth); on
/// the elaborate and run faces it is the top's library, resolved by
/// [`top_library`].
fn command_args(command: &str, library: &str) -> Vec<String> {
    let mut args = vec![command.to_string()];
    args.extend(extra_flags());
    args.push(format!("--work={library}"));
    args
}

/// The library that holds the tops -- the `--work` pin on the elaborate
/// and run argv.
///
/// ghdl's bare-top commands search the WORK library ONLY: `-e tb` with
/// the top analyzed into any other library answers `error: cannot find
/// entity or configuration tb` (exit 1, probed live on ghdl 6.0.0,
/// 2026-11-07, even with that library's sibling file in the cwd), so the
/// drive must name the top's library on every elaboration and run.
///
/// The resolution, loudest voice first:
///
/// 1. `$ORBIT_PROJECT_LIBRARY` -- orbit's own voice. The planner sets it
///    on every spawned target to the working project's HDL library, and
///    the designated testbench is a working-project file, so under orbit
///    this is always the right answer (and generally NOT `work`: orbit
///    defaults a project's library to the project's name). An empty
///    value counts as absent -- the `$ORBIT_TB_NAME` stance.
/// 2. The blueprint's single carried library -- direct invocation
///    without orbit: when every VHDL row declares the same library, the
///    top is necessarily in it.
/// 3. ghdl's default [`DEFAULT_LIBRARY`] -- the leftover case (several
///    libraries, no env): a guess, but a SAFE one. The drive wipes every
///    library file first, so `work` cannot hold a stale unit; a wrong
///    guess dies loudly on ghdl's own elaboration refusal (`error:
///    cannot find entity or configuration <top>`) -- never a silent
///    degradation.
fn top_library(project_library: Option<&str>, entries: &[Entry]) -> String {
    match project_library {
        Some(library) if !library.is_empty() => library.to_string(),
        _ => {
            let mut distinct: Vec<&str> =
                entries.iter().map(|entry| entry.library.as_str()).collect();
            distinct.sort_unstable();
            distinct.dedup();
            match distinct.as_slice() {
                [only] => (*only).to_string(),
                _ => DEFAULT_LIBRARY.to_string(),
            }
        }
    }
}

/// Whether `name` is one of ghdl's library state files: the
/// `<library>-obj<STD>.cf` family -- a `.cf` file carrying ghdl's `-obj`
/// tag, whatever std wrote it (the drive pins no std, so the family --
/// every tag -- is its clock; a `-obj08.cf` left by a pre-ruling drive
/// dies beside a fresh `-obj93.cf`).
fn is_ghdl_library_file(name: &str) -> bool {
    name.ends_with(".cf") && name.contains("-obj")
}

/// Remove every ghdl library state file under `dir` -- the drive's output
/// directory, passed as the cwd (`.`). A file is ghdl's when its name
/// is one of ghdl's library state files (see `is_ghdl_library_file`): one
/// analyzed library, the name lowercased by ghdl (probed), so the suffix
/// alone is the honest key. The scan -- not the blueprint's library list
/// -- is what makes the wipe honest: a library a PREVIOUS drive analyzed
/// but this blueprint no longer names must die too, or a
/// `library <other>;` clause would silently resolve against its stale
/// units. A directory named like a library file is not ghdl's state
/// (ghdl's state is files), and neither is a file of another standard
/// of another std tag (`<library>-obj05.cf`): the whole family is the
/// drive's state -- a tag a previous drive (or std) wrote dies with the
/// rest, so no stale file can serve units to a clause.
fn wipe_libraries(dir: &Path) -> Result<(), String> {
    let unreadable = |e: std::io::Error| {
        format!(
            "cannot wipe the stale work libraries: cannot read {}: {e}",
            dir.display()
        )
    };
    let mut victims = Vec::new();
    for entry in fs::read_dir(dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if path.is_file() && is_ghdl_library_file(name) {
                victims.push(path);
            }
        }
    }
    for path in victims {
        if let Err(e) = fs::remove_file(&path) {
            return Err(format!(
                "cannot wipe the stale library file {}: {e}",
                path.display()
            ));
        }
    }
    Ok(())
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
            DriveError::Library("cannot wipe".to_string()).exit_code(),
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

    #[test]
    fn every_argv_leads_with_the_command_then_the_global_pins() {
        // The probed command-first shape: the command flag leads, the
        // work library pin follows, and NO std flag rides -- the drive
        // takes ghdl's own default (VHDL-93 on ghdl 6.0.0, probed): the
        // hardcoded `--std=08` walled 93-written corpora (the GRLIB
        // stdio homograph face, probed live 2026-11-08: `redeclaration of
        // procedure "HRead"`, exit 1, on a file the default compiles
        // clean).
        assert_eq!(
            command_args("-a", "grlib"),
            vec!["-a".to_string(), "--work=grlib".to_string()]
        );
        assert_eq!(
            command_args("-e", "work"),
            vec!["-e".to_string(), "--work=work".to_string()]
        );
        // The library name rides verbatim -- orbit's planner owns it (the
        // §25 ruling); the drive never normalizes it to ghdl's default.
        assert_eq!(command_args("-r", "MyLib")[1], "--work=MyLib");
        assert_eq!(command_args("-r", "work")[1], "--work=work");
    }

    #[test]
    fn the_flags_env_override_splits_and_the_default_survives() {
        // The vendor-flag face (see FLAGS_ENV): whitespace-split entries
        // ride as separate argv tokens; unset, empty, and blank ride
        // nothing at all. Pure resolver -- no process-env mutation.
        assert_eq!(
            flags_from(Some("-fsynopsys -fexplicit")),
            vec!["-fsynopsys".to_string(), "-fexplicit".to_string()]
        );
        assert_eq!(
            flags_from(Some("  -frelaxed   -fpsl  ")),
            vec!["-frelaxed".to_string(), "-fpsl".to_string()]
        );
        assert_eq!(flags_from(None), Vec::<String>::new());
        assert_eq!(flags_from(Some("")), Vec::<String>::new());
        assert_eq!(flags_from(Some("   ")), Vec::<String>::new());
    }

    #[test]
    fn the_assert_level_env_override_replaces_the_pin_and_the_default_survives() {
        // The assert-level face (see ASSERT_LEVEL_ENV): a non-empty
        // override rides verbatim in the run argv's post-top slot;
        // unset, empty, and blank fall back to the default pin. Pure
        // resolver -- no process-env mutation.
        assert_eq!(
            assert_level_from(Some("failure")),
            "--assert-level=failure"
        );
        assert_eq!(
            assert_level_from(Some("  note  ")),
            "--assert-level=note"
        );
        assert_eq!(assert_level_from(None), ASSERT_LEVEL);
        assert_eq!(assert_level_from(Some("")), ASSERT_LEVEL);
        assert_eq!(assert_level_from(Some("   ")), ASSERT_LEVEL);
    }

    #[test]
    fn the_top_library_reads_orbits_voice_first() {
        let entries = parse_blueprint("VHDL\tgrlib\t./pkg.vhd\nVHDL\twork\t./tb.vhd\n").unwrap();
        // orbit's env is the loudest voice: it wins even where the
        // blueprint could answer on its own.
        assert_eq!(top_library(Some("mylib"), &entries), "mylib");
        assert_eq!(top_library(Some("work"), &entries), "work");
        // An empty value counts as absent -- the `$ORBIT_TB_NAME` stance
        // -- so the ladder falls through to the next voice.
        assert_eq!(top_library(Some(""), &entries), "work");
    }

    #[test]
    fn the_top_library_falls_through_the_ladder() {
        // No env, one carried library: the top is necessarily in it.
        let one = parse_blueprint("VHDL\tmylib\t./a.vhd\nVHDL\tmylib\t./b.vhd\n").unwrap();
        assert_eq!(top_library(None, &one), "mylib");
        // No env, several libraries: ghdl's default `work` -- a guess
        // that cannot pass silently, because the wiped `work` library
        // cannot hold a stale unit: a wrong guess dies on ghdl's own
        // elaboration refusal (`error: cannot find entity or
        // configuration tb`, probed).
        let many = parse_blueprint("VHDL\tgrlib\t./p.vhd\nVHDL\twork\t./tb.vhd\n").unwrap();
        assert_eq!(top_library(None, &many), "work");
        // An empty blueprint has no library to name: the default.
        assert_eq!(top_library(None, &[]), "work");
    }

    #[test]
    fn the_wipe_takes_every_library_file_and_nothing_else() {
        let dir = temp_dir("wipe-libraries");
        // The family, every std tag: the fresh `-obj93.cf` files the
        // no-pin drive writes AND a `-obj08.cf` left by a pre-ruling
        // drive -- ghdl's state is the whole family, because the drive
        // pins no std and a stale file of ANY tag could serve units.
        let work = dir.join("work-obj93.cf");
        let grlib = dir.join("grlib-obj93.cf");
        let lowercased = dir.join("mylib-obj93.cf");
        let legacy_std = dir.join("work-obj08.cf");
        let keep = dir.join("keep.txt");
        for file in [&work, &grlib, &lowercased, &legacy_std, &keep] {
            fs::write(file, "state").unwrap();
        }
        // A directory named like a library file is not ghdl's state:
        // ghdl's state is files.
        let imposter = dir.join("weird-obj08.cf");
        fs::create_dir_all(&imposter).unwrap();

        wipe_libraries(&dir).unwrap();

        assert!(!work.exists(), "the work library file must die");
        assert!(!grlib.exists(), "every library file must die");
        assert!(
            !lowercased.exists(),
            "the lowercased name ghdl writes must die (probed: --work=MyLib writes \
             mylib-obj93.cf under the default std)"
        );
        assert!(
            !legacy_std.exists(),
            "a pre-ruling std tag is ghdl's state all the same: the family scan takes \
             every -obj*.cf"
        );
        assert!(
            keep.exists(),
            "a file without the .cf/-obj shape is not ghdl's state"
        );
        assert!(
            imposter.exists(),
            "a directory named like a library file is not ghdl's state"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_wipe_surfaces_an_unreadable_directory_as_usage_class() {
        let missing = temp_dir("wipe-libraries-missing");
        fs::remove_dir_all(&missing).unwrap();
        let err = wipe_libraries(&missing).unwrap_err();
        assert!(
            err.starts_with("cannot wipe the stale work libraries"),
            "the wipe failure should name itself: {err}"
        );
        assert!(
            err.contains(&missing.display().to_string()),
            "the wipe failure should name the directory: {err}"
        );
    }
}
