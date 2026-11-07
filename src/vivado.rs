//! The vivado target drives: the orbit blueprint in, a judged verdict
//! out -- the richest probed process in this crate.
//!
//! Two orbit targets share this drive, split by [`Mode`]: `vivado-build`
//! (compile + the generated synthesis script + the batch synth, the
//! netlists as payload, exit honest, nothing judged) and `vivado` (the
//! same compile, then the judged runs: xelab per top, xsim per snapshot).
//! The drive reads the blueprint orbit wrote for the target (the TSV
//! plan) and carries BOTH languages: VHDL entries compile through
//! `xvhdl`, `VLOG`/`SYSV` entries through `xvlog` -- vivado is the one
//! carried tool that speaks both.
//!
//! The compile: xvhdl is order-sensitive within the work library
//! (`ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'`,
//! exit 1, probed live on 2025.2.1), so the VHDL files compile one
//! invocation per file in blueprint order through the deferral loop (the
//! ghdl-precedent machinery, borrowed a third time); xvlog is order-free
//! (a consumer-before-provider pair compiled clean, probed live), so the
//! Verilog files compile in ONE invocation. Both compile steps gate on
//! the exit: every probed compile failure exits 1, and the faces are
//! uppercase `ERROR:` on STDOUT -- stderr is empty on every stage-binary
//! drive, error or clean (probed) -- which the `Vivado` vocabulary
//! deliberately does not carry (below). No std flag is pinned: the
//! tool's default compiles GRLIB-scale designs green, and `--2008`
//! breaks them (the `10-3083` homograph walls, probed live).
//!
//! The judge -- not xsim's exit code -- is the sim face's load-bearing
//! gate: xsim exits 0 on BOTH probed runtime failures (a violated
//! `severity failure` AND a violated `severity error`, probed live
//! 2026-11-07). The run exit is a lie; the log is the only truth, judged
//! line-anchored (`Failure:` / `Error:` / `Fatal:` at line start; case
//! drift and mid-line markers never judge). The exit stays the second
//! witness for the crash door: a missing snapshot answers
//! `ERROR: Please check the snapshot name ...` and exits 1 -- an
//! uppercase face the vocabulary does not carry, and the exit convicts
//! instead.
//!
//! The build face's synth is script-generated: one `read_vhdl` /
//! `read_verilog` per file (batch `read_*` takes one file per command;
//! read order irrelevant, probed), per top one `synth_design -top <top>
//! -part <part> -mode out_of_context` and the three netlist writers into
//! `netlists/` (`write_checkpoint`/`write_verilog`/`write_edif`), the
//! whole body inside ONE return-code catch-guard that exits 1 -- Tcl's
//! `catch {script} var` writes the script's NORMAL RETURN VALUE into
//! `var` and the writers return the written filename, so an emptiness
//! guard false-fails a clean synth (probed live: exit 1 with all three
//! netlists written). The part comes from `--part` and is required
//! BEFORE any drive (exit 2 at the CLI): the probed alternative is a
//! ~4s synth-then-fail inside the tool.
//!
//! The launch recipe: every vivado-family spawn needs
//! `LC_ALL=en_US.UTF-8` plus `LOCPATH` naming a directory that carries
//! it (the launcher aborts otherwise on a box without the system
//! locale), and the main wrapper's full init needs the SuSE ncurses5
//! leg (`<prefix>/lib/lnx64.o/SuSE`). The drive sets all three on every
//! spawn -- the environment's own value wins, the probed defaults
//! otherwise (LOCPATH under `$HOME/.local/locale`; the LD_LIBRARY_PATH
//! leg derived from the resolved vivado install) -- and the scoping is
//! the spawn only: `Command::env` never touches this process, because a
//! process-wide LD_LIBRARY_PATH poisons other tools' backends (the ghdl
//! LLVM backend fails on Vivado's bundled libstdc++, probed). The
//! vivado binary resolves from the `VIVADO_BIN` env hook and the stage
//! binaries ride beside it as
//! siblings of the resolved path; a bare PATH name falls back to the
//! PATH lookup for the whole family (a settings64-sourced shell).

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::blueprint::{parse_blueprint, plan_note, vivado_files, BlueprintError, Drive, Mode};
use crate::contracts::{self, Tool};
use crate::ghdl::analyze_with_deferral;
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

/// The env hook naming the vivado binary. The stage binaries run on
/// this alone when the recipe legs are derived.
pub const VIVADO_BIN_ENV: &str = "VIVADO_BIN";

/// The vivado binary's bare name, the PATH fallback when `VIVADO_BIN` is
/// unset. The orbit target contract carries no binary-path field.
pub const VIVADO_BIN: &str = "vivado";

/// The stage binaries, resolved beside the vivado binary.
pub const XVHDL_BIN: &str = "xvhdl";
pub const XVLOG_BIN: &str = "xvlog";
pub const XELAB_BIN: &str = "xelab";
pub const XSIM_BIN: &str = "xsim";

/// The work library every compile lands in (`-work work` on xvhdl and
/// xvlog alike). It lives inside [`XSIM_DIR`] with the snapshots, and the
/// blueprint's library column is carried, not mapped -- one `work`
/// library, like every other drive in this crate.
pub const WORK: &str = "work";

