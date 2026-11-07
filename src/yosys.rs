//! The yosys target drive: the orbit blueprint in, a judged verdict out.
//!
//! ONE orbit target shares this drive: `yosys` (the synthesis face --
//! read the blueprint's Verilog, `synth`, write the netlist). There is no
//! sim face and no judged run: yosys is a synthesis engine, it has no
//! runtime to drive, so the drive's whole output is the per-top netlist
//! and its exit. The drive reads the blueprint orbit wrote for the target
//! (the TSV plan: one `fileset<TAB>library<TAB>filepath` line per
//! collected file) and, per requested top, spawns ONE script-driven
//! process -- `yosys -p "read_verilog <files>; synth -top <top>;
//! write_verilog netlists/<top>.v"` -- whose log is judged against
//! yosys's probed failure vocabulary and whose exit gates the verdict.
//!
//! There is NO deferral loop here, on probed grounds: yosys reads all
//! files into one design and elaborates at `synth` time, so it is
//! order-free (a consumer-before-provider pair synthesized clean, probed
//! live on Yosys 0.52, 2026-11-07) -- one script per top, no passes. The
//! script validates its top at `synth -top` (`ERROR: Module \`nosuch' not
//! found!`, exit 1, probed), so an unknown top fails its own run loudly on
//! this face.
//!
//! The judge is the second witness, not the first: yosys's exit is honest
//! so far as probed -- every probed failure (a syntax error, an unknown
//! top, an unknown `-p` command, a failed netlist write, an instantiation
//! whose provider was never read) exits 1, because a script error aborts
//! the whole `-p` script -- so the exit convicts on every failure seen,
//! and the `ERROR:` face in the log convicts regardless. Either witness
//! convicts, everywhere in this crate.

use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::blueprint::{parse_blueprint, plan_note, verilog_only, BlueprintError, Drive};
use crate::contracts::{self, Tool};
use crate::iverilog::NotCarried;
use crate::proc::{capture, eprint_block, join_log, lossy};

/// The yosys binary the drive spawns, resolved on PATH. The orbit target
/// contract carries no binary-path field.
pub const YOSYS_BIN: &str = "yosys";

/// The netlist directory the drive writes every top into
/// (`netlists/<top>.v`). It is the drive's payload: the probed write
/// (`write_verilog netlists/<top>.v`) lands there, and the synthesis exit
/// is honest, so a netlist present after a clean drive is a netlist that
/// yosys wrote this drive.
///
/// Orbit's output directory persists across runs, and the drive wipes the
/// stale directory first -- the same correctness-before-incrementality
/// stance as the work libraries and the model directory. Probed honestly:
/// the wipe is symmetry here, not load-bearing -- a failed synth exits 1
/// before its netlist could be mistaken for fresh, so a stale netlist can
/// never survive into a reported-clean drive; it exists so every drive
/// starts from a known-clean payload.
pub const NETLIST_DIR: &str = "netlists";

/// Assemble the one `-p` script for a top: every Verilog file read into
/// one design, `synth -top <top>`, the netlist written under
/// [`NETLIST_DIR`]. yosys runs the commands left to right and aborts the
/// whole script on the first error (exit 1, probed), so the netlist is
/// written only on a fully clean synth.
///
/// The file paths ride the script verbatim, whitespace-split like any
/// yosys command: a blueprint path carrying a space would split into two
/// arguments and fail the read loudly. That is a documented v1 limit, not
/// something the drive papers over.
pub fn synth_script(top: &str, files: &[PathBuf]) -> String {
    let mut script = String::new();
    script.push_str("read_verilog");
    for file in files {
        script.push(' ');
        script.push_str(&file.to_string_lossy());
    }
    script.push_str("; synth -top ");
    script.push_str(top);
    script.push_str("; write_verilog ");
    script.push_str(NETLIST_DIR);
    script.push('/');
    script.push_str(top);
    script.push_str(".v");
    script
}

/// The record of one top's synthesis: what yosys exited and what it
/// wrote. The netlist itself is not read back: the exit and the judged log
/// are the verdict, the file is the payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynthOutcome {
    /// The top module that was synthesized.
    pub top: String,
    /// yosys's exit code, recorded but secondary: it never flips the
    /// verdict alone. A `-1` records a run that died by signal rather
    /// than exiting.
    pub exit: i32,
    /// The captured log: yosys writes its whole synthesis log to stdout
    /// and its failure faces to stderr (probed); the drive joins the
    /// streams, so the face reads either way.
    pub log: String,
}

