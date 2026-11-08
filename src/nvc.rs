//! The nvc target drive: the same shapes as the ghdl drive, nvc's probed
//! faces -- and, like ghdl, a consumer of the blueprint's library
//! column.
//!
//! The drive reads the blueprint orbit wrote for the target (the TSV plan:
//! one `fileset<TAB>library<TAB>filepath` line per collected file),
//! analyzes the VHDL entries in blueprint order with the deferral loop --
//! each file into the library its own row declares (`--work=
//! <entry.library>`, the §25 multi-library face, probed live on nvc
//! 1.23.0) -- and elaborates every requested top; the sim face then runs
//! each top and judges the run log against nvc's probed failure
//! vocabulary. EVERY nvc spawn carries `-L .`: nvc creates one directory
//! per analyzed library in the output directory, and a consumer's
//! `library <other>;` clause resolves only when that directory is on the
//! search path. Two orbit targets share this drive, split by [`Mode`]:
//! `nvc-build` (analyze + elaborate per top, exit honest, no simulation,
//! nothing judged) and `nvc` (elaborate, run, judge).
//!
//! The TSV blueprint is orbit's plan format and the deferral loop is
//! order-independence machinery, not ghdl's: the blueprint machinery
//! lives in [`crate::blueprint`], and the deferral loop lives in
//! [`crate::ghdl`] with the first drive that carried it -- both drives
//! share them. Everything that is genuinely nvc's -- the argv shape, the
//! per-library state, the faces -- is probed and pinned here.
//!
//! The judge gates the sim face as second witness, not first: nvc's exits
//! probed honest across every failure face (a `severity error` run
//! continues but still exits 1, unlike bare ghdl's exit 0), and the face
//! in the log convicts regardless -- either witness convicts, everywhere.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::blueprint::{
    parse_blueprint, plan_note, vhdl_entries, BlueprintError, Drive, Entry, Mode, NotCarried,
};
use crate::contracts::{self, Tool};
use crate::ghdl::{self, AnalyzeReport};
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

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

/// The env override for that pin (`ORBIT_DRIVES_NVC_STD`): the value
/// rides every spawn as `--std=<value>` verbatim -- nvc's own revision
/// spellings (`1993`, `2008`, `2019`; an invalid spelling is nvc's own
/// loud rejection at the first analyze). Unset or empty: the default
/// pin.
///
/// WHY the drive carries this face: the tools' default revisions
/// DISAGREE, and a corpus pins its own. The probed case (2026-11-08,
/// the GRLIB leon3 zedboard workspace): the corpus is written for
/// VHDL-93 -- its stdlib's duplicate `HRead` declarations are legal
/// overloads under 93 and die under the homograph rule nvc enforces at
/// its 2008-era default -- and at `--std=1993` the whole corpus (391 of
/// 412 blueprint files, the rest the vendor-library wall) analyzes
/// clean through the drive's own deferral loop. Riding the tool default
/// is right only when the tool's default matches the corpus: ghdl's
/// default IS 93 (so its pin removal was the fix there); nvc's default
/// is 2008, so the ride keeps the wall. The real face is a per-project
/// std selector -- none exists anywhere in the stack today (no
/// manifest field, no target key, no command flag: upstream
/// chaseruskin/orbit#24) -- so until one lands, the env is the drive's
/// honest interim: the user names the corpus's revision, the drive
/// carries it on every spawn.
pub const STD_ENV: &str = "ORBIT_DRIVES_NVC_STD";

/// The resolver behind [`std_arg`], pure so the tests can pin both
/// arms without touching the process env.
fn std_flag_from(value: Option<&str>) -> String {
    match value {
        Some(v) if !v.trim().is_empty() => format!("--std={}", v.trim()),
        _ => STD.to_string(),
    }
}

/// The `--std=` flag every spawn leads with: the env override when set,
/// the pinned default otherwise.
fn std_arg() -> String {
    std_flag_from(std::env::var(STD_ENV).ok().as_deref())
}