/// The xsim state directory: the compiled `work` library AND the per-top
/// snapshots land under it. It is vivado's clock.
///
/// Orbit's output directory persists across runs, and the drive wipes it
/// before every drive -- the same correctness-before-incrementality
/// stance as the work libraries. Probed honestly: the wipe is symmetry
/// here, not load-bearing -- a failed compile or elaboration exits 1
/// before the sim face could run a stale snapshot.
pub const XSIM_DIR: &str = "xsim.dir";

/// The netlist directory the build face writes every top into
/// (`netlists/<top>.dcp`, `<top>.v`, `<top>.edf`). Wiped and recreated
/// before every build-face drive, the same symmetry as yosys's.
pub const NETLIST_DIR: &str = "netlists";

/// The locale the vivado launcher names (`LC_ALL`), probed live: the
/// launcher aborts without a UTF-8 locale it can resolve. The
/// environment's own `LC_ALL` wins; this default rides otherwise, and
/// `LOCPATH` points at where it lives (the no-root `localedef` recipe).
pub const LOCALE: &str = "en_US.UTF-8";

/// Resolve the vivado binary: the `VIVADO_BIN` env hook first, the bare
/// name on PATH second. An empty env value counts as absent, like the
/// exported testbench.
pub fn vivado_bin(env: Option<&OsStr>) -> PathBuf {
    match env {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(VIVADO_BIN),
    }
}

/// The stage binary beside a resolved vivado: when the vivado path names
/// a real install directory carrying it, that absolute sibling; otherwise
/// the bare name on PATH (a settings64-sourced shell has the whole
/// family on PATH).
pub fn stage_bin(vivado: &Path, name: &str) -> PathBuf {
    match vivado.parent() {
        Some(dir) if dir.join(name).exists() => dir.join(name),
        _ => PathBuf::from(name),
    }
}

/// The SuSE ncurses5 library dir of the resolved vivado install:
/// `<prefix>/lib/lnx64.o/SuSE`, where `<prefix>` is vivado's grandparent
/// (`/opt/2025.2.1/Vivado/bin/vivado` -> `/opt/2025.2.1/Vivado`).
/// `None` when the resolution cannot name a grandparent -- a bare PATH
/// name carries no install dir, and then the environment's own
/// `LD_LIBRARY_PATH` (or nothing) rides.
pub fn suse_lib_dir(vivado: &Path) -> Option<PathBuf> {
    let bin_dir = vivado.parent()?;
    let prefix = bin_dir.parent()?;
    Some(prefix.join("lib").join("lnx64.o").join("SuSE"))
}

/// The launch recipe, read off the drive's own environment. The fields
/// are `None` when the environment does not carry them, and the defaults
/// fill in at [`Recipe::env_pairs`].
struct Recipe {
    lc_all: Option<String>,
    locpath: Option<OsString>,
    ld_library_path: Option<OsString>,
    home: Option<OsString>,
}

impl Recipe {
    /// Read the recipe legs off the drive's own environment.
    fn from_env() -> Self {
        Self {
            lc_all: std::env::var("LC_ALL").ok(),
            locpath: std::env::var_os("LOCPATH"),
            ld_library_path: std::env::var_os("LD_LIBRARY_PATH"),
            home: std::env::var_os("HOME"),
        }
    }

    /// The env pairs to set on one vivado-family spawn, probed legs
    /// first:
    ///
    /// - `LC_ALL`: the environment's own wins; the probed
    ///   `en_US.UTF-8` default otherwise.
    /// - `LOCPATH`: the environment's own wins; `$HOME/.local/locale`
    ///   (the no-root `localedef` recipe) otherwise. Omitted when neither
    ///   exists -- a box with the locale system-installed needs no
    ///   `LOCPATH`.
    /// - `LD_LIBRARY_PATH`: the environment's own wins (a fully exported
    ///   recipe passes through); the SuSE leg derived from the resolved
    ///   install otherwise. Omitted when neither exists -- the wrapper
    ///   then answers for itself, loudly.
    ///
    /// The pairs ride ONE spawn (`Command::env`), never this process: a
    /// process-wide `LD_LIBRARY_PATH` poisons other tools' backends.
    fn env_pairs(&self, vivado: &Path) -> Vec<(String, OsString)> {
        let mut pairs: Vec<(String, OsString)> = Vec::new();
        let lc_all: OsString = self
            .lc_all
            .clone()
            .unwrap_or_else(|| LOCALE.to_string())
            .into();
        pairs.push(("LC_ALL".to_string(), lc_all));
        let locpath = self.locpath.clone().or_else(|| {
            self.home.as_ref().map(|home| {
                Path::new(home)
                    .join(".local")
                    .join("locale")
                    .into_os_string()
            })
        });
        if let Some(locpath) = locpath {
            pairs.push(("LOCPATH".to_string(), locpath));
        }
        let ld = self
            .ld_library_path
            .clone()
            .or_else(|| suse_lib_dir(vivado).map(|dir| dir.into_os_string()));
        if let Some(ld) = ld {
            pairs.push(("LD_LIBRARY_PATH".to_string(), ld));
        }
        pairs
    }

    /// Apply the recipe to one vivado-family spawn.
    fn apply_to(&self, command: &mut Command, vivado: &Path) {
        for (key, value) in self.env_pairs(vivado) {
            command.env(key, value);
        }
    }
}

