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
use std::path::PathBuf;

/// One entry of the blueprint: a fileset, a library, and the file's path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The 1-based blueprint line the entry came from.
    pub line: usize,
    /// The fileset orbit filed the file under (`VHDL`, `VLOG`, `SYSV`, or
    /// a custom fileset).
    pub fileset: String,
    /// The orbit library the file belongs to. Carried but not yet mapped:
    /// v1 analyzes every file into the tool's `work` library.
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
/// loudly: mixed-language designs stay outside the VHDL drives' contract,
/// and a silent skip would leave files the user believes were simulated.
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
        .filter(|entry| entry.fileset != "VLOG" && entry.fileset != "SYSV")
        .cloned()
        .collect();
    if foreign.is_empty() {
        Ok(entries.iter().map(|entry| entry.path.clone()).collect())
    } else {
        Err(crate::iverilog::NotCarried { entries: foreign })
    }
}

/// The VHDL and Verilog files of the blueprint, in blueprint order: the
/// pair the vivado and quartus build faces compile -- VHDL entries to
/// their VHDL compilers (xvhdl; the .qsf's `VHDL_FILE` lines), `VLOG`
/// and `SYSV` entries to their Verilog compilers (xvlog; the .qsf's
/// `VERILOG_FILE` lines).
///
/// Two carried tools speak both languages, so this is the filter shape
/// that carries two fileset families. Every OTHER entry -- a custom
/// fileset -- is rejected loudly: a silent skip would leave files the
/// user believes were compiled.
///
/// The error type is [`crate::vivado::NotCarried`] -- its message names
/// the three carried filesets, not one face, because both vivado faces
/// carry it. The quartus build face takes the same pair through
/// [`quartus_files`], whose error message names quartus's faces.
pub fn vivado_files(
    entries: &[Entry],
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), crate::vivado::NotCarried> {
    let foreign: Vec<Entry> = entries
        .iter()
        .filter(|entry| {
            entry.fileset != "VHDL" && entry.fileset != "VLOG" && entry.fileset != "SYSV"
        })
        .cloned()
        .collect();
    if foreign.is_empty() {
        let mut vhdl = Vec::new();
        let mut verilog = Vec::new();
        for entry in entries {
            if entry.fileset == "VHDL" {
                vhdl.push(entry.path.clone());
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
        .filter(|entry| {
            entry.fileset != "VHDL" && entry.fileset != "VLOG" && entry.fileset != "SYSV"
        })
        .cloned()
        .collect();
    if foreign.is_empty() {
        let mut vhdl = Vec::new();
        let mut verilog = Vec::new();
        for entry in entries {
            if entry.fileset == "VHDL" {
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
        // fileset families, each file split to its own compile.
        let entries = parse_blueprint(
            "VLOG\twork\t./b.v\nVHDL\twork\t./a.vhd\nSYSV\twork\t./c.sv\nVHDL\twork\t./d.vhd\n",
        )
        .unwrap();
        let (vhdl, verilog) = vivado_files(&entries).unwrap();
        assert_eq!(
            vhdl,
            vec![PathBuf::from("./a.vhd"), PathBuf::from("./d.vhd")]
        );
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
