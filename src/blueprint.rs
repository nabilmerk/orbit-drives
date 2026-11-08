//! The blueprint machinery every drive starts from: the TSV blueprint
//! parse (orbit's plan format), the argv-level request view ([`Drive`],
//! [`Mode`]), the `$ORBIT_BLUEPRINT_PLAN` note, and the per-language
//! fileset filters.
//!
//! [`crate::ghdl`] carried all of this with the first drive, and every
//! later drive borrowed it from there. It lives here now, owned by no
//! tool: the parse, the request, and the filters speak the blueprint's
//! and the filesets' names, not a binary's. What stayed behind in
//! [`crate::ghdl`] is the deferral loop -- the other piece of shared
//! machinery, still housed with the drive that first carried it.

use std::fmt;
use std::path::{Path, PathBuf};

/// The custom recursive fileset the shipped target entries carry
/// (`config.toml`: `fileset.ALLHDL = { patterns = ["**/*.vhd",
/// "**/*.vhdl", "**/*.v", "**/*.sv"], recursive = true }`) -- the FULL
/// HDL file set of every project in the dependency graph, riding every
/// blueprint regardless of orbit's reference closure (probed live on
/// orbit 2026-11-08: a dependency project's unreferenced source rides
/// the blueprint under ALLHDL where the bare planning drops it -- the
/// default-bound entities, whose only link is a component declaration
/// no reference walk can see). The blueprint through a shipped target
/// carries ALLHDL rows INSTEAD of the built-in VHDL/VLOG/SYSV rows (the
/// custom fileset replaces the built-ins in the plan), so every drive
/// routes an ALLHDL row by its file EXTENSION (`.vhd`/`.vhdl` to the
/// VHDL faces, `.v`/`.sv` to the Verilog faces) -- see
/// [`is_vhdl_row`] / [`is_verilog_row`].
pub const ALL_HDL_FILESET: &str = "ALLHDL";

/// One entry of the blueprint: a fileset, a library, and the file's path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The 1-based blueprint line the entry came from.
    pub line: usize,
    /// The fileset orbit filed the file under (`VHDL`, `VLOG`, `SYSV`, or
    /// a custom fileset).
    pub fileset: String,
    /// The orbit library the file belongs to -- the owning project's HDL
    /// library name (orbit defaults it to the project's own name, not to
    /// any tool's `work`). Consumed verbatim by the VHDL drives' per-file
    /// `--work` pin (the §25 multi-library face, probed live on ghdl
    /// 6.0.0 and nvc 1.23.0) and by the vivado drive where its tools
    /// serve it -- the generated script's `read_vhdl -library` lines on
    /// the build face and the sim face's `xsim.ini` mapped-library lines
    /// (the single-work sim ruling, 2026-11-08: the sim compile rides
    /// ONE `work` library, xsim's LRM-strict default binding being
    /// argv-incurable); the other drives still compile every file into
    /// their tool's single default library.
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

/// What the drive runs: the build face stops after the elaborations, the
/// sim face adds the judged run. Every carried tool's drive splits this
/// way -- one [`Mode`], two orbit target entries per tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The `*-build` targets: analyze the blueprint in order, elaborate
    /// each top, exit honest. NO simulation: there is no run, no run log,
    /// and nothing judged. Analysis and elaboration need no log gate
    /// (both exit nonzero on a broken design, probed live on ghdl 6.0.0
    /// and nvc 1.23.0), so on this face the exit is the only witness --
    /// the judge exists for the sim face, whose ghdl exit lies.
    Build,
    /// The `ghdl` / `nvc` targets: elaborate and run each top, gating
    /// every run log on the probed vocabulary AND the exit. Either
    /// witness convicts.
    Sim,
}

impl Mode {
    /// The word the drive stamps into its progress lines, so an orbit log
    /// says which face ran.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Build => "build",
            Mode::Sim => "sim",
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
    /// The extra tokens riding the drive's last tool argv: orbit appends
    /// the user's extras with the `--` separator stripped, and the argv
    /// parser passes through every token the drive does not own.
    pub extra: Vec<String>,
    /// The FPGA part the vivado and quartus build faces synthesize for
    /// (`--part`, e.g. `xc7z010clg400-1`). Only those two build faces
    /// read it -- the argv parser rejects the flag on every other face,
    /// so every other drive sees `None` here.
    pub part: Option<String>,
    /// The python test module the cocotb face drives (`--test-module`,
    /// a path to a `.py` file carrying `@cocotb.test()` coroutines).
    /// Only the cocotb face reads it, and only there is it REQUIRED --
    /// the argv parser rejects the flag on every other face, so every
    /// other drive sees `None` here.
    pub test_module: Option<String>,
}

