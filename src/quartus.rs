//! The quartus target drives: the orbit blueprint in, a judged verdict
//! out -- the DOCUMENTED-ONLY drive of this crate.
//!
//! Two orbit targets share this drive, split by [`Mode`]: `quartus-build`
//! (the generated `.qsf` + one batch `quartus_map` per top, exit honest,
//! nothing judged) and `quartus` (the bundled Questa drive: vlib/vmap/
//! vcom per file through the deferral loop, then the judged `vsim` run).
//! The drive reads the blueprint orbit wrote for the target (the TSV
//! plan). The build face carries BOTH languages -- the `.qsf` gains one
//! `VHDL_FILE` / `VERILOG_FILE` line per file; the sim face carries the
//! VHDL fileset only, because the documented recipe is vcom's alone.
//!
//! **Nothing in this drive is probed.** quartus is not installed on the
//! probing box, so every CLI face below is the documented hypothesis,
//! and every hypothesis carries its first-contact rule (see the README's
//! honest limits): the DEVICE-only sufficiency of the `.qsf`, the Questa
//! sibling-directory derive (`questa_fse/bin` beside the quartus
//! install), vsim's exit honesty, the vcom default standard, and the
//! `-L`-before-top order. The failure vocabulary
//! ([`crate::contracts::QUARTUS_VOCABULARY`]) rides the ModelSim-family
//! marker hypothesis -- line-start `Fatal:` / `Error:` / `Failure:`,
//! UNPROBED -- and the exit-contract hypothesis is vsim honest. On the
//! first quartus box the owner probes the real faces and rewrites both;
//! until then neither witness is trusted alone and either convicts, the
//! same as everywhere in this crate.
//!
//! The deferral loop is [`crate::ghdl::analyze_with_deferral`], borrowed
//! a fourth time: vcom is a stateful library compiler (the ModelSim
//! workflow compiles in dependency order), the documented
//! order-sensitivity hypothesis, so a failing file defers to a retry
//! after the others and a no-progress pass stops the loop.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::blueprint::{parse_blueprint, plan_note, BlueprintError, Drive, Mode};
use crate::contracts::{self, Tool};
use crate::ghdl::analyze_with_deferral;
use crate::proc::{capture, eprint_block, join_log, lossy, print_block};

/// The env hook naming the quartus binary (the `VIVADO_BIN` precedent):
/// the quartus_map binary or any quartus-family binary path, from which
/// the bundled Questa derives as the `questa_fse/bin` sibling. The bare
/// PATH name is the fallback.
pub const QUARTUS_BIN_ENV: &str = "QUARTUS_BIN";

/// The quartus_map binary's bare name, the PATH fallback when
/// `QUARTUS_BIN` is unset. The orbit target contract carries no
/// binary-path field.
pub const QUARTUS_MAP_BIN: &str = "quartus_map";

/// The bundled Questa binaries, resolved beside the quartus install.
pub const VLIB_BIN: &str = "vlib";
pub const VMAP_BIN: &str = "vmap";
pub const VCOM_BIN: &str = "vcom";
pub const VSIM_BIN: &str = "vsim";

/// The Questa sibling directory: the bundled simulator lives in
/// `questa_fse/bin` beside the quartus install (the documented derive --
/// UNPROBED; the first-contact rule is the README's). The quartus binary
/// path `<install>/quartus/bin/quartus_map` derives
/// `<install>/quartus/questa_fse/bin/<name>`.
pub const QUESTA_SUBDIR: &str = "questa_fse";

/// The language standard pinned on every vcom invocation (VHDL-2008).
/// vcom spells the standards `-93` / `-2002` / `-2008`; this crate pins
/// VHDL-2008 on every VHDL drive (ghdl's `--std=08`, nvc's `--std=2008`),
/// so the quartus drive pins the same standard in vcom's spelling. The
/// pin cannot be overridden from the argv yet -- the honest limits name
/// it. DOCUMENTED-ONLY: that vcom accepts `-2008` is part of the
/// hypothesis.
pub const STD: &str = "-2008";

