//! The std-token mapping tables.
//!
//! Orbit (the fork, chaseruskin/orbit#24) carries each project's
//! declared language standard to the target process: per-row in the
//! blueprint's trailing std field, and project-wide as `ORBIT_STD`.
//! The drives speak their tools' own spellings, so this module maps
//! orbit's documented vocabulary tokens onto each tool's selector flag
//! — and answers `None` (the caller's loud face) for a combination the
//! tool does not carry.
//!
//! Every mapping row below is a probed fact or a documented-only fact,
//! never invented:
//! - ghdl 6.0.0 (probed live 2026-11-22): `--std=93`, `--std=08`,
//!   `--std=19` answer; `--std=87` is REJECTED (ghdl 6.0 dropped the
//!   VHDL-87 selector), so vhdl-87 through ghdl is `None`.
//! - nvc 1.23.1 (probed live 2026-11-22): `--std=1993`, `--std=2008`,
//!   `--std=2019` answer; VHDL-87 is not carried — `None`.
//! - iverilog 12.0 (probed live 2026-11-22): `-g1995`, `-g2001`,
//!   `-g2005`, `-g2012`, `-g2005-sv` answer.
//! - verilator 5.032 (probed live 2026-11-22): `--language 1364-1995`,
//!   `1364-2001`, `1364-2005`, `1800-2005`, `1800-2012`, `1800-2017`
//!   answer.
//! - quartus's vcom (documented-only — the tool is absent on this box):
//!   vcom spells `-87` / `-93` / `-2008`; VHDL-2019 is not documented —
//!   `None`.
//! - cocotb rides its backing simulator's runner; the ghdl runner takes
//!   the std through ghdl's own spellings (cocotb 2.1.0's
//!   `runner.build(build_args=[...])`, signature-probed 2026-11-22).

/// The flags come back as argv pieces, not one string: verilator's
/// selector is a two-token pair (`--language`, `<value>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Ghdl,
    Nvc,
    Quartus,
    Cocotb,
    Iverilog,
    Verilator,
}

/// Maps a VHDL std token onto the tool's selector argv pieces.
/// `None` = the tool does not carry that revision: the caller's loud
/// exit-2 face names the tool and the token.
pub fn vhdl_flag(tool: Tool, token: &str) -> Option<Vec<String>> {
    let pieces: &[&str] = match (tool, token) {
        // "93" is ghdl's default revision: no flag rides (see the test
        // above for the probed wall the explicit pin creates)
        (Tool::Ghdl, "93") => &[],
        (Tool::Ghdl, "2008") => &["--std=08"],
        (Tool::Ghdl, "2019") => &["--std=19"],
        (Tool::Nvc, "93") => &["--std=1993"],
        (Tool::Nvc, "2008") => &["--std=2008"],
        (Tool::Nvc, "2019") => &["--std=2019"],
        (Tool::Quartus, "87") => &["-87"],
        (Tool::Quartus, "93") => &["-93"],
        (Tool::Quartus, "2008") => &["-2008"],
        // cocotb compiles through its backing simulator's runner; the
        // shipped drive rides the ghdl runner, so ghdl's spellings apply
        (Tool::Cocotb, _) => return vhdl_flag(Tool::Ghdl, token),
        _ => return None,
    };
    Some(pieces.iter().map(|piece| String::from(*piece)).collect())
}

/// Maps a Verilog/SystemVerilog std token onto the tool's selector argv
/// pieces. `is_sv` separates the two vocabularies' spellings where a
/// tool distinguishes them. `None` = the tool does not carry it.
pub fn verilog_flag(tool: Tool, token: &str, is_sv: bool) -> Option<Vec<String>> {
    let pieces: &[&str] = match (tool, token, is_sv) {
        (Tool::Iverilog, "95", false) => &["-g1995"],
        (Tool::Iverilog, "2001", false) => &["-g2001"],
        (Tool::Iverilog, "2005", false) => &["-g2005"],
        (Tool::Iverilog, "2012", _) => &["-g2012"],
        (Tool::Iverilog, "2005", true) => &["-g2005-sv"],
        (Tool::Verilator, "95", false) => &["--language", "1364-1995"],
        (Tool::Verilator, "2001", false) => &["--language", "1364-2001"],
        (Tool::Verilator, "2005", false) => &["--language", "1364-2005"],
        (Tool::Verilator, "2005", true) => &["--language", "1800-2005"],
        (Tool::Verilator, "2012", _) => &["--language", "1800-2012"],
        (Tool::Verilator, "2017", _) => &["--language", "1800-2017"],
        _ => return None,
    };
    Some(pieces.iter().map(|piece| String::from(*piece)).collect())
}