/// The note for a blueprint plan the drive does not carry: orbit exports
/// `$ORBIT_BLUEPRINT_PLAN` as the plan it wrote, and the drive speaks the
/// TSV plan only. Loud but non-fatal -- the file is still read as TSV
/// lines, and a non-TSV file fails the parse loudly right after. `tool`
/// is the driving binary's name, stamped into the note so an orbit log
/// says which drive wrote it.
pub(crate) fn plan_note(tool: &str, plan: Option<&str>) -> Option<String> {
    match plan {
        Some(plan) if plan != "tsv" => Some(format!(
            "orbit-drives: {tool}: note: $ORBIT_BLUEPRINT_PLAN is {plan:?}; the drive \
             carries the TSV plan only, reading the blueprint as TSV lines"
        )),
        _ => None,
    }
}

/// The error for blueprint filesets the VHDL drives do not carry.
///
/// TWO drives share this error -- the ghdl drive and the nvc drive both
/// ride [`vhdl_only`] -- so the message names the drive family, never
/// one drive's name: an `orbit-drives nvc` user on a Verilog blueprint
/// must not be told the refusal is ghdl's. The shape is the
/// [`crate::iverilog::NotCarried`] precedent, where the same
/// two-shares-one situation wrote the same drive-neutral plural.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotCarried {
    /// Every non-VHDL entry, in blueprint order.
    pub entries: Vec<Entry>,
}

impl fmt::Display for NotCarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the VHDL drives carry the VHDL fileset (ALLHDL rows by their .vhd/.vhdl extension) only; not yet carried:"
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

/// The VHDL entries of the blueprint, in blueprint order, libraries
/// attached: the shape the VHDL drives' multi-library faces consume --
/// each file analyzed into the library its own row declares (`--work=
/// <entry.library>`), so the deferral loop stays per-file and cross-library
/// references defer across libraries exactly as they do within one.
/// [`ALL_HDL_FILESET`] rows ride by their extension (see
/// [`is_vhdl_row`]).
///
/// Whether one blueprint row is a VHDL row for the drives' routing: the
/// built-in `VHDL` fileset, or an [`ALL_HDL_FILESET`] row whose file
/// carries a `.vhd`/`.vhdl` extension (the shipped targets ride ALLHDL
/// rows instead of the built-ins, and the extension is the only
/// language signal a merged fileset row carries).
pub fn is_vhdl_row(entry: &Entry) -> bool {
    entry.fileset == "VHDL" || (entry.fileset == ALL_HDL_FILESET && vhdl_extension(&entry.path))
}

/// Whether one blueprint row is a Verilog/SV row for the drives'
/// routing: the built-in `VLOG`/`SYSV` filesets, or an
/// [`ALL_HDL_FILESET`] row whose file carries a `.v`/`.sv` extension.
pub fn is_verilog_row(entry: &Entry) -> bool {
    entry.fileset == "VLOG"
        || entry.fileset == "SYSV"
        || (entry.fileset == ALL_HDL_FILESET && verilog_extension(&entry.path))
}

/// Whether `path` carries a VHDL extension (`.vhd`/`.vhdl`, lowercase --
/// orbit's fileset globs are case-sensitive too).
fn vhdl_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("vhd") | Some("vhdl")
    )
}

/// Whether `path` carries a Verilog/SV extension (`.v`/`.sv`,
/// lowercase).
fn verilog_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("v") | Some("sv")
    )
}

/// Every non-VHDL entry -- `VLOG`, `SYSV`, an ALLHDL row with a Verilog
/// extension, a custom fileset -- is rejected
/// loudly, through the same shared [`NotCarried`] refusal as
/// [`vhdl_only`]: mixed-language designs stay outside the VHDL drives'
/// contract, and a silent skip would leave files the user believes were
/// simulated.
pub fn vhdl_entries(entries: &[Entry]) -> Result<Vec<Entry>, NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| !is_vhdl_row(entry))
        .cloned()
        .collect();
    if foreign.is_empty() {
        Ok(entries.to_vec())
    } else {
        Err(NotCarried { entries: foreign })
    }
}