/// The Questa work library every file compiles into (`-work work`). The
/// blueprint's library column is carried, not mapped -- one `work`
/// library, like every other drive in this crate.
pub const WORK: &str = "work";

/// The Questa library directory `vlib work` creates and the drive wipes
/// before every sim face -- Questa's clock, the nvc-precedent shape.
/// DOCUMENTED-ONLY: that a stale `work/` poisons a re-drive is part of
/// the hypothesis; the wipe is the stance regardless.
pub const WORK_LIBRARY_DIR: &str = "work";

/// The library mapping file `vmap work work` writes into the working
/// directory. Wiped with the library -- a stale mapping is drive state.
pub const MODELSIM_INI: &str = "modelsim.ini";

/// Resolve the quartus_map binary: the `QUARTUS_BIN` env hook first, the
/// bare name on PATH second. An empty env value counts as absent, like
/// the exported testbench.
pub fn quartus_bin(env: Option<&OsStr>) -> PathBuf {
    match env {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(QUARTUS_MAP_BIN),
    }
}

/// The bundled Questa binary beside a resolved quartus: when the quartus
/// path's grandparent carries `questa_fse/bin/<name>`, that absolute
/// sibling; otherwise the bare name on PATH (a settings64-sourced shell
/// has the whole family on PATH). DOCUMENTED-ONLY: the derive is the
/// hypothesis, the PATH fallback the first-contact escape hatch.
pub fn questa_bin(quartus: &Path, name: &str) -> PathBuf {
    let derived = quartus
        .parent()
        .and_then(|bin_dir| bin_dir.parent())
        .map(|install| install.join(QUESTA_SUBDIR).join("bin").join(name));
    match derived {
        Some(path) if path.exists() => path,
        _ => PathBuf::from(name),
    }
}

/// Generate the one project-settings file for a top: the DEVICE (from
/// `--part`), the TOP_LEVEL_ENTITY, and one `VHDL_FILE` /
/// `VERILOG_FILE` line per blueprint file. DOCUMENTED-ONLY: that these
/// assignments alone suffice for `quartus_map` is the hypothesis -- on
/// the first quartus box the owner probes what else a minimal `.qsf`
/// needs and extends this function. The file paths ride verbatim, split
/// on whitespace by the tool's own parser, like every script face in
/// this crate.
pub fn qsf_script(top: &str, part: &str, vhdl: &[PathBuf], verilog: &[PathBuf]) -> String {
    let mut script = String::new();
    script.push_str("set_global_assignment -name DEVICE ");
    script.push_str(part);
    script.push('\n');
    script.push_str("set_global_assignment -name TOP_LEVEL_ENTITY ");
    script.push_str(top);
    script.push('\n');
    for file in vhdl {
        script.push_str("set_global_assignment -name VHDL_FILE ");
        script.push_str(&file.to_string_lossy());
        script.push('\n');
    }
    for file in verilog {
        script.push_str("set_global_assignment -name VERILOG_FILE ");
        script.push_str(&file.to_string_lossy());
        script.push('\n');
    }
    script
}

/// The record of one top's vsim run: what it exited and what it wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    /// The top unit whose simulation ran.
    pub top: String,
    /// vsim's exit code, recorded but secondary by ruling: it never
    /// flips the verdict. A `-1` records a run that died by signal
    /// rather than exiting.
    pub exit: i32,
    /// The captured run log: stdout, then stderr.
    pub log: String,
}

impl RunOutcome {
    /// `true` when the log carries a quartus failure face: a line that
    /// LEADS with one of the three hypothesized ModelSim-family markers
    /// (`Fatal:` / `Error:` / `Failure:`). Case drift and mid-line
    /// markers never judge -- the vocabulary is line-anchored, riding
    /// vivado's probed anchor on the documented hypothesis.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Quartus, &self.log)
    }

    /// The gate: `true` when the run is clean -- no failure face in the
    /// log AND vsim exited 0.
    ///
    /// Either witness convicts, and neither is trusted alone: the exit
    /// honesty is the documented hypothesis (vsim honest -- UNPROBED),
    /// and the vocabulary is the marker hypothesis (UNPROBED). Until the
    /// first quartus box probes both, this gate is as double-keyed as
    /// the crate can make it.
    pub fn clean(&self) -> bool {
        !self.has_face() && self.exit == 0
    }
}

