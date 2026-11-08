//! The real vivado spawn shape, pinned end to end: the REAL toolchain
//! (vivado + the xvhdl/xvlog/xelab/xsim stage binaries) driven the way
//! orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the working
//! directory at the target's output directory -- across the deferral
//! loop's order-sensitive compile, the part gate, a judged compile
//! failure, the LYING RUN EXIT (a severity-failure testbench that exits 0
//! and must fail on its `Failure:` face), the clean sim, the
//! single-work merge of a two-library blueprint (the mapped `xsim.ini`,
//! the bare top, the merged work library) through BOTH faces, and the
//! generated script's `read_vhdl -library` lines.
//!
//! Ignored by default -- it needs the Vivado install; run it with
//! `cargo test -- --ignored`. The test locates the install the way the
//! drive does (the `VIVADO_BIN` env hook, else the standard prefix) and
//! lets the drive set the launch recipe env itself (LOCPATH / LC_ALL /
//! LD_LIBRARY_PATH defaults derived from the resolved install), which is
//! exactly the shipped path: point `VIVADO_BIN` at the binary and the
//! recipe legs ride the drive's own defaults.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const STD_INSTALL: &str = "/opt/2025.2.1/Vivado/bin/vivado";

fn vivado_bin() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VIVADO_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let std_path = PathBuf::from(STD_INSTALL);
    std_path.exists().then_some(std_path)
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider, consumer, and testbenches of the probed fixture:
/// xvhdl is order-sensitive within work (probed live on 2025.2.1), so
/// this fixture hands the files in PROVIDER order -- gate, top, then the
/// testbenches; the deferral loop exists for the day orbit hands them
/// consumer-first.
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
    assert y = '0' report \"the design is wrong\" severity failure;
    report \"sim done\";
    wait;
  end process stim;
end architecture sim;
",
    );
    write(
        &out.join("tb_fatal.vhd"),
        "\
library ieee;
use ieee.std_logic_1164.all;

entity tb_fatal is
end entity tb_fatal;

architecture sim of tb_fatal is
  signal a : std_logic := '0';
  signal y : std_logic;
begin
  dut : entity work.top
    port map (a => a, y => y);
  stim : process is
  begin
    a <= '1';
    wait for 1 ns;
    assert y = '1' report \"the design is wrong\" severity failure;
    report \"unreachable\";
    wait;
  end process stim;
end architecture sim;
",
    );
}

/// The grlib package of the multi-library fixture, and the design and
/// testbench that consume it across the library boundary -- under the
/// single-work sim face the blueprint's two source libraries MERGE into
/// `work` (the drive maps `grlib=xsim.dir/work` in the cwd `xsim.ini`,
/// so the sources' `library grlib;` clauses resolve), and the testbench
/// rides the DIRECT `entity work.gate` instantiation: the merged work
/// library holds every unit, so the reference resolves trivially on the
/// sim face while the build face's `read_vhdl -library` reads keep the
/// rows' named libraries. (The DEFAULT component-binding face does NOT
/// survive the merge -- probed: a component reached through a mapped
/// alias still black-boxes on the 10-4940 face; that wall is the
/// README's honest limit.)
fn write_multilib(out: &Path) {
    write(
        &out.join("pkg.vhd"),
        "\
package grlib_pkg is
  constant WIDTH : integer := 8;
end package;
",
    );
    write(
        &out.join("gate.vhd"),
        "\
library grlib;
use grlib.grlib_pkg.all;
entity gate is
  port ( o : out integer );
end entity;
architecture rtl of gate is
begin
  o <= WIDTH;
end architecture;
",
    );
    write(
        &out.join("tb.vhd"),
        "\
library grlib;
use grlib.grlib_pkg.all;
entity tb is
end entity;
architecture sim of tb is
  signal o : integer;
begin
  dut : entity work.gate port map (o);
  process begin
    wait for 1 ns;
    assert o = 8 report \"width drifted\" severity failure;
    report \"SIM PASSED\";
    wait;
  end process;
end architecture;
",
    );
}

/// The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
/// working directory at the target's output directory, `VIVADO_BIN`
/// pointing at the resolved install; `$ORBIT_PROJECT_LIBRARY` is
/// stripped regardless (the single-work face reads no project library,
/// and an inherited value from the driving shell must not leak).
fn spawn_shape<'a>(
    command: &'a mut Command,
    out: &Path,
    vivado: &Path,
    tb: &str,
) -> &'a mut Command {
    command
        .env("VIVADO_BIN", vivado)
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", tb)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .env_remove("ORBIT_PROJECT_LIBRARY")
        .env_remove("LOCPATH")
        .env_remove("LD_LIBRARY_PATH")
        .current_dir(out)
}