/// The env override for nvc's unified heap cap (`ORBIT_DRIVES_NVC_HEAP`):
/// the value rides every spawn's global block as the `-H <value>` pair,
/// verbatim -- nvc's own size spellings (`2G`, `512M`; an invalid
/// spelling is nvc's own loud rejection). Unset or empty: nothing rides
/// (nvc's own default cap).
///
/// WHY the drive carries this face: nvc caps the simulation heap by
/// default, and the cap is a DESIGN fact nothing else in the stack
/// carries. The probed case (2026-11-08, the GRLIB leon3 zedboard
/// systest): the design's memory models exhaust the default heap at
/// simulation init (`** Fatal: (init): out of memory attempting to
/// allocate 36864 byte object`). And `-H` is a GLOBAL option -- nvc
/// rejects it after the run command (`** Fatal: unrecognised run option
/// -H`, probed live) -- so the drive's last-step extras cannot carry it;
/// the global block is the only placement, and the value is the user's
/// design call.
pub const HEAP_ENV: &str = "ORBIT_DRIVES_NVC_HEAP";

/// The resolver behind the heap flag, pure so the tests can pin both
/// arms without touching the process env. `None` when unset/blank --
/// no heap pair rides at all.
fn heap_args_from(value: Option<&str>) -> Vec<String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(size) => vec!["-H".to_string(), size.to_string()],
        None => Vec::new(),
    }
}

fn heap_args() -> Vec<String> {
    heap_args_from(std::env::var(HEAP_ENV).ok().as_deref())
}

/// The library search path every nvc spawn carries: `-L .` -- the output
/// directory where nvc creates one directory per analyzed library. A
/// consumer's `library <other>;` clause resolves ONLY when the other
/// library's directory is on the search path: without `-L .` the path is
/// nvc's install dirs alone, and the analyze dies on `** Error: library
/// <OTHER> not found` above `** Fatal: required library <OTHER> not
/// found`, with the search-path note naming the `-L` fix (probed live on
/// nvc 1.23.0, 2026-11-07). Probed honestly: `-L .` is accepted on every
/// spawn -- analyze, elaborate, run -- including the first one, before
/// any library directory exists.
pub const SEARCH_PATH_ARGS: [&str; 2] = ["-L", "."];

/// nvc's own marker file inside every work library directory it creates
/// (`<lib>/_NVC_LIB`, carrying nvc's version string). nvc itself refuses
/// to recognize a directory as a library without it, and it is written
/// even when the analysis into the fresh library fails -- probed on nvc
/// 1.23.0 (2026-11-07): a syntax-broken analyze into `--work=freshlib`
/// and an option-error one (`--work=optfail -a --bogus`) both leave
/// `<lib>/_NVC_LIB` behind.
///
/// The drive's state wipe keys on this marker: every directory in the
/// output directory carrying it is one of nvc's per-library directories,
/// and every one of them dies with the drive start -- the same
/// correctness-before-incrementality stance as ghdl, extended to the
/// multi-library face. The scan -- not the blueprint's library list --
/// is what makes it honest: a library a PREVIOUS drive analyzed but this
/// blueprint no longer names must die too, or a `library <other>;`
/// clause would silently resolve against its stale units. Probed
/// honestly: nvc tolerates re-analysis in a stale library without the
/// obsoleted-design refusal (nvc 1.23.0, 2026-11-07), so the wipe is not
/// load-bearing against nvc itself the way ghdl's is -- it is
/// load-bearing against the drive's own multi-library resolution.
pub const LIBRARY_MARKER: &str = "_NVC_LIB";

/// nvc's default work library name (`work`), and the drive's last resort
/// when it must name the library that holds the top: see [`top_library`].
pub const DEFAULT_LIBRARY: &str = "work";