/// Resolves a row's std token: the blueprint row's own declaration
/// wins; a row without one inherits the run-wide `ORBIT_STD` face.
/// `None` = nothing declares a std anywhere — the drive rides its
/// tool's default or its own interim pin.
///
/// The pure core is [`resolve_std`]; this wrapper only reads the
/// process env, so the tests never mutate shared state (a parallel
/// test reading `ORBIT_STD` would race a mutation — the default-path
/// tests of the single-set drives do exactly that).
pub fn row_std(entry_std: Option<&str>) -> Option<String> {
    resolve_std(
        entry_std,
        std::env::var("ORBIT_STD").ok().as_deref(),
    )
}

/// The pure resolver behind [`row_std`], injectable so the tests pin
/// both tiers without touching the process env.
pub fn resolve_std(entry_std: Option<&str>, env_std: Option<&str>) -> Option<String> {
    entry_std
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(String::from)
        .or_else(|| {
            env_std
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(String::from)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vhdl_mappings_carry_the_probed_spellings() {
        // ghdl's "93" rides the tool's DEFAULT: the explicit `--std=93`
        // pin enforces strict-93 local-static rules that the default
        // analysis does not, and the pin walls GRLIB-class corpora
        // (probed live 2026-11-22 on ghdl 6.0.0: GRLIB's amba_tp
        // `2**(gAWidth-2)` bound errors under the explicit pin and
        // compiles clean at the default -- the 2026-11-08 un-pinning
        // ruling, now proven for the declared-93 face too). A user who
        // genuinely wants the strict pin passes it through
        // `ORBIT_DRIVES_GHDL_FLAGS`.
        assert_eq!(vhdl_flag(Tool::Ghdl, "93"), Some(Vec::new()));
        assert_eq!(
            vhdl_flag(Tool::Ghdl, "2008"),
            Some(vec![String::from("--std=08")])
        );
        assert_eq!(
            vhdl_flag(Tool::Ghdl, "2019"),
            Some(vec![String::from("--std=19")])
        );
        // ghdl 6.0 dropped the VHDL-87 selector (probed)
        assert_eq!(vhdl_flag(Tool::Ghdl, "87"), None);

        assert_eq!(
            vhdl_flag(Tool::Nvc, "93"),
            Some(vec![String::from("--std=1993")])
        );
        assert_eq!(
            vhdl_flag(Tool::Nvc, "2008"),
            Some(vec![String::from("--std=2008")])
        );
        assert_eq!(
            vhdl_flag(Tool::Nvc, "2019"),
            Some(vec![String::from("--std=2019")])
        );
        assert_eq!(vhdl_flag(Tool::Nvc, "87"), None);

        // documented-only: the quartus family is absent on this box
        assert_eq!(
            vhdl_flag(Tool::Quartus, "2008"),
            Some(vec![String::from("-2008")])
        );
        assert_eq!(
            vhdl_flag(Tool::Quartus, "87"),
            Some(vec![String::from("-87")])
        );
        assert_eq!(vhdl_flag(Tool::Quartus, "2019"), None);

        // cocotb rides ghdl's spellings through the runner
        assert_eq!(vhdl_flag(Tool::Cocotb, "2008"), vhdl_flag(Tool::Ghdl, "2008"));
    }

    #[test]
    fn verilog_mappings_carry_the_probed_spellings() {
        assert_eq!(
            verilog_flag(Tool::Iverilog, "2012", false),
            Some(vec![String::from("-g2012")])
        );
        assert_eq!(
            verilog_flag(Tool::Iverilog, "2005", true),
            Some(vec![String::from("-g2005-sv")])
        );
        assert_eq!(verilog_flag(Tool::Iverilog, "2017", true), None);

        assert_eq!(
            verilog_flag(Tool::Verilator, "2005", false),
            Some(vec![
                String::from("--language"),
                String::from("1364-2005")
            ])
        );
        assert_eq!(
            verilog_flag(Tool::Verilator, "2017", true),
            Some(vec![
                String::from("--language"),
                String::from("1800-2017")
            ])
        );
        assert_eq!(verilog_flag(Tool::Verilator, "95", true), None);
    }

    #[test]
    fn the_row_std_prefers_the_row_and_falls_to_the_env() {
        // the row wins over the env face
        assert_eq!(
            resolve_std(Some("2008"), Some("93")),
            Some(String::from("2008"))
        );
        // blank row fields count as absent and fall to the env face
        assert_eq!(resolve_std(Some("  "), Some("93")), Some(String::from("93")));
        assert_eq!(resolve_std(Some(""), Some("93")), Some(String::from("93")));
        assert_eq!(resolve_std(None, Some("93")), Some(String::from("93")));
        // a blank env face counts as absent too
        assert_eq!(resolve_std(None, Some("  ")), None);
        assert_eq!(resolve_std(None, None), None);
    }
}