#[test]
#[ignore = "needs the Vivado install (VIVADO_BIN or the standard prefix); run with `cargo test -- --ignored`"]
fn the_real_vivado_sim_faces_catch_the_lying_exit_and_run_clean() {
    let Some(vivado) = vivado_bin() else {
        eprintln!("skipping: no Vivado install found");
        return;
    };

    let out = std::env::temp_dir().join(format!("orbit-drives-viv-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_design(&out);

    // The clean sim first: exit 0, the judged log clean, the xsim banner
    // relayed.
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\nVHDL\twork\t./tb_clean.vhd\n",
    );
    let clean = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado"),
        &out,
        &vivado,
        "tb_clean",
    )
    .output()
    .unwrap();
    assert!(
        clean.status.success(),
        "the clean sim failed:\n{}{}",
        String::from_utf8_lossy(&clean.stdout),
        String::from_utf8_lossy(&clean.stderr)
    );
    let stdout = String::from_utf8_lossy(&clean.stdout);
    assert!(
        stdout.contains("tb_clean: pass"),
        "the clean sim should pass:\n{stdout}"
    );
    assert!(
        stdout.contains("compiling 3 VHDL file(s) in blueprint order with the deferral loop"),
        "the VHDL compile should ride the deferral loop:\n{stdout}"
    );
    assert!(
        !out.join("xsim.ini").exists(),
        "a single-`work` blueprint writes no mapped-library file"
    );

    // The lying exit, live: the severity-failure testbench stops at the
    // assert, xsim EXITS 0 (probed lie), and the drive must fail it on
    // the line-start `Failure:` face -- exit 1.
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\nVHDL\twork\t./tb_fatal.vhd\n",
    );
    let failing = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado"),
        &out,
        &vivado,
        "tb_fatal",
    )
    .output()
    .unwrap();
    assert_eq!(
        failing.status.code(),
        Some(1),
        "the lying exit-0 run must fail on its face:\n{}{}",
        String::from_utf8_lossy(&failing.stdout),
        String::from_utf8_lossy(&failing.stderr)
    );
    let stdout = String::from_utf8_lossy(&failing.stdout);
    let stderr = String::from_utf8_lossy(&failing.stderr);
    assert!(
        stdout.contains("tb_fatal: fail (the log carries a probed vivado failure face)"),
        "the drive should name the face witness:\n{stdout}"
    );
    assert!(
        stderr.contains("1 run(s) carried a finding"),
        "the drive should count the finding:\n{stderr}"
    );
    assert!(
        stdout.contains("Failure: the design is wrong"),
        "the probed face should relay verbatim:\n{stdout}"
    );
}

