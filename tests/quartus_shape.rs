//! The real quartus spawn shape, pinned end to end: the REAL toolchain
//! (quartus_map; the bundled Questa's vlib/vmap/vcom/vsim) driven the way
//! orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the working
//! directory at the target's output directory -- across the generated
//! `.qsf`, the batch `quartus_map`, the vcom deferral loop, and the
//! judged vsim run.
//!
//! Ignored by default AND skipped unless the toolchain is present: the
//! drive resolves the binaries from `QUARTUS_BIN` (the quartus_map path,
//! the Questa binaries deriving as the questa_fse/bin sibling), bare
//! PATH names otherwise. quartus is NOT installed on the probing box,
//! so these tests have never run here -- they are the first-contact
//! harness: point `QUARTUS_BIN` at an install, run
//! `cargo test -- --ignored`, and every hypothesis this crate records
//! gets its first live answer.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn quartus_bin() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("QUARTUS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    // The documented derive needs an install to derive from; without the
    // env hook there is nothing to resolve and the tests skip.
    None
}

/// The quartus binaries on this box, the way the drive resolves them:
/// the Questa siblings derive from the quartus install. `None` skips the
/// test -- the harness only runs where the toolchain lives.
fn questa_or_skip(quartus: &Path, name: &str) -> Option<PathBuf> {
    let derived = quartus
        .parent()
        .and_then(|bin| bin.parent())
        .map(|install| install.join("questa_fse").join("bin").join(name));
    let path = derived.filter(|p| p.exists());
    path.is_some().then_some(path.unwrap())
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider, consumer, and testbench of the probed fixture shape:
/// the same gate/top/testbench triple every VHDL drive carries.
fn write_design(out: &Path) {
    write(
        &out.join("gate.vhd"),
        "\
library ieee;
use ieee.std_logic_1164.all;

entity gate is
  port (a : in std_logic; y : out std_logic);
end entity gate;

architecture rtl of gate is
begin
  y <= not a;
end architecture rtl;
",
    );
    write(
        &out.join("top.vhd"),
        "\
library ieee;
use ieee.std_logic_1164.all;

entity top is
  port (a : in std_logic; y : out std_logic);
end entity top;

architecture rtl of top is
begin
  u_gate : entity work.gate
    port map (a => a, y => y);
end architecture rtl;
",
    );
    write(
        &out.join("tb_clean.vhd"),
        "\
library ieee;
use ieee.std_logic_1164.all;

entity tb_clean is
end entity tb_clean;

architecture sim of tb_clean is
  signal a : std_logic := '0';
  signal y : std_logic;
begin
  dut : entity work.top
    port map (a => a, y => y);
  stim : process is
  begin
    a <= '1';
    wait for 1 ns;
    assert y = '0' report \"sim done\" severity note;
    wait;
  end process stim;
end architecture sim;
",
    );
}

/// The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
/// working directory at the target's output directory, `QUARTUS_BIN`
/// pointing at the resolved install.
fn spawn_shape<'a>(
    command: &'a mut Command,
    out: &Path,
    quartus: &Path,
    tb: &str,
) -> &'a mut Command {
    command
        .env("QUARTUS_BIN", quartus)
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", tb)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .current_dir(out)
}

#[test]
#[ignore = "needs a Quartus install (QUARTUS_BIN); run with `cargo test -- --ignored`"]
fn the_real_quartus_build_face_writes_the_qsf_and_maps_clean() {
    let Some(quartus) = quartus_bin() else {
        eprintln!("skipping: no Quartus install found (QUARTUS_BIN)");
        return;
    };

    let out = std::env::temp_dir().join(format!("orbit-drives-qrt-build-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_design(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\n",
    );

    let build = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("quartus-build")
            .arg("--part")
            .arg("xc7z010clg400-1"),
        &out,
        &quartus,
        "top",
    )
    .output()
    .unwrap();
    assert!(
        build.status.success(),
        "the clean map failed (the exit honesty is the documented hypothesis; this is the \
         first contact):\n{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let stdout = String::from_utf8_lossy(&build.stdout);
    assert!(
        stdout.contains("top: clean"),
        "the drive should report the clean map:\n{stdout}"
    );
    // The generated .qsf carries the DEVICE, the top, and both files.
    let qsf = fs::read_to_string(out.join("top.qsf")).unwrap();
    assert!(qsf.contains("set_global_assignment -name DEVICE xc7z010clg400-1"));
    assert!(qsf.contains("set_global_assignment -name TOP_LEVEL_ENTITY top"));
    assert!(qsf.contains("set_global_assignment -name VHDL_FILE ./gate.vhd"));
    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs a Quartus install (QUARTUS_BIN); run with `cargo test -- --ignored`"]
fn the_real_quartus_sim_face_compiles_through_vcom_and_runs_vsim() {
    let Some(quartus) = quartus_bin() else {
        eprintln!("skipping: no Quartus install found (QUARTUS_BIN)");
        return;
    };
    let Some(vsim) = questa_or_skip(&quartus, "vsim") else {
        eprintln!("skipping: no bundled Questa derived beside the quartus install");
        return;
    };
    let _ = vsim;

    let out = std::env::temp_dir().join(format!("orbit-drives-qrt-sim-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_design(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\nVHDL\twork\t./tb_clean.vhd\n",
    );

    let sim = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("quartus"),
        &out,
        &quartus,
        "tb_clean",
    )
    .output()
    .unwrap();
    assert!(
        sim.status.success(),
        "the clean sim failed (vsim's exit honesty is the documented hypothesis; this is \
         the first contact):\n{}{}",
        String::from_utf8_lossy(&sim.stdout),
        String::from_utf8_lossy(&sim.stderr)
    );
    let stdout = String::from_utf8_lossy(&sim.stdout);
    assert!(
        stdout.contains("tb_clean: pass"),
        "the clean sim should pass:\n{stdout}"
    );
    assert!(
        stdout.contains("compiling 3 VHDL file(s) in blueprint order with the deferral loop"),
        "the vcom compile should ride the deferral loop:\n{stdout}"
    );
    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs a Quartus install (QUARTUS_BIN); run with `cargo test -- --ignored`"]
fn the_real_quartus_sim_face_rejects_the_verilog_fileset_loudly() {
    let Some(quartus) = quartus_bin() else {
        eprintln!("skipping: no Quartus install found (QUARTUS_BIN)");
        return;
    };

    let out = std::env::temp_dir().join(format!("orbit-drives-qrt-nc-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write(&out.join("gate.v"), "module gate; endmodule\n");
    write(&out.join("blueprint.tsv"), "VLOG\twork\t./gate.v\n");

    let sim = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("quartus"),
        &out,
        &quartus,
        "gate",
    )
    .output()
    .unwrap();
    assert_eq!(
        sim.status.code(),
        Some(2),
        "a VLOG-only blueprint must be the loud not-carried face on the sim face:\n{}{}",
        String::from_utf8_lossy(&sim.stdout),
        String::from_utf8_lossy(&sim.stderr)
    );
    let stderr = String::from_utf8_lossy(&sim.stderr);
    assert!(
        stderr.contains("the quartus sim face carries the VHDL fileset"),
        "the not-carried face should name the sim face's fileset:\n{stderr}"
    );
    fs::remove_dir_all(&out).ok();
}