impl SynthOutcome {
    /// `true` when the log carries a probed yosys failure face.
    pub fn has_face(&self) -> bool {
        !contracts::judge(&Tool::Yosys, &self.log)
    }

    /// The gate: `true` when the synth is clean -- no probed failure face
    /// in the log AND yosys exited 0.
    ///
    /// Either witness convicts. The exit is honest so far as probed
    /// (every probed failure exits 1), so it convicts on every failure
    /// seen; the face stays the gate because the exit cannot name the
    /// face, and a tool update that stopped printing what it exits for
    /// must not silently pass.
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
    /// yosys could not be spawned at all: usage-class, exit 2.
    Spawn(String),
    /// The stale netlist directory could not be reset: usage-class,
    /// exit 2.
    Library(String),
    /// A top failed to synthesize: findings, exit 1 (yosys's exit is
    /// honest there -- every probed failure exits 1 -- and the judge is
    /// the second witness on the same log).
    Synth {
        /// The top module yosys refused to synthesize.
        top: String,
        /// The captured log of the failed synth, verbatim.
        log: String,
    },
}

impl DriveError {
    /// The exit code the variant maps to: 2 for usage-class problems the
    /// drive never got past, 1 for findings yosys or the judge reported.
    pub fn exit_code(&self) -> u8 {
        match self {
            DriveError::Blueprint(_)
            | DriveError::NotCarried(_)
            | DriveError::Spawn(_)
            | DriveError::Library(_) => 2,
            DriveError::Synth { .. } => 1,
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
            DriveError::Synth { top, .. } => write!(f, "the yosys synth failed for {top}"),
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

/// Drive the yosys target end to end: blueprint in, per-top netlists out.
///
/// Orbit spawns this process with the working directory set to the
/// target's output directory, so the blueprint's relative paths and the
/// `netlists/` payload land there. All progress and diagnostics are
/// printed here; the returned error only picks the exit code.
///
/// There is no deferral loop: yosys reads all files into one design and
/// elaborates at `synth` time, order-free (probed live on Yosys 0.52,
/// 2026-11-07). Each requested top gets its own script-driven process --
/// yosys's `synth` consumes the design it runs on, so a second top could
/// not synthesize over the first one's mapped netlist -- and each run
/// validates its own top.
pub fn drive(request: &Drive) -> Result<(), DriveError> {
    // yosys must be spawnable before anything else, so a missing binary
    // is a loud usage error instead of a silent synth failure. `-V` is
    // yosys's identity flag (probed live; there is no `--version`).
    capture(YOSYS_BIN, Command::new(YOSYS_BIN).arg("-V")).map_err(DriveError::Spawn)?;

    // Run-scoped payload state: see `NETLIST_DIR`. Every drive starts
    // from a known-clean payload (probed honestly: the wipe is symmetry
    // here -- a failed synth exits before its netlist could be mistaken
    // for fresh).
    if let Err(e) = fs::remove_dir_all(NETLIST_DIR) {
        if e.kind() != std::io::ErrorKind::NotFound {
            let message = format!("cannot wipe the stale netlist directory {NETLIST_DIR}/: {e}");
            eprintln!("orbit-drives: yosys: error: {message}");
            return Err(DriveError::Library(message));
        }
    }
    if let Err(e) = fs::create_dir_all(NETLIST_DIR) {
        let message = format!("cannot create the netlist directory {NETLIST_DIR}/: {e}");
        eprintln!("orbit-drives: yosys: error: {message}");
        return Err(DriveError::Library(message));
    }

    if let Some(note) = plan_note(
        "yosys",
        std::env::var("ORBIT_BLUEPRINT_PLAN").ok().as_deref(),
    ) {
        eprintln!("{note}");
    }

    let text = fs::read_to_string(&request.blueprint).map_err(|source| {
        let e = BlueprintError::Read {
            path: request.blueprint.clone(),
            source,
        };
        eprintln!("orbit-drives: yosys: error: {e}");
        DriveError::Blueprint(e)
    })?;
    let entries = parse_blueprint(&text).map_err(|e| {
        let e = BlueprintError::Parse(e);
        eprintln!("orbit-drives: yosys: error: {e}");
        DriveError::Blueprint(e)
    })?;
    // The fileset door is the Verilog drives' shared one: yosys joins
    // iverilog and verilator on the same VLOG and SYSV filesets -- the
    // filter lives with the blueprint machinery, its message with the
    // first Verilog drive. yosys is a Verilog synthesis engine: VHDL
    // rides nothing here, the same loud not-carried face as on verilator.
    let files = verilog_only(&entries).map_err(|e| {
        eprintln!("orbit-drives: yosys: error: {e}");
        DriveError::NotCarried(e)
    })?;

    println!(
        "orbit-drives: yosys: build: synthesizing {} Verilog file(s), order-free \
         (probed live 2026-11-07)",
        files.len()
    );

    for top in &request.tops {
        println!("orbit-drives: yosys: build: synthesizing {top} into {NETLIST_DIR}");
        let outcome = synth_top(top, &files, &request.extra)?;
        if outcome.clean() {
            println!("orbit-drives: yosys: build: {top}: clean ({NETLIST_DIR}/{top}.v written)");
        } else if outcome.has_face() {
            eprint_block(&outcome.log);
            eprintln!(
                "orbit-drives: yosys: error: the yosys synth failed for {top} (the log \
                 carries a probed yosys failure face)"
            );
            return Err(DriveError::Synth {
                top: top.clone(),
                log: outcome.log,
            });
        } else {
            eprint_block(&outcome.log);
            eprintln!(
                "orbit-drives: yosys: error: the yosys synth failed for {top} (yosys \
                 exited {} without a judged face)",
                outcome.exit
            );
            return Err(DriveError::Synth {
                top: top.clone(),
                log: outcome.log,
            });
        }
    }

    Ok(())
}

/// One synth: `yosys -p "<script>" <extras>` with the script of
/// [`synth_script`]. The extras ride the synth argv verbatim -- there is
/// no other step to carry them, and silently dropping a user argument
/// would be a degradation; an option yosys does not speak is yosys's own
/// loud rejection (`Error parsing options: Option 'x' does not exist`,
/// exit 1, probed -- notably a face without the `ERROR:` marker, so
/// there the exit witness convicts alone). The log joins both streams
/// (yosys's log is stdout, its failure faces stderr, probed); a run
/// killed by a signal has no exit code and [`SynthOutcome::exit`]
/// records -1.
fn synth_top(top: &str, files: &[PathBuf], extra: &[String]) -> Result<SynthOutcome, DriveError> {
    let script = synth_script(top, files);
    let mut synth = Command::new(YOSYS_BIN);
    synth.arg("-p").arg(&script);
    synth.args(extra);
    let output = capture(YOSYS_BIN, &mut synth).map_err(DriveError::Spawn)?;
    let log = join_log(lossy(&output.stdout), lossy(&output.stderr));
    Ok(SynthOutcome {
        top: top.to_string(),
        exit: output.status.code().unwrap_or(-1),
        log,
    })
}

/// The path of one top's netlist payload: the file a clean drive leaves
/// behind. Kept beside [`NETLIST_DIR`] so the live shapes can assert on
/// the same shape the drive writes.
pub fn netlist_path(top: &str) -> PathBuf {
    Path::new(NETLIST_DIR).join(format!("{top}.v"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::MalformedLine;

    // The probed syntax face (yosys 0.52, 2026-11-07): the ERROR marker
    // rides mid-line behind the source location, on stderr. Byte-for-byte
    // as recorded live.
    const SYNTAX_LOG: &str = "\
1. Executing Verilog-2005 frontend: broken.v
broken.v:3: ERROR: syntax error, unexpected TOK_ENDMODULE
";

    // The probed unknown-top face (`synth -top nosuch`, exit 1, stderr):
    // line-start, backquoted module name.
    const UNKNOWN_TOP_LOG: &str = "\
ERROR: Module `nosuch' not found!
";

    // The probed unknown-command face (`yosys -p "read_verilog gate.v;
    // frobnicate"`, exit 1, stderr): line-start.
    const UNKNOWN_COMMAND_LOG: &str = "\
ERROR: No such command: frobnicate (type 'help' for a command overview)
";

    // The probed failed-write face (`write_verilog` into a directory that
    // does not exist, exit 1, stderr): line-start.
    const FAILED_WRITE_LOG: &str = "\
ERROR: Can't open output file `netlists-missing/x.v' for writing: No such file or directory
";

    // The probed missing-provider face (`synth` over a design whose
    // instantiation was never read, exit 1, stderr): line-start, with the
    // backquoted hierarchical names.
    const NOT_PART_OF_DESIGN_LOG: &str = "\
ERROR: Module `\\gate' referenced in module `\\top' in cell `\\u_gate' is not part of the design.
";

    // A clean synth's log tail (yosys 0.52, probed): the banner, the
    // command echo, the `End of script` summary -- no ERROR marker
    // anywhere.
    const CLEAN_LOG: &str = "\
-- Running command `read_verilog gate.v top.v; synth -top top; write_verilog netlists/top.v' --

2.27. Executing OPT pass (performing simple optimizations).
End of script. Logfile hash: 8f1ba39a8d, CPU: user 0.02s system 0.01s, MEM: 17.06 MB peak
";

    #[test]
    fn every_probed_face_judges_fail() {
        // The five probed failure faces judge FAIL: the syntax face is
        // mid-line behind its source location, the four script faces are
        // line-start. All carry the same uppercase `ERROR:` marker.
        for log in [
            SYNTAX_LOG,
            UNKNOWN_TOP_LOG,
            UNKNOWN_COMMAND_LOG,
            FAILED_WRITE_LOG,
            NOT_PART_OF_DESIGN_LOG,
        ] {
            assert!(
                !contracts::judge(&Tool::Yosys, log),
                "the probed face did not judge its log: {log}"
            );
        }
    }

    #[test]
    fn a_clean_log_passes_the_gate() {
        // A clean synth log judges PASS: the banner and the summary carry
        // no failure face.
        assert!(contracts::judge(&Tool::Yosys, CLEAN_LOG));
    }

    #[test]
    fn a_lowercase_drift_never_judges() {
        // yosys spells its marker in uppercase, case-exactly: a lowercase
        // drift is not one of its faces and never judges a log.
        assert!(contracts::judge(
            &Tool::Yosys,
            "broken.v:3: error: syntax error, unexpected TOK_ENDMODULE\n"
        ));
    }

    #[test]
    fn a_failed_synth_is_a_finding_on_both_witnesses() {
        // The probed stop: face AND exit 1. Either witness convicts; here
        // both do.
        let outcome = SynthOutcome {
            top: "broken".to_string(),
            exit: 1,
            log: SYNTAX_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_face_convicts_an_exit_of_zero() {
        // The judge's reason to exist as the second witness: a tool
        // update that printed the face but exited 0 must not pass. Probed
        // yosys never does this (every probed failure exits 1); the gate
        // holds the door anyway.
        let outcome = SynthOutcome {
            top: "broken".to_string(),
            exit: 0,
            log: SYNTAX_LOG.to_string(),
        };
        assert!(outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_nonzero_exit_convicts_without_a_face() {
        // The crash door: a killed or crashed yosys can exit nonzero
        // without writing any probed face, and a clean log must not
        // green-light it. Either witness convicts.
        let outcome = SynthOutcome {
            top: "top".to_string(),
            exit: 137,
            log: CLEAN_LOG.to_string(),
        };
        assert!(!outcome.has_face());
        assert!(!outcome.clean());
    }

    #[test]
    fn a_signal_death_is_a_finding_on_its_minus_one_exit() {
        // A run killed by a signal has no exit code; the drive records
        // -1 (see `SynthOutcome.exit`), and -1 convicts.
        let outcome = SynthOutcome {
            top: "top".to_string(),
            exit: -1,
            log: String::new(),
        };
        assert!(!outcome.clean());
    }

    #[test]
    fn the_script_reads_every_file_then_synths_then_writes() {
        // The probed script shape: one read_verilog carrying every file,
        // `synth -top`, the netlist under netlists/. yosys runs the
        // commands left to right and aborts on the first error, so the
        // write only happens on a clean synth.
        let script = synth_script(
            "top",
            &[PathBuf::from("./top.v"), PathBuf::from("./gate.v")],
        );
        assert_eq!(
            script,
            "read_verilog ./top.v ./gate.v; synth -top top; write_verilog netlists/top.v"
        );
    }

    #[test]
    fn the_netlist_path_rides_the_netlist_directory() {
        assert_eq!(netlist_path("top"), PathBuf::from("netlists/top.v"));
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
            DriveError::Spawn("yosys: no such file".to_string()).exit_code(),
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
    }

    #[test]
    fn the_plan_note_fires_only_off_tsv() {
        assert!(plan_note("yosys", Some("tsv")).is_none());
        assert!(plan_note("yosys", None).is_none());
        let note = plan_note("yosys", Some("json")).unwrap();
        assert!(
            note.contains("json"),
            "the note should name the plan: {note}"
        );
        assert!(note.starts_with("orbit-drives: yosys: note:"));
    }

    #[test]
    fn drive_errors_render_the_words_the_drive_prints() {
        let e = DriveError::Synth {
            top: "top".to_string(),
            log: String::new(),
        };
        assert_eq!(e.to_string(), "the yosys synth failed for top");
    }
}