/// The std.standard `CHARACTER` enumeration literals (VHDL LRM, stable
/// since VHDL-87): the ASCII control positions carry their standard
/// names (NUL..DEL), and nvc spells the 128..159 positions C128..C159.
///
/// A work library named identically to any of these is UNUSABLE in nvc
/// (probed 1.23.0, 2026-11-08): nvc resolves a secondary unit's entity
/// indication (`architecture rtl of gate`) through the qualified
/// `<work>.<entity>` name, and the literal -- std.standard is visible
/// in every scope -- shadows the library symbol, so the analyze dies
/// with `** Error: name GATE not found in literal SUB` over
/// `= Note: library SUB is hidden`, every pass of the deferral loop.
/// Probed honestly: all 34 alphabetical names AND the whole C128..C159
/// range fail the same way; a package-only file analyzes fine (the
/// failure is the entity indication alone); the pre-made directory and
/// the `_NVC_LIB` marker change nothing; uppercase and underscore
/// spellings collide identically (nvc idents fold case and
/// underscores); and ghdl carries the same names cleanly
/// (`ghdl -a --work=sub` succeeds) -- the refusal is the nvc drive's
/// alone.
///
/// The drive refuses these names at the blueprint face, before any
/// analysis: burning the deferral loop on a name no retry can save
/// would misname the DESIGN as broken when only the library NAME is
/// unusable, and the fix lives in the manifest's `library` field.
pub const RESERVED_LIBRARY_NAMES: &[&str] = &[
    "NUL", "SOH", "STX", "ETX", "EOT", "ENQ", "ACK", "BEL", "BS", "HT", "LF", "VT", "FF", "CR",
    "SO", "SI", "DLE", "DC1", "DC2", "DC3", "DC4", "NAK", "SYN", "ETB", "CAN", "EM", "SUB", "ESC",
    "FSP", "GSP", "RSP", "USP", "DEL", "C128", "C129", "C130", "C131", "C132", "C133", "C134",
    "C135", "C136", "C137", "C138", "C139", "C140", "C141", "C142", "C143", "C144", "C145", "C146",
    "C147", "C148", "C149", "C150", "C151", "C152", "C153", "C154", "C155", "C156", "C157", "C158",
    "C159",
];

/// The reserved-name check: the std.standard literal a blueprint row's
/// library collides with, if any. The fold is nvc's own ident
/// comparison -- case-insensitive, underscore-insensitive -- so `sub`,
/// `SUB`, and `SU_B` all collide with the `SUB` literal.
pub fn reserved_library_name(library: &str) -> Option<&'static str> {
    let folded: String = library
        .to_uppercase()
        .chars()
        .filter(|c| *c != '_')
        .collect();
    RESERVED_LIBRARY_NAMES
        .iter()
        .find(|name| **name == folded)
        .copied()
}

/// The env var orbit sets on every spawned target
/// (`ORBIT_PROJECT_LIBRARY`): the working project's HDL library name --
/// the same name orbit's planner writes into the blueprint's library
/// column for the project's own files, which is where the designated
/// testbench lives. The planner's voice, not the drive's guess: orbit
/// defaults a project's library to the project's NAME, so under orbit
/// this is generally NOT nvc's `work`.
pub const PROJECT_LIBRARY_ENV: &str = "ORBIT_PROJECT_LIBRARY";

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
    Blueprint(BlueprintError),
    /// The blueprint carried filesets the drive does not speak yet:
    /// usage-class, exit 2.
    NotCarried(NotCarried),
    /// nvc could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale work libraries could not be wiped: usage-class, exit 2.
    Library(String),
    /// A blueprint row's library name collides with a std.standard
    /// CHARACTER literal nvc cannot carry into: usage-class, exit 2 --
    /// the fix is the manifest's `library` field, nothing the drive
    /// can do. See [`RESERVED_LIBRARY_NAMES`].
    ReservedLibrary {
        /// The colliding library name, verbatim from the blueprint.
        library: String,
        /// The std.standard literal that shadows it.
        literal: &'static str,
    },
    /// Files never analyzed after the deferral loop exhausted: findings,
    /// exit 1.
    Analyze(AnalyzeReport),
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
            | DriveError::Library(_)
            | DriveError::ReservedLibrary { .. } => 2,
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
            DriveError::ReservedLibrary { library, literal } => write!(
                f,
                "the library \"{library}\" collides with std.standard's CHARACTER literal \
                 {literal} -- nvc's entity-indication resolution shadows the work library \
                 with the literal (probed nvc 1.23.0), so no secondary unit can analyze \
                 into it. Rename the project's `library` field."
            ),
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