#[test]
#[ignore = "needs the Vivado install (VIVADO_BIN or the standard prefix); run with `cargo test -- --ignored`"]
fn the_real_vivado_drive_judges_a_failed_compile_and_names_the_stage() {
    let Some(vivado) = vivado_bin() else {
        eprintln!("skipping: no Vivado install found");
        return;
    };

    let out = std::env::temp_dir().join(format!("orbit-drives-viv-fail-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // The probed compile face (xvhdl, exit 1, stdout): the syntax errors
    // land uppercase and the file never compiles after the deferral loop
    // exhausts.
    write(
        &out.join("broken.vhd"),
        "\
entity broken is
end entity broken;
architecture rtl of broken is
begin
  y <= not a
end architecture rtl;
",
    );
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./broken.vhd\n");

    let drive = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado"),
        &out,
        &vivado,
        "broken",
    )
    .output()
    .unwrap();
    assert_eq!(
        drive.status.code(),
        Some(1),
        "a judged compile failure must exit 1:\n{}{}",
        String::from_utf8_lossy(&drive.stdout),
        String::from_utf8_lossy(&drive.stderr)
    );
    let stderr = String::from_utf8_lossy(&drive.stderr);
    assert!(
        stderr.contains("1 VHDL file(s) never compiled after"),
        "the residual should be counted:\n{stderr}"
    );
    assert!(
        stderr.contains("ERROR: [VRFC 10-4982] syntax error near 'end'"),
        "the probed face must relay verbatim:\n{stderr}"
    );
    assert!(
        stderr.contains("the xvhdl compile failed"),
        "the error should name the stage:\n{stderr}"
    );
}

#[test]
#[ignore = "needs the Vivado install (VIVADO_BIN or the standard prefix); run with `cargo test -- --ignored`"]
fn the_real_vivado_build_face_requires_the_part_before_any_drive() {
    let Some(vivado) = vivado_bin() else {
        eprintln!("skipping: no Vivado install found");
        return;
    };

    let out = std::env::temp_dir().join(format!("orbit-drives-viv-part-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // A clean file and no --part: the exit-2 gate fires BEFORE any drive
    // -- the probed alternative is a ~4s synth-then-fail inside the tool.
    write_design(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\n",
    );
    let gate = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado-build"),
        &out,
        &vivado,
        "top",
    )
    .output()
    .unwrap();
    assert_eq!(
        gate.status.code(),
        Some(2),
        "a missing part must exit 2 before any drive:\n{}{}",
        String::from_utf8_lossy(&gate.stdout),
        String::from_utf8_lossy(&gate.stderr)
    );
    let stderr = String::from_utf8_lossy(&gate.stderr);
    assert!(
        stderr.contains("no part to synthesize: pass --part <part>"),
        "the gate should name the missing flag:\n{stderr}"
    );

    // The wrong-face rejection is a parse error: the sim face consumes
    // no part.
    let wrong = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("vivado")
            .arg("--part")
            .arg("xc7z010clg400-1"),
        &out,
        &vivado,
        "top",
    )
    .output()
    .unwrap();
    assert_eq!(
        wrong.status.code(),
        Some(2),
        "--part on the sim face must be a usage error:\n{}{}",
        String::from_utf8_lossy(&wrong.stdout),
        String::from_utf8_lossy(&wrong.stderr)
    );
    assert!(
        String::from_utf8_lossy(&wrong.stderr).contains("--part is the build faces' flag"),
        "the rejection should name the flag's faces:\n{}",
        String::from_utf8_lossy(&wrong.stderr)
    );
}

#[test]
#[ignore = "needs the Vivado install (VIVADO_BIN or the standard prefix); run with `cargo test -- --ignored`"]
fn the_real_vivado_build_face_synthesizes_and_writes_the_netlists() {
    let Some(vivado) = vivado_bin() else {
        eprintln!("skipping: no Vivado install found");
        return;
    };
    // The standard install carries the probed zynq-7000 part; anything
    // else would need its own part name, so the test rides the probed
    // one.
    if vivado != Path::new(STD_INSTALL) {
        eprintln!("skipping: the netlist test pins the probed install's part");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-viv-synth-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_design(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./gate.vhd\nVHDL\twork\t./top.vhd\n",
    );

    let synth = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("vivado-build")
            .arg("--part")
            .arg("xc7z010clg400-1"),
        &out,
        &vivado,
        "top",
    )
    .output()
    .unwrap();
    assert!(
        synth.status.success(),
        "the clean synth failed:\n{}{}",
        String::from_utf8_lossy(&synth.stdout),
        String::from_utf8_lossy(&synth.stderr)
    );
    let stdout = String::from_utf8_lossy(&synth.stdout);
    assert!(
        stdout.contains("top: clean (netlists/top.dcp, .v, .edf written)"),
        "the drive should report the payload:\n{stdout}"
    );
    for netlist in ["netlists/top.dcp", "netlists/top.v", "netlists/top.edf"] {
        assert!(
            out.join(netlist).exists(),
            "the {netlist} payload should land"
        );
    }

    // The generated script's library face, live: every VHDL read names
    // the library the file was compiled into -- the single-`work`
    // blueprint reads back as explicit `-library work`, not the bare
    // `read_vhdl` the pre-library drive wrote.
    let script = fs::read_to_string(out.join("orbit-synth-top.tcl")).unwrap();
    assert!(
        script.contains("read_vhdl -library work ./gate.vhd\n"),
        "the script should read gate back out of work:\n{script}"
    );
    assert!(
        script.contains("read_vhdl -library work ./top.vhd\n"),
        "the script should read top back out of work:\n{script}"
    );
    assert!(
        !script.contains("read_verilog"),
        "a VHDL-only blueprint should write no Verilog read:\n{script}"
    );
}

#[test]
#[ignore = "needs the Vivado install (VIVADO_BIN or the standard prefix); run with `cargo test -- --ignored`"]
fn the_real_vivado_drive_consumes_two_libraries_end_to_end() {
    let Some(vivado) = vivado_bin() else {
        eprintln!("skipping: no Vivado install found");
        return;
    };

    let out =
        std::env::temp_dir().join(format!("orbit-drives-viv-multilib-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // Two source libraries: `grlib` carries the package, `work` the
    // design and the testbench (which instantiates DIRECTLY --
    // `entity work.gate`, the merged work library holding every unit).
    // The consumer rows come FIRST, so the drive only completes through
    // the deferral loop -- pass 1 fails tb and gate on their not-yet-
    // compiled providers, pkg retires; pass 2 retires gate while tb
    // waits one more round on the not-yet-compiled gate (the tb row
    // comes first); pass 3 retires tb.
    write_multilib(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./tb.vhd\nVHDL\twork\t./gate.vhd\nVHDL\tgrlib\t./pkg.vhd\n",
    );

    // The single-work face: every file compiles into `work`, and the
    // drive maps `grlib=xsim.dir/work` in the cwd `xsim.ini` so the
    // sources' `library grlib; use grlib.grlib_pkg.all;` clauses resolve
    // against the merged library (the GRLIB vmap convention, probed).
    let orbit_shape = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado"),
        &out,
        &vivado,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        orbit_shape.status.success(),
        "the multi-library run failed:\n{}{}",
        String::from_utf8_lossy(&orbit_shape.stdout),
        String::from_utf8_lossy(&orbit_shape.stderr)
    );
    let stdout = String::from_utf8_lossy(&orbit_shape.stdout);
    assert!(
        stdout.contains("3 VHDL file(s) compiled in 3 pass(es)"),
        "the deferral loop should defer: tb waits on its providers across two \
         passes:\n{stdout}"
    );
    assert!(
        stdout.contains("tb: pass"),
        "the merged-library run should judge clean:\n{stdout}"
    );
    assert!(
        stdout.contains("SIM PASSED"),
        "the relayed log should carry the testbench's own report -- the direct \
         instantiation resolved inside the merged work library:\n{stdout}"
    );
    assert!(
        !stdout.contains("remains a black box"),
        "the probed black-box face must NOT appear: the direct instantiation resolves \
         inside work:\n{stdout}"
    );
    let ini = fs::read_to_string(out.join("xsim.ini")).unwrap();
    assert_eq!(
        ini, "grlib=xsim.dir/work\n",
        "the mapped-library file should name \
         the blueprint's non-work library exactly once:\n{ini}"
    );

    // The second run in the SAME directory: the wholesale `xsim.dir/`
    // wipe and the fresh `xsim.ini` keep the clock clean -- the run-
    // scoped state covers the merged face.
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("vivado"),
        &out,
        &vivado,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        second.status.success(),
        "the second merged run failed:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    // The build face on the same blueprint: the script reads each file
    // out of ITS ROW's library (the library column's build face, the
    // ruling keeps) and the synth completes -- the netlists land. The
    // top here is the fixture's own `tb` (the fixture has no entity
    // named `top`).
    let synth = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("vivado-build")
            .arg("--part")
            .arg("xc7z010clg400-1"),
        &out,
        &vivado,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        synth.status.success(),
        "the multi-library synth failed:\n{}{}",
        String::from_utf8_lossy(&synth.stdout),
        String::from_utf8_lossy(&synth.stderr)
    );
    let script = fs::read_to_string(out.join("orbit-synth-tb.tcl")).unwrap();
    assert!(
        script.contains("read_vhdl -library grlib ./pkg.vhd\n"),
        "the script should read pkg back out of grlib:\n{script}"
    );
    assert!(
        script.contains("read_vhdl -library work ./gate.vhd\n"),
        "the script should read gate back out of work:\n{script}"
    );
    assert!(
        script.contains("read_vhdl -library work ./tb.vhd\n"),
        "the script should read tb back out of work:\n{script}"
    );
    for netlist in ["netlists/tb.dcp", "netlists/tb.v", "netlists/tb.edf"] {
        assert!(
            out.join(netlist).exists(),
            "the {netlist} payload should land on the multi-library synth"
        );
    }

    fs::remove_dir_all(&out).ok();
}