/// The VHDL files of the blueprint, in blueprint order.
///
/// Every non-VHDL entry -- `VLOG`, `SYSV`, a custom fileset -- is rejected
/// loudly: mixed-language designs stay outside the VHDL drives' contract,
/// and a silent skip would leave files the user believes were simulated.
pub fn vhdl_only(entries: &[Entry]) -> Result<Vec<PathBuf>, NotCarried> {
    vhdl_entries(entries).map(|carried| carried.iter().map(|entry| entry.path.clone()).collect())
}

/// The Verilog files of the blueprint, in blueprint order.
///
/// Every non-Verilog entry -- `VHDL`, a custom fileset -- is rejected
/// loudly: iverilog and verilator are Verilog/SV-native, and a silent
/// skip would leave files the user believes were compiled. The blueprint
/// order is carried but not load-bearing: iverilog's single compile is
/// order-free (probed live on Icarus Verilog 12.0, 2026-11-07).
///
/// The error type is the Verilog drives' shared one,
/// [`crate::iverilog::NotCarried`] -- its message names the filesets, not
/// one drive, because two drives carry it.
pub fn verilog_only(entries: &[Entry]) -> Result<Vec<PathBuf>, crate::iverilog::NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| !is_verilog_row(entry))
        .cloned()
        .collect();
    if foreign.is_empty() {
        Ok(entries.iter().map(|entry| entry.path.clone()).collect())
    } else {
        Err(crate::iverilog::NotCarried { entries: foreign })
    }
}

/// The VHDL entries and the Verilog files of the blueprint, in blueprint
/// order: the pair the vivado faces consume -- the VHDL entries with
/// their blueprint libraries ATTACHED (the generated synthesis script
/// reads each back with `read_vhdl -library <entry.library>`, and the
/// sim face maps each non-`work` library to the merged one in its
/// `xsim.ini`; the sim COMPILE rides single-work since the 2026-11-08
/// ruling -- the column has no compile face there), the `VLOG` and
/// `SYSV` entries as bare paths (Verilog carries no library clauses --
/// xvlog lands them all in `work`).
///
/// Two carried tools speak both languages, so this is the filter shape
/// that carries two fileset families. Every OTHER entry -- a custom
/// fileset -- is rejected loudly: a silent skip would leave files the
/// user believes were compiled.
///
/// The error type is [`crate::vivado::NotCarried`] -- its message names
/// the three carried filesets, not one face, because both vivado faces
/// carry it. The quartus build face takes the paths-only pair through
/// [`quartus_files`], whose error message names quartus's faces.
pub fn vivado_files(
    entries: &[Entry],
) -> Result<(Vec<Entry>, Vec<PathBuf>), crate::vivado::NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| !is_vhdl_row(entry) && !is_verilog_row(entry))
        .cloned()
        .collect();
    if foreign.is_empty() {
        let mut vhdl = Vec::new();
        let mut verilog = Vec::new();
        for entry in entries {
            if is_vhdl_row(entry) {
                vhdl.push(entry.clone());
            } else {
                verilog.push(entry.path.clone());
            }
        }
        Ok((vhdl, verilog))
    } else {
        Err(crate::vivado::NotCarried { entries: foreign })
    }
}