/// The error for blueprint filesets a quartus face does not carry. The
/// two faces carry different families -- the build face takes VHDL,
/// `VLOG`, and `SYSV` (the `.qsf` assignment lines); the sim face takes
/// VHDL only (the documented vcom recipe) -- so the error is labeled
/// with the face that refused the fileset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// The face that refused the fileset: `"build face"` or
    /// `"sim face"`.
    pub face: &'static str,
    /// Every refused entry, in blueprint order.
    pub entries: Vec<crate::blueprint::Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.face {
            "build face" => write!(
                f,
                "the quartus build face carries the VHDL, VLOG, and SYSV filesets only; \
                 not yet carried:"
            )?,
            _ => write!(
                f,
                "the quartus sim face carries the VHDL fileset only; not yet carried:"
            )?,
        }
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
    /// The blueprint carried filesets the face does not speak:
    /// usage-class, exit 2.
    NotCarried(NotCarried),
    /// A quartus-family binary could not be spawned at all: usage-class,
    /// exit 2.
    Spawn(String),
    /// The run-scoped state could not be reset: usage-class, exit 2.
    Library(String),
    /// A top failed to synthesize (the build face): findings, exit 1
    /// (the exit honesty is the documented hypothesis -- quartus_map
    /// honest nonzero on failure, UNPROBED).
    Synth {
        /// The top module the batch refused to synthesize.
        top: String,
        /// The captured log of the failed synth, verbatim.
        log: String,
    },
    /// A Questa compile-stage step (vlib, vmap, or the vcom deferral
    /// loop's residual) failed: findings, exit 1 (the exit honesty is
    /// the documented hypothesis, UNPROBED).
    Compile {
        /// The stage binary that refused: `vlib`, `vmap`, or `vcom`.
        stage: String,
        /// The captured log(s) of the failed step, verbatim.
        log: String,
    },
    /// At least one run log carried a failure face: findings, exit 1.
    Findings(Vec<RunOutcome>),
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings quartus or the judge
    /// reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_) => 2,
            DriveError::Synth { .. } | DriveError::Compile { .. } | DriveError::Findings(_) => 1,
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
            DriveError::Synth { top, .. } => write!(f, "the quartus_map synth failed for {top}"),
            DriveError::Compile { stage, .. } => write!(f, "the {stage} compile failed"),
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

/// Drive the quartus target end to end: blueprint, and then -- per face
/// -- the generated synthesis or the judged Questa runs.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths, the
/// generated `.qsf`, and Questa's library state land there. All progress
/// and diagnostics are printed here; the returned error only picks the
/// exit code. The mode picks the face: `Build` writes the `.qsf` and
/// runs one batch `quartus_map` per top (no run, no log, nothing
/// judged); `Sim` compiles the VHDL through vcom and judges each vsim
/// run.
pub fn drive(request: &Drive, mode: Mode) -> Result<(), DriveError> {
    let label = mode.label();
    let quartus = quartus_bin(std::env::var_os(QUARTUS_BIN_ENV).as_deref());

    if let Some(note) = plan_note(
        "quartus",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: quartus: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: quartus: error: {e}");
        DriveError::Blueprint(e)
    })?;

    match mode {
        Mode::Build => drive_build(request, &entries, &quartus, label),
        Mode::Sim => drive_sim(request, &entries, &quartus, label),
    }
}