/// Generate the one synthesis script for a top: one `read_vhdl` /
/// `read_verilog` per file, the `synth_design` for the top, the three
/// netlist writers -- the whole body inside ONE return-code catch-guard
/// that exits 1. See the module docs for the probed guard trap: the
/// script tests `catch`'s return code, never the caught string.
pub fn synth_script(top: &str, part: &str, vhdl: &[PathBuf], verilog: &[PathBuf]) -> String {
    let mut script = String::new();
    script.push_str("if {[catch {\n");
    for file in vhdl {
        script.push_str("read_vhdl ");
        script.push_str(&file.to_string_lossy());
        script.push('\n');
    }
    for file in verilog {
        script.push_str("read_verilog ");
        script.push_str(&file.to_string_lossy());
        script.push('\n');
    }
    script.push_str("synth_design -top ");
    script.push_str(top);
    script.push_str(" -part ");
    script.push_str(part);
    script.push_str(" -mode out_of_context\n");
    script.push_str("write_checkpoint -force ");
    script.push_str(NETLIST_DIR);
    script.push('/');
    script.push_str(top);
    script.push_str(".dcp\n");
    script.push_str("write_verilog -force ");
    script.push_str(NETLIST_DIR);
    script.push('/');
    script.push_str(top);
    script.push_str(".v\n");
    script.push_str("write_edif -force ");
    script.push_str(NETLIST_DIR);
    script.push('/');
    script.push_str(top);
    script.push_str(".edf\n");
    script.push_str("} synth_err]} {\n");
    script.push_str("  puts \"SYNTH FAILED: $synth_err\"\n");
    script.push_str("  exit 1\n");
    script.push_str("}\n");
    script.push_str("exit 0\n");
    script
}