/// The VHDL and Verilog files of the blueprint, in blueprint order: the
/// pair the quartus-build face writes into the generated `.qsf` --
/// VHDL entries as `VHDL_FILE` lines, `VLOG` and `SYSV` entries as
/// `VERILOG_FILE` lines. Every OTHER entry -- a custom fileset -- is
/// rejected loudly, like on every other face. (The quartus SIM face
/// carries the VHDL fileset only -- the documented vcom recipe -- and
/// rides [`vhdl_only`]'s shape through [`crate::quartus::NotCarried`],
/// whose message names the quartus face.)
///
/// The error type is [`crate::quartus::NotCarried`], labeled with the
/// face that refused the fileset.
pub fn quartus_files(
    entries: &[Entry],
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), crate::quartus::NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| !is_vhdl_row(entry) && !is_verilog_row(entry))
        .cloned()
        .collect();
    if foreign.is_empty() {
        let mut vhdl = Vec::new();
        let mut verilog = Vec::new();
        for entry in entries {
            if is_vhdl_row(entry) {
                vhdl.push(entry.path.clone());
            } else {
                verilog.push(entry.path.clone());
            }
        }
        Ok((vhdl, verilog))
    } else {
        Err(crate::quartus::NotCarried {
            face: "build face",
            entries: foreign,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_drives_carry_vhdl_only_and_name_the_rest() {
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
    fn the_allhdl_rows_ride_by_their_extension() {
        // The shipped targets' custom recursive fileset (config.toml's
        // `fileset.ALLHDL`, the owner's 2026-11-08 ruling): the blueprint
        // through such a target carries ALLHDL rows INSTEAD of the
        // built-ins (probed live on orbit -- the custom fileset replaces
        // the built-in collection in the plan), so every drive routes a
        // row by its file EXTENSION: .vhd/.vhdl to the VHDL faces, .v/.sv
        // to the Verilog faces -- and the library column rides the row
        // unchanged.
        let vhdl_rows =
            parse_blueprint("ALLHDL\tgrlib\t./pkg.vhd\nALLHDL\twork\t./tb.vhd\n").unwrap();
        let verilog_rows =
            parse_blueprint("ALLHDL\twork\t./top.v\nALLHDL\twork\t./tb.sv\n").unwrap();
        // The VHDL faces carry the ALLHDL .vhd/.vhdl rows (libraries
        // attached); the Verilog faces carry the ALLHDL .v/.sv rows.
        let carried = vhdl_entries(&vhdl_rows).unwrap();
        assert_eq!(carried.len(), 2);
        assert_eq!(carried[0].path, PathBuf::from("./pkg.vhd"));
        assert_eq!(carried[0].library, "grlib");
        assert_eq!(carried[1].path, PathBuf::from("./tb.vhd"));
        assert_eq!(
            verilog_only(&verilog_rows).unwrap(),
            vec![PathBuf::from("./top.v"), PathBuf::from("./tb.sv")]
        );
        // A MIXED ALLHDL blueprint is still a loud rejection on the
        // single-language faces (the per-face carry rules are unchanged:
        // the Verilog rows are foreign to the VHDL drives and vice
        // versa) -- only the both-language faces split it.
        let mixed = parse_blueprint("ALLHDL\tgrlib\t./pkg.vhd\nALLHDL\twork\t./top.v\n").unwrap();
        assert_eq!(vhdl_entries(&mixed).unwrap_err().entries.len(), 1);
        assert_eq!(verilog_only(&mixed).unwrap_err().entries.len(), 1);
        // The both-language faces split the merged fileset by extension.
        let both = parse_blueprint(
            "ALLHDL\tgrlib\t./pkg.vhd\nALLHDL\twork\t./tb.vhd\nALLHDL\twork\t./top.v\n\
             ALLHDL\twork\t./tb.sv\n",
        )
        .unwrap();
        let (vhdl, verilog) = vivado_files(&both).unwrap();
        assert_eq!(vhdl.len(), 2);
        assert_eq!(vhdl[0].library, "grlib");
        assert_eq!(
            verilog,
            vec![PathBuf::from("./top.v"), PathBuf::from("./tb.sv")]
        );
        // The built-in filesets ride exactly as before beside the
        // ALLHDL routing.
        assert!(is_vhdl_row(&both[0]));
        assert!(is_verilog_row(&both[2]));
        assert!(!is_vhdl_row(&both[2]));
        // An ALLHDL row with neither family's extension is a loud
        // not-carried face, never a silent skip.
        let weird = parse_blueprint("ALLHDL\twork\t./x.txt\n").unwrap();
        assert!(vhdl_entries(&weird).is_err());
        assert!(verilog_only(&weird).is_err());
    }

    #[test]
    fn the_entry_filter_hands_the_drive_both_paths_and_libraries() {
        // The VHDL drives' multi-library face: the carried entries carry
        // their blueprint library beside their path, in blueprint order.
        let entries = parse_blueprint("VHDL\tgrlib\t./pkg.vhd\nVHDL\twork\t./tb.vhd\n").unwrap();
        let carried = vhdl_entries(&entries).unwrap();
        assert_eq!(carried.len(), 2);
        assert_eq!(carried[0].library, "grlib");
        assert_eq!(carried[0].path, PathBuf::from("./pkg.vhd"));
        assert_eq!(carried[1].library, "work");
        assert_eq!(carried[1].path, PathBuf::from("./tb.vhd"));
        // The refusal is the shared one, byte-for-byte the paths-only
        // face's.
        let err = vhdl_entries(&parse_blueprint("VLOG\twork\t./b.v\n").unwrap()).unwrap_err();
        assert_eq!(err.entries.len(), 1);
        assert_eq!(err.entries[0].fileset, "VLOG");
    }

    #[test]
    fn the_not_carried_refusal_names_the_family_never_one_drive() {
        // Two drives share this error (ghdl and nvc), so the message
        // names the fileset family, never one drive's name: an
        // `orbit-drives nvc` user on a Verilog blueprint must not be
        // told the refusal is ghdl's (proven live: the shared Display
        // once hardcoded "the ghdl drive carries").
        let err = vhdl_only(&parse_blueprint("VLOG\twork\t./b.v\n").unwrap()).unwrap_err();
        let message = err.to_string();
        assert!(
            message.starts_with("the VHDL drives carry the VHDL fileset"),
            "the refusal should name the drive family: {message}"
        );
        assert!(
            !message.contains("ghdl") && !message.contains("nvc"),
            "the refusal should not name a single drive: {message}"
        );
        assert!(
            message.contains("VLOG (blueprint line 1, ./b.v)"),
            "the refusal should name the offending entry: {message}"
        );
    }

    #[test]
    fn the_verilog_drives_carry_vlog_and_sysv_and_name_the_rest() {
        let entries = parse_blueprint("VLOG\twork\t./b.v\nSYSV\twork\t./c.sv\n").unwrap();
        let carried = verilog_only(&entries).unwrap();
        assert_eq!(
            carried,
            vec![PathBuf::from("./b.v"), PathBuf::from("./c.sv")]
        );
        let err = verilog_only(&parse_blueprint("VHDL\twork\t./a.vhd\n").unwrap()).unwrap_err();
        assert_eq!(err.entries.len(), 1);
        assert_eq!(err.entries[0].fileset, "VHDL");
    }

    #[test]
    fn the_vivado_drives_carry_both_languages_and_name_the_rest() {
        // Vivado speaks VHDL and Verilog: the one filter that carries two
        // fileset families -- the VHDL entries with their blueprint
        // libraries attached (the §25 column, consumed on both faces),
        // the Verilog entries as bare paths (no library clauses to
        // consume).
        let entries = parse_blueprint(
            "VLOG\twork\t./b.v\nVHDL\tgrlib\t./a.vhd\nSYSV\twork\t./c.sv\nVHDL\twork\t./d.vhd\n",
        )
        .unwrap();
        let (vhdl, verilog) = vivado_files(&entries).unwrap();
        assert_eq!(vhdl.len(), 2);
        assert_eq!(vhdl[0].path, PathBuf::from("./a.vhd"));
        assert_eq!(vhdl[0].library, "grlib");
        assert_eq!(vhdl[1].path, PathBuf::from("./d.vhd"));
        assert_eq!(vhdl[1].library, "work");
        assert_eq!(
            verilog,
            vec![PathBuf::from("./b.v"), PathBuf::from("./c.sv")]
        );
        // A custom fileset is the loud not-carried face, in blueprint
        // order.
        let err =
            vivado_files(&parse_blueprint("VHDL\twork\t./a.vhd\nMYSET\twork\t./x.v\n").unwrap())
                .unwrap_err();
        assert_eq!(err.entries.len(), 1);
        assert_eq!(err.entries[0].fileset, "MYSET");
    }

    #[test]
    fn the_quartus_build_face_carries_both_languages_and_names_the_rest() {
        // quartus_map speaks both languages through the generated .qsf:
        // the same two-family filter shape as vivado's, each file split
        // to its own assignment line.
        let entries = parse_blueprint(
            "VLOG\twork\t./b.v\nVHDL\twork\t./a.vhd\nSYSV\twork\t./c.sv\nVHDL\twork\t./d.vhd\n",
        )
        .unwrap();
        let (vhdl, verilog) = quartus_files(&entries).unwrap();
        assert_eq!(
            vhdl,
            vec![PathBuf::from("./a.vhd"), PathBuf::from("./d.vhd")]
        );
        assert_eq!(
            verilog,
            vec![PathBuf::from("./b.v"), PathBuf::from("./c.sv")]
        );
        let err =
            quartus_files(&parse_blueprint("VHDL\twork\t./a.vhd\nMYSET\twork\t./x.v\n").unwrap())
                .unwrap_err();
        assert_eq!(err.entries.len(), 1);
        assert_eq!(err.entries[0].fileset, "MYSET");
        assert_eq!(err.face, "build face");
    }
}