/// The build face: the generated `.qsf` and one batch `quartus_map` per
/// top, exit honest, nothing judged. Carries both languages through
/// [`crate::blueprint::quartus_files`].
fn drive_build(
    request: &Drive,
    entries: &[crate::blueprint::Entry],
    quartus: &Path,
    label: &str,
) -> Result<(), DriveError> {
    // quartus_map must be spawnable before anything else, so a missing
    // binary is a loud usage error. `--version` is the documented
    // identity face, UNPROBED like everything on this drive.
    let mut probe = Command::new(quartus);
    probe.arg("--version");
    capture(QUARTUS_MAP_BIN, &mut probe).map_err(DriveError::Spawn)?;

    // Both languages ride the .qsf: VHDL entries as VHDL_FILE lines,
    // VLOG and SYSV entries as VERILOG_FILE lines, a custom fileset the
    // loud not-carried face.
    let (vhdl, verilog) = crate::blueprint::quartus_files(entries).map_err(|e| {
        eprintln!("orbit-drives: quartus: error: {e}");
        DriveError::NotCarried(e)
    })?;
    println!(
        "orbit-drives: quartus: {label}: carrying {} VHDL and {} Verilog file(s) into the \
         generated .qsf",
        vhdl.len(),
        verilog.len()
    );

    for top in &request.tops {
        println!(
            "orbit-drives: quartus: {label}: synthesizing {top} for {} (generated .qsf)",
            request.part.as_deref().unwrap_or("?")
        );
        let script = qsf_script(top, request.part.as_deref().unwrap_or(""), &vhdl, &verilog);
        let script_path = format!("{top}.qsf");
        fs::write(&script_path, &script).map_err(|e| {
            let message = format!("cannot write the project settings {script_path}: {e}");
            eprintln!("orbit-drives: quartus: error: {message}");
            DriveError::Library(message)
        })?;
        // The documented batch argv: fresh settings read and written
        // nowhere but the generated file, project = top, revision = top.
        // UNPROBED -- on the first quartus box the owner probes the real
        // read/write-settings behavior (see the honest limits).
        let mut synth = Command::new(quartus);
        synth
            .arg("--read_settings_files=off")
            .arg("--write_settings_files=off")
            .arg(top)
            .arg("-c")
            .arg(top);
        // The extras ride the drive's last tool argv verbatim: the
        // quartus_map argv on this face (there is no later step to carry
        // them, and a dropped argument would be a silent degradation).
        synth.args(&request.extra);
        let output = capture(QUARTUS_MAP_BIN, &mut synth).map_err(DriveError::Spawn)?;
        let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
        if !output.status.success() {
            eprint_block(&log);
            eprintln!("orbit-drives: quartus: error: the quartus_map synth failed for {top}");
            return Err(DriveError::Synth {
                top: top.clone(),
                log,
            });
        }
        println!("orbit-drives: quartus: {label}: {top}: clean");
    }

    Ok(())
}