/// Drive the nvc target end to end: blueprint, analyze, elaborate, and --
/// on the sim face -- run and judge.
///
/// The request is the shared argv-level view every drive takes ([`Drive`];
/// the blueprint is orbit's format, the mode is the shared face split);
/// everything past it is nvc's own probed shape.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths, the
/// blueprint itself, and nvc's per-library directories land there. All
/// progress and diagnostics are printed here; the returned error only
/// picks the exit code. The mode picks how far the drive goes: `Build`
/// stops after the elaborations (no run, no log, nothing judged); `Sim`
/// runs each elaborated top and judges it.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    // nvc must be spawnable before anything else, so a missing binary is
    // a loud usage error instead of a deferral-loop residual.
    capture(NVC_BIN, Command::new(NVC_BIN).arg("--version")).map_err(DriveError::Spawn)?;

    // Run-scoped library state: see `LIBRARY_MARKER`. Every directory nvc
    // has ever made a work library in this output directory dies with the
    // drive start, so a second run starts from a known-clean clock and no
    // consumer resolves a library this blueprint no longer analyzes.
    wipe_libraries(Path::new(".")).map_err(DriveError::Library)?;

    if let Some(note) = plan_note("nvc", std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref()) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: nvc: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: nvc: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let carried = vhdl_entries(&entries).map_err(|e| {
        eprintln!("orbit-drives: nvc: error: {e}");
        DriveError::NotCarried(e)
    })?;

    // The reserved-library pre-flight: a `--work` name nvc cannot carry
    // would burn the whole deferral loop on `not found in literal <X>`
    // and misname the design as broken, when only the library NAME is
    // unusable. See `RESERVED_LIBRARY_NAMES` for the probed mechanism.
    for entry in &carried {
        if let Some(literal) = reserved_library_name(&entry.library) {
            let e = DriveError::ReservedLibrary {
                library: entry.library.clone(),
                literal,
            };
            eprintln!("orbit-drives: nvc: error: {e}");
            return Err(e);
        }
    }

    println!(
        "orbit-drives: nvc: {label}: analyzing {} VHDL file(s) in blueprint order",
        carried.len()
    );
    let paths: Vec<PathBuf> = carried.iter().map(|entry| entry.path.clone()).collect();
    let report = ghdl::analyze_with_deferral(&paths, |file| {
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
        "orbit-drives: nvc: {label}: {} file(s) analyzed in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if !report.residual.is_empty() {
        eprintln!(
            "orbit-drives: nvc: error: {} file(s) never analyzed after {} pass(es)",
            report.residual.len(),
            report.passes
        );
        for residual in &report.residual {
            eprintln!(
                "orbit-drives: nvc: error: analyze failed: {}",
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
        println!("orbit-drives: nvc: {label}: elaborating {top}");
        let mut elaborate = Command::new(NVC_BIN);
        // nvc's argv is option-first: the global block leads (`--std`,
        // `-L .`, `--work=<the top's library>`), the command follows
        // (`nvc --std=2008 -L . --work=work -e <top>`); after the command
        // the globals are rejected (`** Fatal: unrecognised elaboration
        // option`, probed live).
        elaborate.args(global_args(&top_library)).arg("-e").arg(top);
        // The extras ride the drive's last step verbatim: the run argv on
        // the sim face, the elaboration argv on the build face (there is
        // no run to carry them, and silently dropping a user argument
        // would be a degradation). An option the elaboration does not
        // speak is nvc's own loud rejection, surfaced as an exit-1
        // elaborate failure.
        if mode == Mode::Build {
            elaborate.args(&request.extra);
        }
        let output = capture(NVC_BIN, &mut elaborate).map_err(DriveError::Spawn)?;
        if !output.status.success() {
            let stderr = lossy(&output.stderr);
            eprint_block(&stderr);
            eprintln!("orbit-drives: nvc: error: nvc -e failed for {top}");
            return Err(DriveError::Elaborate {
                top: top.clone(),
                stderr,
            });
        }

        match mode {
            Mode::Build => println!("orbit-drives: nvc: {label}: {top}: elaborated"),
            Mode::Sim => {
                let outcome = run_top(top, &top_library, &request.extra, label)?;
                if outcome.clean() {
                    println!("orbit-drives: nvc: {label}: {top}: pass");
                } else if outcome.has_face() {
                    println!(
                        "orbit-drives: nvc: {label}: {top}: fail (the log carries a probed nvc failure face)"
                    );
                    findings.push(outcome);
                } else {
                    println!(
                        "orbit-drives: nvc: {label}: {top}: fail (nvc exited {} without a judged face)",
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
            "orbit-drives: nvc: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// Run one elaborated top and capture its log: `nvc --std=2008 -L .
/// --work=<the top's library> -r <top> <extras>`. The run options are
/// legal after the toplevel unit (probed clean with `--stop-time=1ns`);
/// there is no `--assert-level` analogue pinned for nvc because none is
/// needed -- nvc's run exit probed honest on every failure face, and the
/// judge gates the log regardless.
fn run_top(
    top: &str,
    library: &str,
    extra: &[String],
    label: &str,
) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: nvc: {label}: running {top}");
    let mut run = Command::new(NVC_BIN);
    run.args(global_args(library))
        .arg("-r")
        .arg(top)
        .args(extra);
    let output = capture(NVC_BIN, &mut run).map_err(DriveError::Spawn)?;
    let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
    print_block(&log);
    Ok(RunOutcome {
        top: top.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

/// One `nvc -a` attempt: `Ok` when nvc accepted the file, `Err` with the
/// stderr when it did not. The global block leads the command (`--std`,
/// `-L .`, `--work=<the row's library>`), per the probed option-first
/// shape -- and the work library is what makes the attempt multi-library:
/// each file lands in the directory of the library its blueprint row
/// declares.
fn analyze_once(file: &Path, library: &str) -> Result<(), String> {
    let mut command = Command::new(NVC_BIN);
    command.args(global_args(library)).arg("-a").arg(file);
    match capture(NVC_BIN, &mut command) {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(lossy(&output.stderr)),
        Err(message) => Err(message),
    }
}

/// The leading global block every nvc argv carries: the std pin, the cwd
/// library search path (`-L .`), and the work library pin
/// (`--work=<library>`). All three are GLOBAL options and nvc's argv is
/// option-first -- placed after the command they are rejected (`**
/// Fatal: unrecognised analysis option --std=2008`, probed) -- so the
/// block always leads: analyze, elaborate, and run alike.
///
/// The `--work` value is consumed verbatim from the blueprint's library
/// column on the analyze faces (the §25 ruling: the library name is the
/// project's declared name, orbit's planner is its source of truth); on
/// the elaborate and run faces it is the top's library, resolved by
/// [`top_library`].
fn global_args(library: &str) -> Vec<String> {
    let mut args = vec![std_arg()];
    args.extend(SEARCH_PATH_ARGS.iter().map(|arg| arg.to_string()));
    args.push(format!("--work={library}"));
    args.extend(heap_args());
    args
}

/// The library that holds the tops -- the `--work` pin on the elaborate
/// and run argv.
///
/// nvc's bare-top commands search the WORK library ONLY: `-e tb` with the
/// top analyzed into any other library answers `** Fatal: cannot find
/// unit WORK.TB in library WORK` (exit 1, probed live on nvc 1.23.0,
/// 2026-11-07, even with that library's directory on the search path), so
/// the drive must name the top's library on every elaboration and run.
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
/// 3. nvc's default [`DEFAULT_LIBRARY`] -- the leftover case (several
///    libraries, no env): a guess, but a SAFE one. The drive wipes every
///    library directory first, so `work` cannot hold a stale unit; a
///    wrong guess dies loudly on nvc's own `cannot find unit WORK.TB`
///    elaboration refusal -- never a silent degradation.
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

/// Remove every nvc work library directory under `dir` -- the drive's
/// output directory, passed as the cwd (`.`). A directory is nvc's when
/// it carries the [`LIBRARY_MARKER`] file: nvc's own recognition rule
/// (`_NVC_LIB`), and the only honest way to catch the libraries this
/// blueprint no longer names beside the ones it does -- a library left
/// by a previous drive's blueprint would otherwise stay resolvable and
/// serve stale units to a `library <other>;` clause.
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
        if path.is_dir() && path.join(LIBRARY_MARKER).exists() {
            victims.push(path);
        }
    }
    for path in victims {
        if let Err(e) = fs::remove_dir_all(&path) {
            return Err(format!(
                "cannot wipe the stale work library {}/: {e}",
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
            DriveError::Blueprint(BlueprintError::Read {
                path: Path::new("nope").to_path_buf(),
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
            DriveError::Spawn("nvc: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::ReservedLibrary {
                library: "sub".to_string(),
                literal: "SUB",
            }
            .exit_code(),
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
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("nvc", Some("tsv")).is_none());
        assert!(plan_note("nvc", None).is_none());
        let note = plan_note("nvc", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: nvc: note:"));
    }

    #[test]
    fn every_argv_leads_with_the_pinned_global_block() {
        // The probed option-first shape: the std pin, the cwd search
        // path, the work library pin -- all GLOBAL options, always
        // leading, never following a command (after it nvc answers
        // `** Fatal: unrecognised analysis option`).
        let args = global_args("grlib");
        assert!(
            args[0].starts_with("--std="),
            "every argv leads with the std pin: {:?}",
            args
        );
        assert_eq!(&args[1..], &["-L", ".", "--work=grlib"]);
        // The library name rides verbatim -- orbit's planner owns it (the
        // §25 ruling); the drive never normalizes it to nvc's default.
        assert_eq!(global_args("work")[3], "--work=work");
        assert_eq!(global_args("MyLib")[3], "--work=MyLib");
    }

    #[test]
    fn the_std_env_override_replaces_the_pin_and_the_default_survives() {
        // The interim std face (see STD_ENV): a non-empty override rides
        // verbatim; unset, empty, and blank fall back to the default
        // pin. Pure resolver -- no process-env mutation.
        assert_eq!(std_flag_from(Some("1993")), "--std=1993");
        assert_eq!(std_flag_from(Some("2019")), "--std=2019");
        assert_eq!(std_flag_from(Some("  1993  ")), "--std=1993");
        assert_eq!(std_flag_from(None), STD);
        assert_eq!(std_flag_from(Some("")), STD);
        assert_eq!(std_flag_from(Some("   ")), STD);
    }

    #[test]
    fn the_heap_env_override_rides_the_global_block_as_a_pair() {
        // The interim heap face (see HEAP_ENV): a non-empty size rides
        // as the two-token `-H <size>` pair (nvc's getopt shape -- one
        // merged token is rejected); unset, empty, and blank ride
        // nothing at all. Pure resolver -- no process-env mutation.
        assert_eq!(heap_args_from(Some("2G")), vec!["-H", "2G"]);
        assert_eq!(heap_args_from(Some(" 512M ")), vec!["-H", "512M"]);
        assert_eq!(heap_args_from(None), Vec::<String>::new());
        assert_eq!(heap_args_from(Some("")), Vec::<String>::new());
        assert_eq!(heap_args_from(Some("   ")), Vec::<String>::new());
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
        // No env, several libraries: nvc's default `work` -- a guess that
        // cannot pass silently, because the wiped `work` library cannot
        // hold a stale unit: a wrong guess dies on nvc's own elaboration
        // refusal (`** Fatal: cannot find unit WORK.TB in library WORK`).
        let many = parse_blueprint("VHDL\tgrlib\t./p.vhd\nVHDL\twork\t./tb.vhd\n").unwrap();
        assert_eq!(top_library(None, &many), "work");
        // An empty blueprint has no library to name: the default.
        assert_eq!(top_library(None, &[]), "work");
    }

    #[test]
    fn the_wipe_takes_every_marked_library_and_nothing_else() {
        let dir =
            std::env::temp_dir().join(format!("orbit-drives-nvc-wipe-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let grlib = dir.join("grlib");
        let work = dir.join("work");
        let plain = dir.join("plain");
        for library in [&grlib, &work, &plain] {
            fs::create_dir_all(library).unwrap();
        }
        fs::write(grlib.join(LIBRARY_MARKER), "nvc 1.23.0").unwrap();
        fs::write(work.join(LIBRARY_MARKER), "nvc 1.23.0").unwrap();
        // A file named like the marker at the top level is not a library
        // directory: it is not the drive's state to take.
        fs::write(dir.join(LIBRARY_MARKER), "stray").unwrap();
        // A directory without the marker is not nvc's state either -- a
        // library the marker does not recognize is not one nvc would
        // resolve either (its own rule).
        fs::write(plain.join("keep.txt"), "keep").unwrap();

        wipe_libraries(&dir).unwrap();

        assert!(!grlib.exists(), "the marked library must die");
        assert!(!work.exists(), "the marked library must die");
        assert!(
            plain.exists(),
            "a directory without the marker is not nvc's state"
        );
        assert!(
            dir.join(LIBRARY_MARKER).exists(),
            "a stray marker FILE is not a library directory"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_wipe_surfaces_an_unreadable_directory_as_usage_class() {
        let missing = std::env::temp_dir().join(format!(
            "orbit-drives-nvc-wipe-missing-{}",
            std::process::id()
        ));
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

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Elaborate {
            top: "tb".to_string(),
            stderr: String::new(),
        };
        assert_eq!(e.to_string(), "nvc -e failed for tb");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
        let e = DriveError::ReservedLibrary {
            library: "sub".to_string(),
            literal: "SUB",
        };
        assert_eq!(
            e.to_string(),
            "the library \"sub\" collides with std.standard's CHARACTER literal SUB -- \
             nvc's entity-indication resolution shadows the work library with the literal \
             (probed nvc 1.23.0), so no secondary unit can analyze into it. Rename the \
             project's `library` field."
        );
    }

    #[test]
    fn every_reserved_library_name_is_matched_through_the_ident_fold() {
        // The reserved set is exactly the std.standard CHARACTER
        // literals nvc shadows the work library with: every spelling
        // nvc's ident comparison equates to one of them collides --
        // lowercase, uppercase, and underscored alike.
        for name in RESERVED_LIBRARY_NAMES {
            assert_eq!(
                reserved_library_name(name),
                Some(*name),
                "the canonical spelling must hit"
            );
            assert_eq!(
                reserved_library_name(&name.to_lowercase()),
                Some(*name),
                "nvc idents fold case: {name} must hit lowercase"
            );
            let underscored = format!("{}_{}", name.to_lowercase(), "");
            assert_eq!(
                reserved_library_name(&underscored),
                Some(*name),
                "nvc idents fold underscores: {underscored} must hit"
            );
        }
    }

    #[test]
    fn ordinary_library_names_carry_no_reservation() {
        // The work name itself, the multi-library demo shapes, the
        // probe benches, and near-misses that must NOT fold into a
        // reserved name.
        for name in ["work", "demo2", "grlib", "gaisler", "techmap", "sub2", "suba", "c160"] {
            assert_eq!(
                reserved_library_name(name),
                None,
                "{name} must be free to analyze into"
            );
        }
    }

    #[test]
    fn a_reserved_library_refusal_names_the_literal_and_the_fix() {
        // The pre-flight face, end to end: the words carry the
        // colliding pair and the one fix that exists.
        let e = DriveError::ReservedLibrary {
            library: "sub".to_string(),
            literal: reserved_library_name("sub").expect("sub is the probed case"),
        };
        assert_eq!(e.exit_code(), 2);
        let words = e.to_string();
        assert!(words.contains("\"sub\""));
        assert!(words.contains("SUB"));
        assert!(words.contains("Rename the project's `library` field"));
    }
}