/// The record of one top's xsim run: what it exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top unit whose snapshot ran.
    pub top: String,
    /// xsim's exit code, recorded but secondary: it never flips the
    /// verdict -- and it CANNOT, for the probed run exit is a lie: xsim
    /// exits 0 on a violated `severity failure` AND on a violated
    /// `severity error` (probed live on 2025.2.1). A `-1` records a run
    /// that died by signal rather than exiting.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a probed vivado failure face: a line
    /// that LEADS with one of the three markers (`Fatal:` / `Error:` /
    /// `Failure:`). Case drift and mid-line markers never judge -- the
    /// vocabulary is line-anchored, the only one in this crate.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Vivado, &self.log)
    }

    /// The gate: `true` when the run is clean -- no probed failure face
    /// in the log AND xsim exited 0.
    ///
    /// Either witness convicts, and the face is the FIRST witness here:
    /// the probed run exit is a lie (xsim exits 0 on both runtime
    /// failures), so the face in the log is the only witness on those
    /// failures. The exit stays a witness for the crash door -- a
    /// missing snapshot exits 1 without any carried face -- and a clean
    /// log must not green-light it.
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
    /// A vivado-family binary could not be spawned at all: usage-class,
    /// exit 2.
    Spawn(String),
    /// The run-scoped state could not be reset: usage-class, exit 2.
    Library(String),
    /// No FPGA part was named on the build face: usage-class, exit 2
    /// (the CLI gate fires first; the drive carries the same door for
    /// direct library callers, the cocotb `TestModule` door's shape).
    Part,
    /// A compile failed after the deferral loop exhausted or the one
    /// xvlog refused: findings, exit 1 (the stage binaries' compile
    /// exits are honest: exit 1 on every probed failure, probed live).
    Compile {
        /// The stage binary that refused: `xvhdl` or `xvlog`.
        stage: String,
        /// The captured log(s) of the failed compile, verbatim.
        log: String,
    },
    /// A top failed to elaborate (the sim face): findings, exit 1 (the
    /// xelab exit is honest: `ERROR: [XSIM 43-3225] Cannot find design
    /// unit ...`, exit 1, probed).
    Elaborate {
        /// The top unit xelab refused.
        top: String,
        /// The captured log of the failed elaboration, verbatim.
        log: String,
    },
    /// A top failed to synthesize (the build face): findings, exit 1
    /// (the batch exit is honest behind the catch-guard: exit 1 on every
    /// probed failure, fail-fast).
    Synth {
        /// The top module the batch refused to synthesize.
        top: String,
        /// The captured log of the failed synth, verbatim.
        log: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings vivado or the judge
    /// reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_)
            | DriveError::Part => 2,
            DriveError::Compile { .. }
            | DriveError::Elaborate { .. }
            | DriveError::Synth { .. }
            | DriveError::Findings(_) => 1,
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
            DriveError::Part => write!(f, "no part to synthesize"),
            DriveError::Compile { stage, .. } => write!(f, "the {stage} compile failed"),
            DriveError::Elaborate { top, .. } => {
                write!(f, "the xelab elaboration failed for {top}")
            }
            DriveError::Synth { top, .. } => write!(f, "the vivado synth failed for {top}"),
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

/// The build face's part door: `--part` names the FPGA fabric every
/// synth targets, and there is no honest default -- the probed
/// alternative is a ~4s synth-then-fail inside the tool. The CLI gate
/// fires first (exit 2 naming the flag); this door keeps the drive
/// honest for direct library callers.
fn require_part(request: &Drive) -> Result<&str, DriveError> {
    match request.part.as_deref() {
        Some(part) if !part.is_empty() => Ok(part),
        _ => Err(DriveError::Part),
    }
}

/// The error for blueprint filesets the vivado drives do not carry: a
/// custom fileset, the one thing neither language family speaks. VHDL,
/// `VLOG`, and `SYSV` all ride -- vivado is the one carried tool that
/// speaks both languages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// Every custom-fileset entry, in blueprint order.
    pub entries: Vec<crate::blueprint::Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the vivado drives carry the VHDL, VLOG, and SYSV filesets only; not yet carried:"
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

/// Drive the vivado target end to end: blueprint, compile, and then --
/// per face -- the judged runs or the generated synthesis.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and
/// vivado's state (`xsim.dir/`, `netlists/`) land there. All progress
/// and diagnostics are printed here; the returned error only picks the
/// exit code. The mode picks the face: `Build` compiles and synthesizes
/// per top (no run, no log, nothing judged); `Sim` compiles, elaborates
/// per top, and judges each run.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    let vivado = vivado_bin(std::env::var_os(VIVADO_BIN_ENV).as_deref());
    // vivado must be spawnable before anything else, so a missing binary
    // is a loud usage error instead of a silent compile failure.
    // `-version` (single dash) is the family's identity face, probed exit
    // 0 on every binary.
    let mut probe = Command::new(&vivado);
    probe.arg("-version");
    Recipe::from_env().apply_to(&mut probe, &vivado);
    capture(VIVADO_BIN, &mut probe).map_err(DriveError::Spawn)?;
    let xvhdl = stage_bin(&vivado, XVHDL_BIN);
    let xvlog = stage_bin(&vivado, XVLOG_BIN);
    let xelab = stage_bin(&vivado, XELAB_BIN);
    let xsim = stage_bin(&vivado, XSIM_BIN);

    // The part door, BEFORE any drive: the build face names the FPGA
    // part every synth targets (`--part`), and there is no honest
    // default -- the probed alternative is a ~4s synth-then-fail inside
    // the tool. The CLI gate fires first (exit 2 naming the flag); this
    // door keeps the drive honest for direct library callers. The sim
    // face never sees a part (the argv parser rejects the flag there).
    let part: Option<&str> = match mode {
        Mode::Build => Some(require_part(request).map_err(|e| {
            eprintln!("orbit-drives: vivado: error: {e}: pass --part <part>");
            e
        })?),
        Mode::Sim => None,
    };

    // Run-scoped state: see `XSIM_DIR`. Both faces wipe the whole state
    // directory -- it holds the compiled work library AND the snapshots
    // -- and the build face resets its payload directory too (probed
    // honestly: both wipes are symmetry).
    if let Err(e) = fs::remove_dir_all(XSIM_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale state directory {XSIM_DIR}/: {e}");
            eprintln!("orbit-drives: vivado: error: {message}");
            return Err(DriveError::Library(message));
        }
    }
    if mode == Mode::Build {
        if let Err(e) = fs::remove_dir_all(NETLIST_DIR) {
            if e.kind() != std::io::ErrorKind::NotFound {
                let message =
                    format!("cannot wipe the stale netlist directory {NETLIST_DIR}/: {e}");
                eprintln!("orbit-drives: vivado: error: {message}");
                return Err(DriveError::Library(message));
            }
        }
        if let Err(e) = fs::create_dir_all(NETLIST_DIR) {
            let message = format!("cannot create the netlist directory {NETLIST_DIR}/: {e}");
            eprintln!("orbit-drives: vivado: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    if let Some(note) = plan_note(
        "vivado",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: vivado: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: vivado: error: {e}");
        DriveError::Blueprint(e)
    })?;
    // Vivado speaks both languages: VHDL entries to xvhdl, VLOG and SYSV
    // entries to xvlog, a custom fileset the loud not-carried face.
    let (vhdl, verilog) = vivado_files(&entries).map_err(|e| {
        eprintln!("orbit-drives: vivado: error: {e}");
        DriveError::NotCarried(e)
    })?;

    // The compile: xvhdl first (order-sensitive within the work library,
    // probed -- the deferral loop), then xvlog (order-free, one
    // invocation, probed). Both gate on the honest exit; no std flag is
    // pinned (the tool default compiles GRLIB-scale designs green;
    // `--2008` breaks them, probed live).
    compile_vhdl(&vhdl, &xvhdl, &vivado)?;
    compile_verilog(&verilog, &xvlog, &vivado)?;
    println!(
        "orbit-drives: vivado: {label}: compiled {} VHDL and {} Verilog file(s) into {WORK} \
         (v1 carries no std flag: the tool default)",
        vhdl.len(),
        verilog.len()
    );

    let mut findings = Vec::new();
    for top in &request.tops {
        match mode {
            Mode::Build => {
                let part = part.expect("the build face validated the part before any drive");
                println!(
                    "orbit-drives: vivado: {label}: synthesizing {top} for {part} into \
                     {NETLIST_DIR}"
                );
                let script = synth_script(top, part, &vhdl, &verilog);
                let script_path = format!("orbit-synth-{top}.tcl");
                fs::write(&script_path, &script).map_err(|e| {
                    let message = format!("cannot write the synthesis script {script_path}: {e}");
                    eprintln!("orbit-drives: vivado: error: {message}");
                    DriveError::Library(message)
                })?;
                // The extras ride the drive's last tool argv verbatim:
                // the batch argv on the build face, the xsim run argv on
                // the sim face (there is no later step to carry them, and
                // a dropped argument would be a silent degradation). An
                // option vivado does not speak is its own loud rejection,
                // surfaced as an exit-1 synth failure.
                let mut synth = Command::new(&vivado);
                synth
                    .arg("-mode")
                    .arg("batch")
                    .arg("-nojournal")
                    .arg("-nolog")
                    .arg("-source")
                    .arg(&script_path);
                synth.args(&request.extra);
                Recipe::from_env().apply_to(&mut synth, &vivado);
                let output = capture(VIVADO_BIN, &mut synth).map_err(DriveError::Spawn)?;
                let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
                if !output.status.success() {
                    eprint_block(&log);
                    eprintln!("orbit-drives: vivado: error: the vivado synth failed for {top}");
                    return Err(DriveError::Synth {
                        top: top.clone(),
                        log,
                    });
                }
                println!(
                    "orbit-drives: vivado: {label}: {top}: clean ({NETLIST_DIR}/{top}.dcp, \
                     .v, .edf written)"
                );
            }
            Mode::Sim => {
                println!("orbit-drives: vivado: {label}: elaborating {top} from {WORK}");
                // The `-L work` pin (probed shape: `xelab -L work
                // work.<top> -s <top>`); the snapshot lands as
                // `xsim.dir/<top>` and is validated by elaboration (an
                // unknown top answers `ERROR: [XSIM 43-3225] Cannot find
                // design unit ...`, exit 1, probed).
                let mut elaborate = Command::new(&xelab);
                elaborate
                    .arg("-L")
                    .arg(WORK)
                    .arg(format!("{WORK}.{top}"))
                    .arg("-s")
                    .arg(top);
                Recipe::from_env().apply_to(&mut elaborate, &vivado);
                let output = capture(XELAB_BIN, &mut elaborate).map_err(DriveError::Spawn)?;
                let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
                if !output.status.success() {
                    eprint_block(&log);
                    eprintln!(
                        "orbit-drives: vivado: error: the xelab elaboration failed for {top}"
                    );
                    return Err(DriveError::Elaborate {
                        top: top.clone(),
                        log,
                    });
                }

                let outcome = run_snapshot(&xsim, top, &request.extra, &vivado, label)?;
                if outcome.clean() {
                    println!("orbit-drives: vivado: {label}: {top}: pass");
                } else if outcome.has_face() {
                    println!(
                        "orbit-drives: vivado: {label}: {top}: fail (the log carries a probed \
                         vivado failure face)"
                    );
                    findings.push(outcome);
                } else {
                    println!(
                        "orbit-drives: vivado: {label}: {top}: fail (xsim exited {} without a \
                         judged face)",
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
            "orbit-drives: vivado: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// Compile the VHDL files in blueprint order, ONE xvhdl invocation per
/// file, through the deferral loop: xvhdl is order-sensitive within the
/// work library (`ERROR: [VRFC 10-2987] 'top' is not compiled in library
/// 'work'`, exit 1, probed live on 2025.2.1), so a failing file defers to
/// a retry after the others; a pass that makes no progress stops the
/// loop. The faces land on STDOUT (stderr is empty on every stage-binary
/// drive, probed), so the residual logs carry the joined streams. The
/// step gates on the exit and is never judged: the compile exits are
/// honest, and the uppercase `ERROR:` faces are deliberately outside the
/// `Vivado` vocabulary.
fn compile_vhdl(files: &[PathBuf], xvhdl: &Path, vivado: &Path) -> Result<(), DriveError> {
    if files.is_empty() {
        return Ok(());
    }
    let mut probe = Command::new(xvhdl);
    probe.arg("-version");
    Recipe::from_env().apply_to(&mut probe, vivado);
    capture(XVHDL_BIN, &mut probe).map_err(DriveError::Spawn)?;

    println!(
        "orbit-drives: vivado: compiling {} VHDL file(s) in blueprint order with the \
         deferral loop (order-sensitive within {WORK}, probed live 2026-11-07)",
        files.len()
    );
    let recipe = Recipe::from_env();
    let report = analyze_with_deferral(files, |file| {
        let mut compile = Command::new(xvhdl);
        compile.arg("-work").arg(WORK).arg(file);
        recipe.apply_to(&mut compile, vivado);
        match capture(XVHDL_BIN, &mut compile) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(join_log(lossy(&output.stdout), lossy(&output.stderr))),
            Err(message) => Err(message),
        }
    });
    println!(
        "orbit-drives: vivado: {} VHDL file(s) compiled in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if report.residual.is_empty() {
        return Ok(());
    }
    eprintln!(
        "orbit-drives: vivado: error: {} VHDL file(s) never compiled after {} pass(es)",
        report.residual.len(),
        report.passes
    );
    let mut log = String::new();
    for residual in &report.residual {
        eprintln!(
            "orbit-drives: vivado: error: xvhdl failed: {}",
            residual.path.display()
        );
        eprint_block(&residual.stderr);
        log.push_str(&format!("{}:\n", residual.path.display()));
        log.push_str(&residual.stderr);
        if !residual.stderr.is_empty() && !residual.stderr.ends_with('\n') {
            log.push('\n');
        }
    }
    eprintln!("orbit-drives: vivado: error: the xvhdl compile failed");
    Err(DriveError::Compile {
        stage: XVHDL_BIN.to_string(),
        log,
    })
}

/// Compile the Verilog files in ONE xvlog invocation: xvlog is
/// order-free (a consumer-before-provider pair compiled clean, probed
/// live on 2025.2.1), so there is nothing to defer. The step gates on
/// the honest exit and is never judged, like the VHDL compile.
fn compile_verilog(files: &[PathBuf], xvlog: &Path, vivado: &Path) -> Result<(), DriveError> {
    if files.is_empty() {
        return Ok(());
    }
    let mut probe = Command::new(xvlog);
    probe.arg("-version");
    Recipe::from_env().apply_to(&mut probe, vivado);
    capture(XVLOG_BIN, &mut probe).map_err(DriveError::Spawn)?;

    println!(
        "orbit-drives: vivado: compiling {} Verilog file(s) into one invocation (order-free, \
         probed live 2026-11-07)",
        files.len()
    );
    let mut compile = Command::new(xvlog);
    compile.arg("-work").arg(WORK).args(files);
    Recipe::from_env().apply_to(&mut compile, vivado);
    let output = capture(XVLOG_BIN, &mut compile).map_err(DriveError::Spawn)?;
    if output.status.success() {
        return Ok(());
    }
    let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
    eprint_block(&log);
    eprintln!("orbit-drives: vivado: error: the xvlog compile failed");
    Err(DriveError::Compile {
        stage: XVLOG_BIN.to_string(),
        log,
    })
}

/// Run one elaborated snapshot and capture its log:
/// `xsim <top> -R <extras>`. The probed failure faces (`Failure:`,
/// `Error:` at line start) land on stdout; the log joins both streams,
/// so it reads either way. The probed run exit is a LIE -- xsim exits 0
/// on both runtime failures -- so the judge reads the log; the exit
/// stays the crash-door witness (a missing snapshot exits 1, probed,
/// with an uppercase face the vocabulary does not carry).
fn run_snapshot(
    xsim: &Path,
    top: &str,
    extra: &[String],
    vivado: &Path,
    label: &str,
) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: vivado: {label}: running {top}");
    let mut run = Command::new(xsim);
    run.arg(top).arg("-R").args(extra);
    Recipe::from_env().apply_to(&mut run, vivado);
    let output = capture(XSIM_BIN, &mut run).map_err(DriveError::Spawn)?;
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

    // The probed severity-failure fixture (xsim 2025.2.1, 2026-11-07):
    // the run CONTINUES to `$finish` and EXITS 0 -- the exit is a lie,
    // the line-start `Failure:` face is the only witness. Byte-for-byte
    // as recorded live.
    const FAILURE_LOG: &str = "\
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

    // The probed severity-error fixture (xsim 2025.2.1, 2026-11-07): the
    // same lie -- the face, then the continuation note, then exit 0.
    const ERROR_LOG: &str = "\
run -all
Error: check failed
Time: 1 ns  Iteration: 0  Process: /tb_err/stim  File: ./tb_err.vhd
Note: continued past the error
Time: 1 ns  Iteration: 0  Process: /tb_err/stim  File: ./tb_err.vhd
exit
";

    // The probed clean run: the severity-note face of a passing report
    // and the kernel INFO lines -- continuations, never judged.
    const CLEAN_LOG: &str = "\
run -all
Note: sim done
Time: 1 ns  Iteration: 0  Process: /tb_clean/stim  File: ./tb_clean.vhd
exit
INFO: xsimkernel Simulation Memory Usage: 269612 KB (Peak: 335148 KB)
INFO: [Common 17-206] Exiting xsim at Sat Nov  7 18:07:16 2026...
";

    // The probed compile faces (xvhdl, 2025.2.1): UPPERCASE `ERROR:`,
    // line-start, on stdout -- deliberately outside the vocabulary; the
    // compile gates on its honest exit and is never judged.
    const COMPILE_ERROR_LOG: &str = "\
INFO: [VRFC 10-163] Analyzing VHDL file \"./broken.vhd\" into library work
INFO: [VRFC 10-3107] analyzing entity 'broken'
ERROR: [VRFC 10-4982] syntax error near 'end' [./broken.vhd:9]
ERROR: [VRFC 10-2989] 'y' is not declared [./broken.vhd:8]
INFO: [VRFC 10-8704] VHDL file './broken.vhd' is ignored due to errors
";

    // The probed order-sensitivity face (xvhdl, consumer before provider,
    // exit 1, stdout): the deferral loop's reason to exist here.
    const ORDER_ERROR_LOG: &str = "\
ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work' [./tb_clean.vhd:11]
ERROR: [VRFC 10-9458] unit 'sim' is ignored due to previous errors [./tb_clean.vhd:7]
";

    // The probed elaboration refusal (xelab on a top naming no analyzed
    // unit, exit 1, stdout): honest, the elaboration gates on the exit.
    const UNKNOWN_UNIT_LOG: &str = "\
ERROR: [XSIM 43-3225] Cannot find design unit work.tb_nosuch in library work located at xsim.dir/work.
";

    // The probed crash door (xsim on a missing snapshot, exit 1,
    // stdout): an uppercase face the vocabulary does not carry -- the
    // exit witness convicts.
    const MISSING_SNAPSHOT_LOG: &str = "\
ERROR: Please check the snapshot name which is created during 'xelab',the current snapshot name \"xsim.dir/nosuch_snap/xsimk\" does not exist
";

    #[test]
    fn a_failure_run_convicts_on_the_face_alone() {
        // The lie, pinned: xsim exits 0 on a violated `severity failure`
        // (probed live on 2025.2.1). The face is the only witness on
        // that run; an exit of 0 must never pass it.
        let outcome = RunOutcome {
            top: "tb_fail".to_string(),
            exit: 0,
            log: FAILURE_LOG.to_string(),
        };
        assert_eq!(outcome.exit, 0);
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn an_error_run_convicts_on_the_face_alone() {
        // The same lie for a violated `severity error`: the run
        // CONTINUES (the note follows) and exits 0. The face convicts.
        let outcome = RunOutcome {
            top: "tb_err".to_string(),
            exit: 0,
            log: ERROR_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_crashed_run_is_a_finding_on_its_exit_alone() {
        // The crash door: a missing snapshot (or a killed xsim) exits
        // nonzero without any carried face, and a clean log must not
        // green-light it. Either witness convicts.
        let outcome = RunOutcome {
            top: "nosuch_snap".to_string(),
            exit: 1,
            log: MISSING_SNAPSHOT_LOG.to_string(),
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
    fn a_clean_run_passes_the_gate() {
        // The probed clean run: the `Note:` face and the kernel INFO
        // lines on a run that exits 0 -- continuations, never judged.
        let outcome = RunOutcome {
            top: "tb_clean".to_string(),
            exit: 0,
            log: CLEAN_LOG.to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn the_compile_and_elaboration_faces_never_judge() {
        // The probed compile and elaboration faces are UPPERCASE
        // `ERROR:` -- deliberately outside the mixed-case vocabulary
        // (the case drift pin): they never judge a log, the stages gate
        // on their honest exits.
        assert!(contracts::judge(&Tool::Vivado, COMPILE_ERROR_LOG));
        assert!(contracts::judge(&Tool::Vivado, ORDER_ERROR_LOG));
        assert!(contracts::judge(&Tool::Vivado, UNKNOWN_UNIT_LOG));
        assert!(contracts::judge(&Tool::Vivado, MISSING_SNAPSHOT_LOG));
    }

    #[test]
    fn the_faces_are_line_anchored() {
        // The line anchor, pinned both ways: a marker at line start
        // judges; the same marker mid-line never judges -- and the case
        // drift (`FATAL:`) never judges even at line start.
        assert!(contracts::judge(
            &Tool::Vivado,
            "the sim says: Failure: retrying\n"
        ));
        assert!(!contracts::judge(
            &Tool::Vivado,
            "Failure: the design is wrong\n"
        ));
        assert!(contracts::judge(
            &Tool::Vivado,
            "FATAL: the design is wrong\n"
        ));
    }

    #[test]
    fn vivado_and_quartus_are_the_line_anchored_tools() {
        // The registry's line-anchored vocabularies: vivado's (the
        // probed pins) and quartus's (the documented ModelSim-family
        // hypothesis riding the same anchor). Every other tool's faces
        // ride the substring search.
        assert!(Tool::Vivado.line_anchored());
        assert!(Tool::Quartus.line_anchored());
        for tool in contracts::tools() {
            if tool.name() != "vivado" && tool.name() != "quartus" {
                assert!(
                    !tool.line_anchored(),
                    "{} should not be line-anchored",
                    tool.name()
                );
            }
        }
    }

    #[test]
    fn the_synthesis_script_guards_on_catchs_return_code() {
        // The probed Tcl trap (2025.2.1, 2026-11-07): `catch {script}
        // var` writes the script's NORMAL RETURN VALUE into `var`, and
        // the netlist writers return the written filename -- a guard
        // that tested the caught string's emptiness false-failed a CLEAN
        // synth (exit 1 with all three netlists written). The generated
        // script must test catch's return code instead.
        let script = synth_script(
            "top",
            "xc7z010clg400-1",
            &[PathBuf::from("./top.vhd"), PathBuf::from("./gate.vhd")],
            &[PathBuf::from("./vgate.v")],
        );
        assert!(script.starts_with("if {[catch {\n"));
        assert!(script.contains("read_vhdl ./top.vhd\n"));
        assert!(script.contains("read_vhdl ./gate.vhd\n"));
        assert!(script.contains("read_verilog ./vgate.v\n"));
        assert!(
            script.contains("synth_design -top top -part xc7z010clg400-1 -mode out_of_context\n")
        );
        assert!(script.contains("write_checkpoint -force netlists/top.dcp\n"));
        assert!(script.contains("write_verilog -force netlists/top.v\n"));
        assert!(script.contains("write_edif -force netlists/top.edf\n"));
        assert!(script.contains("} synth_err]} {\n"));
        assert!(script.contains("  puts \"SYNTH FAILED: $synth_err\"\n"));
        assert!(script.contains("  exit 1\n"));
        assert!(script.ends_with("}\nexit 0\n"));
    }

    #[test]
    fn the_recipe_env_carries_the_probed_legs() {
        // The probed recipe, as env pairs: LC_ALL always set (the env's
        // own wins, the probed locale otherwise), LOCPATH defaulted from
        // HOME, the LD_LIBRARY_PATH leg derived from the resolved
        // install.
        let recipe = Recipe {
            lc_all: None,
            locpath: None,
            ld_library_path: None,
            home: Some("/home/nabil".into()),
        };
        let vivado = Path::new("/opt/2025.2.1/Vivado/bin/vivado");
        let pairs = recipe.env_pairs(vivado);
        assert_eq!(pairs[0], ("LC_ALL".to_string(), "en_US.UTF-8".into()));
        assert!(pairs.contains(&("LOCPATH".to_string(), "/home/nabil/.local/locale".into())));
        assert!(pairs.contains(&(
            "LD_LIBRARY_PATH".to_string(),
            "/opt/2025.2.1/Vivado/lib/lnx64.o/SuSE".into()
        )));

        // The environment's own legs win over every default.
        let exported = Recipe {
            lc_all: Some("C.UTF-8".to_string()),
            locpath: Some("/custom/locale".into()),
            ld_library_path: Some("/custom/lib".into()),
            home: Some("/home/nabil".into()),
        };
        let pairs = exported.env_pairs(vivado);
        assert_eq!(pairs[0], ("LC_ALL".to_string(), "C.UTF-8".into()));
        assert!(pairs.contains(&("LOCPATH".to_string(), "/custom/locale".into())));
        assert!(pairs.contains(&("LD_LIBRARY_PATH".to_string(), "/custom/lib".into())));
    }

    #[test]
    fn the_recipe_omits_legs_it_cannot_name() {
        // No HOME and no LOCPATH: the pair is omitted, not defaulted to
        // nothing. No install dir to derive the SuSE leg from: the same.
        let bare = Recipe {
            lc_all: None,
            locpath: None,
            ld_library_path: None,
            home: None,
        };
        let pairs = bare.env_pairs(Path::new("vivado"));
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "LC_ALL");
    }

    #[test]
    fn the_binaries_resolve_from_the_hook_and_the_siblings() {
        // VIVADO_BIN wins, an empty value counts as absent, the bare
        // name is the fallback.
        assert_eq!(
            vivado_bin(Some("/opt/2025.2.1/Vivado/bin/vivado".as_ref())),
            PathBuf::from("/opt/2025.2.1/Vivado/bin/vivado")
        );
        assert_eq!(vivado_bin(Some("".as_ref())), PathBuf::from(VIVADO_BIN));
        assert_eq!(vivado_bin(None), PathBuf::from(VIVADO_BIN));

        // A resolved install carries its stage binaries as siblings; a
        // bare PATH name falls back to the PATH lookup.
        let dir = std::env::temp_dir().join(format!("orbit-drives-viv-res-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("vivado"), "").unwrap();
        fs::write(dir.join("xvhdl"), "").unwrap();
        let installed = dir.join("vivado");
        assert_eq!(stage_bin(&installed, "xvhdl"), dir.join("xvhdl"));
        assert_eq!(stage_bin(&installed, XVLOG_BIN), PathBuf::from(XVLOG_BIN));
        assert_eq!(
            stage_bin(Path::new("vivado"), "xvhdl"),
            PathBuf::from("xvhdl")
        );

        // The SuSE leg derives from the install's grandparent; a bare
        // name carries none.
        assert_eq!(
            suse_lib_dir(&installed),
            Some(
                dir.parent()
                    .unwrap()
                    .join("lib")
                    .join("lnx64.o")
                    .join("SuSE")
            )
        );
        assert_eq!(suse_lib_dir(Path::new("vivado")), None);
        fs::remove_dir_all(&dir).unwrap();
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
                text: "MYSET\twork".to_string(),
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
            DriveError::Spawn("vivado: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(DriveError::Part.exit_code(), 2);
        assert_eq!(
            DriveError::Compile {
                stage: "xvhdl".to_string(),
                log: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(
            DriveError::Elaborate {
                top: "tb".to_string(),
                log: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(
            DriveError::Synth {
                top: "top".to_string(),
                log: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(DriveError::Findings(Vec::new()).exit_code(), 1);
    }

    #[test]
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("vivado", Some("tsv")).is_none());
        assert!(plan_note("vivado", None).is_none());
        let note = plan_note("vivado", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: vivado: note:"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Compile {
            stage: "xvhdl".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the xvhdl compile failed");
        let e = DriveError::Elaborate {
            top: "tb".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the xelab elaboration failed for tb");
        let e = DriveError::Synth {
            top: "top".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the vivado synth failed for top");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
        // The part door, the CLI gate's message family.
        assert_eq!(DriveError::Part.to_string(), "no part to synthesize");
    }

    #[test]
    fn the_build_face_requires_the_part_for_direct_callers() {
        // The part door: present and non-empty rides; absent and empty
        // are the loud usage error. The CLI gate fires first; this door
        // keeps the direct library caller honest.
        let request = Drive {
            blueprint: PathBuf::from("b.tsv"),
            tops: vec!["top".to_string()],
            extra: Vec::new(),
            part: Some("xc7z010clg400-1".to_string()),
            test_module: None,
        };
        assert_eq!(require_part(&request).unwrap(), "xc7z010clg400-1");
        let absent = Drive {
            part: None,
            ..request.clone()
        };
        assert!(matches!(require_part(&absent), Err(DriveError::Part)));
        let empty = Drive {
            part: Some(String::new()),
            ..request
        };
        assert!(matches!(require_part(&empty), Err(DriveError::Part)));
    }
}