/// The sim face: the bundled Questa. `vlib`/`vmap` open the work
/// library, vcom compiles the VHDL files in blueprint order through the
/// deferral loop (the documented order-sensitivity hypothesis), and each
/// top gets one judged `vsim -c` run. Carries the VHDL fileset only --
/// the documented recipe is vcom's alone; Verilog entries are the loud
/// not-carried face here (they ride the build face's `VERILOG_FILE`
/// lines).
fn drive_sim(
    request: &Drive,
    entries: &[crate::blueprint::Entry],
    quartus: &Path,
    label: &str,
) -> Result<(), DriveError> {
    let vlib = questa_bin(quartus, VLIB_BIN);
    let vmap = questa_bin(quartus, VMAP_BIN);
    let vcom = questa_bin(quartus, VCOM_BIN);
    let vsim = questa_bin(quartus, VSIM_BIN);

    // vsim must be spawnable before anything else, so a missing binary
    // is a loud usage error. `-version` is the ModelSim-family identity
    // face, UNPROBED like everything on this drive.
    let mut probe = Command::new(&vsim);
    probe.arg("-version");
    capture(VSIM_BIN, &mut probe).map_err(DriveError::Spawn)?;

    // Run-scoped library state: see `WORK_LIBRARY_DIR`. Every drive
    // starts from an empty library and a fresh mapping -- the
    // correctness-before-incrementality stance, DOCUMENTED-ONLY here.
    if let Err(e) = fs::remove_dir_all(WORK_LIBRARY_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale Questa state {WORK_LIBRARY_DIR}/: {e}");
            eprintln!("orbit-drives: quartus: error: {message}");
            return Err(DriveError::Library(message));
        }
    }
    if let Err(e) = fs::remove_file(MODELSIM_INI) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale Questa state {MODELSIM_INI}: {e}");
            eprintln!("orbit-drives: quartus: error: {message}");
            return Err(DriveError::Library(message));
        }
    }

    // The sim face carries the VHDL fileset only: the documented recipe
    // is vcom's alone, and a silent skip would leave Verilog files the
    // user believes were simulated. Every non-VHDL entry is the loud
    // not-carried face, in blueprint order.
    let foreign: Vec<crate::blueprint::Entry> = entries
        .iter()
        .filter(|entry| entry.fileset != "VHDL")
        .cloned()
        .collect();
    if !foreign.is_empty() {
        let e = NotCarried {
            face: "sim face",
            entries: foreign,
        };
        eprintln!("orbit-drives: quartus: error: {e}");
        return Err(DriveError::NotCarried(e));
    }
    let files: Vec<PathBuf> = entries.iter().map(|entry| entry.path.clone()).collect();

    // Open the work library and map it: `vlib work`, then `vmap work
    // work` (the ModelSim workflow's documented opening pair, UNPROBED).
    for (stage, binary, args) in [
        (VLIB_BIN, &vlib, vec![WORK.to_string()]),
        (VMAP_BIN, &vmap, vec![WORK.to_string(), WORK.to_string()]),
    ] {
        let mut command = Command::new(binary);
        command.args(&args);
        let output = capture(stage, &mut command).map_err(DriveError::Spawn)?;
        if !output.status.success() {
            let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
            eprint_block(&log);
            eprintln!("orbit-drives: quartus: error: the {stage} step failed");
            return Err(DriveError::Compile {
                stage: stage.to_string(),
                log,
            });
        }
    }

    println!(
        "orbit-drives: quartus: {label}: compiling {} VHDL file(s) in blueprint order with \
         the deferral loop (order-sensitive within {WORK}, DOCUMENTED-ONLY)",
        files.len()
    );
    let report = analyze_with_deferral(&files, |file| {
        let mut compile = Command::new(&vcom);
        compile.arg(STD).arg("-work").arg(WORK).arg(file);
        match capture(VCOM_BIN, &mut compile) {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(join_log(lossy(&output.stdout), lossy(&output.stderr))),
            Err(message) => Err(message),
        }
    });
    println!(
        "orbit-drives: quartus: {label}: {} file(s) compiled in {} pass(es)",
        report.analyzed.len(),
        report.passes
    );
    if !report.residual.is_empty() {
        eprintln!(
            "orbit-drives: quartus: error: {} VHDL file(s) never compiled after {} pass(es)",
            report.residual.len(),
            report.passes
        );
        let mut log = String::new();
        for residual in &report.residual {
            eprintln!(
                "orbit-drives: quartus: error: vcom failed: {}",
                residual.path.display()
            );
            eprint_block(&residual.stderr);
            log.push_str(&format!("{}:\n", residual.path.display()));
            log.push_str(&residual.stderr);
            if !residual.stderr.is_empty() && !residual.stderr.ends_with('\n') {
                log.push('\n');
            }
        }
        eprintln!("orbit-drives: quartus: error: the vcom compile failed");
        return Err(DriveError::Compile {
            stage: VCOM_BIN.to_string(),
            log,
        });
    }

    let mut findings = Vec::new();
    for top in &request.tops {
        let outcome = run_top(&vsim, top, &request.extra, label)?;
        if outcome.clean() {
            println!("orbit-drives: quartus: {label}: {top}: pass");
        } else if outcome.has_face() {
            println!(
                "orbit-drives: quartus: {label}: {top}: fail (the log carries a hypothesized \
                 quartus failure face)"
            );
            findings.push(outcome);
        } else {
            println!(
                "orbit-drives: quartus: {label}: {top}: fail (vsim exited {} without a \
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
            "orbit-drives: quartus: error: {} run(s) carried a finding",
            findings.len()
        );
        Err(DriveError::Findings(findings))
    }
}

/// Run one elaborated top and capture its log:
/// `vsim -c -do "run -all; quit" -L work work.<top> <extras>`. The
/// DOCUMENTED-ONLY invocation: the `-L work` pin rides before the top
/// (the ModelSim workflow's documented order, UNPROBED -- the
/// first-contact rule is the README's), and `-c` + the `-do` script make
/// the run batch. The log joins both streams; a run killed by a signal
/// has no exit code and [`RunOutcome::exit`] records -1.
fn run_top(
    vsim: &Path,
    top: &str,
    extra: &[String],
    label: &str,
) -> Result<RunOutcome, DriveError> {
    println!("orbit-drives: quartus: {label}: running {top}");
    let mut run = Command::new(vsim);
    run.arg("-c")
        .arg("-do")
        .arg("run -all; quit")
        .arg("-L")
        .arg(WORK)
        .arg(format!("{WORK}.{top}"));
    run.args(extra);
    let output = capture(VSIM_BIN, &mut run).map_err(DriveError::Spawn)?;
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

    // NO probed fixtures exist for this drive -- quartus is not
    // installed on the probing box. The fixtures below are the
    // HYPOTHESIZED shapes, spelled exactly as the ModelSim-family
    // rendering would write them; the tests pin the judge MECHANICS
    // (line-anchor, case-exactness) on those shapes, and the honest
    // limits name the first-contact rule that rewrites them from live
    // logs.

    // The hypothesized severity-failure face: line-start `Failure:`,
    // per the ModelSim-family rendering hypothesis.
    const FAILURE_LOG_HYPOTHESIS: &str = "\
# vsim work.tb_fail
run -all
Failure: the design is wrong
 quit
";

    // The hypothesized case drift and mid-line negatives: never judge,
    // same as vivado's probed pins.
    const DRIFT_LOG_HYPOTHESIS: &str = "\
FATAL: the design is wrong
the sim says: Error: retrying
";

    // The hypothesized clean run: the banner and the quit, no marker.
    const CLEAN_LOG_HYPOTHESIS: &str = "\
# vsim work.tb_clean
run -all
sim done
 quit
";

    #[test]
    fn a_hypothesized_failure_face_judges_fail_at_line_start() {
        // The judge mechanics on the hypothesized shape: a line LEADING
        // with the marker convicts.
        let outcome = RunOutcome {
            top: "tb_fail".to_string(),
            exit: 1,
            log: FAILURE_LOG_HYPOTHESIS.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn the_hypothesized_drifts_never_judge() {
        // The negative pins ride vivado's probed rule: a case drift at
        // line start and a marker mid-line never judge, even on the
        // hypothesized shapes.
        let outcome = RunOutcome {
            top: "tb_drift".to_string(),
            exit: 0,
            log: DRIFT_LOG_HYPOTHESIS.to_string(),
        };
        assert!(!outcome.has_face());
        assert!(outcome.clean());
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
    fn a_hypothesized_clean_run_passes_the_gate() {
        // The hypothesized clean run passes: no marker, exit 0.
        let outcome = RunOutcome {
            top: "tb_clean".to_string(),
            exit: 0,
            log: CLEAN_LOG_HYPOTHESIS.to_string(),
        };
        assert!(outcome.clean());
    }

    #[test]
    fn a_nonzero_exit_convicts_without_a_face() {
        // The exit is a witness on its own -- even on the documented
        // hypothesis where no face is carried yet.
        let outcome = RunOutcome {
            top: "tb_crash".to_string(),
            exit: 1,
            log: CLEAN_LOG_HYPOTHESIS.to_string(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn the_qsf_carries_the_device_the_top_and_every_file() {
        // The generated .qsf shape: DEVICE from --part, TOP_LEVEL_ENTITY
        // from --top, one VHDL_FILE / VERILOG_FILE line per blueprint
        // file.
        let script = qsf_script(
            "top",
            "xc7z010clg400-1",
            &[PathBuf::from("./top.vhd"), PathBuf::from("./gate.vhd")],
            &[PathBuf::from("./vgate.v")],
        );
        assert_eq!(
            script,
            "set_global_assignment -name DEVICE xc7z010clg400-1\n\
             set_global_assignment -name TOP_LEVEL_ENTITY top\n\
             set_global_assignment -name VHDL_FILE ./top.vhd\n\
             set_global_assignment -name VHDL_FILE ./gate.vhd\n\
             set_global_assignment -name VERILOG_FILE ./vgate.v\n"
        );
    }

    #[test]
    fn the_binaries_resolve_from_the_hook_and_the_questa_sibling() {
        // QUARTUS_BIN wins, an empty value counts as absent, the bare
        // name is the fallback.
        assert_eq!(
            quartus_bin(Some("/opt/intelFPGA/21.1/quartus/bin/quartus_map".as_ref())),
            PathBuf::from("/opt/intelFPGA/21.1/quartus/bin/quartus_map")
        );
        assert_eq!(
            quartus_bin(Some("".as_ref())),
            PathBuf::from(QUARTUS_MAP_BIN)
        );
        assert_eq!(quartus_bin(None), PathBuf::from(QUARTUS_MAP_BIN));

        // A resolved install carries the bundled Questa as the
        // questa_fse/bin sibling; a bare PATH name falls back to the
        // PATH lookup.
        let dir = std::env::temp_dir().join(format!("orbit-drives-qrt-res-{}", std::process::id()));
        let install_bin = dir.join("quartus").join("bin");
        let questa = dir.join("quartus").join(QUESTA_SUBDIR).join("bin");
        fs::create_dir_all(&install_bin).unwrap();
        fs::create_dir_all(&questa).unwrap();
        fs::write(install_bin.join("quartus_map"), "").unwrap();
        fs::write(questa.join("vcom"), "").unwrap();
        let installed = install_bin.join("quartus_map");
        assert_eq!(questa_bin(&installed, "vcom"), questa.join("vcom"));
        assert_eq!(questa_bin(&installed, VSIM_BIN), PathBuf::from(VSIM_BIN));
        assert_eq!(
            questa_bin(Path::new("quartus_map"), "vcom"),
            PathBuf::from("vcom")
        );
        fs::remove_dir_all(&dir).unwrap();
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
                face: "sim face",
                entries: Vec::new(),
            })
            .exit_code(),
            2
        );
        assert_eq!(
            DriveError::Spawn("quartus_map: no such file".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Library("cannot wipe".to_string()).exit_code(),
            2
        );
        assert_eq!(
            DriveError::Synth {
                top: "top".to_string(),
                log: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(
            DriveError::Compile {
                stage: "vcom".to_string(),
                log: String::new(),
            }
            .exit_code(),
            1
        );
        assert_eq!(DriveError::Findings(Vec::new()).exit_code(), 1);
    }

    #[test]
    fn the_not_carried_error_names_its_face_and_filesets() {
        let build = NotCarried {
            face: "build face",
            entries: Vec::new(),
        };
        assert!(build
            .to_string()
            .starts_with("the quartus build face carries the VHDL, VLOG, and SYSV filesets only"));
        let sim = NotCarried {
            face: "sim face",
            entries: Vec::new(),
        };
        assert!(sim
            .to_string()
            .starts_with("the quartus sim face carries the VHDL fileset only"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Compile {
            stage: "vcom".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the vcom compile failed");
        let e = DriveError::Synth {
            top: "top".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the quartus_map synth failed for top");
        let e = DriveError::Findings(Vec::new());
        assert_eq!(e.to_string(), "0 run(s) carried a finding");
    }

    #[test]
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("quartus", Some("tsv")).is_none());
        assert!(plan_note("quartus", None).is_none());
        let note = plan_note("quartus", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: quartus: note:"));
    }
}
