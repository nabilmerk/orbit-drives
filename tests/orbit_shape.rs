//! The real ghdl spawn shape, pinned end to end: the REAL binary driven
//! the way orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the
//! user's extras riding the argv bare with orbit's `--` separator
//! stripped -- against REAL ghdl, twice in a row in the same persistent
//! output directory. Both defects that bounced this crate in review lived
//! exactly here, unpinned: a target that could not run through
//! `orbit test` at all, and a second run that obsoleted its own design
//! out from under `ghdl -e`. And the multi-library face lives here too:
//! a REAL two-library blueprint -- a dependency library a consumer
//! references, the consumers handed FIRST -- driven through the REAL
//! ghdl drive, the sim's pass report relayed; and a project whose own
//! library is its NAME -- orbit's default -- pinned on the elaborate and
//! run argv.
//!
//! Ignored by default -- it needs ghdl on PATH; run it with `cargo test
//! -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ghdl_present() -> bool {
    Command::new("ghdl").arg("--version").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The grlib package of the multi-library fixture, and the design and
/// testbench that consume it across the library boundary.
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
/// working directory at the target's output directory, and
/// `$ORBIT_PROJECT_LIBRARY` set only when the test asks for it -- orbit
/// always sets it on a real target, so the direct-invocation tests strip
/// it (an inherited value from the driving shell would leak in).
/// `Command::arg`/`Command::env` return `&mut Command`, so the shape
/// chains on the borrow.
fn spawn_shape<'a>(
    command: &'a mut Command,
    out: &Path,
    tb: &str,
    project_library: Option<&str>,
) -> &'a mut Command {
    let shaped = command
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", tb)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .env_remove("ORBIT_PROJECT_LIBRARY");
    let shaped = match project_library {
        Some(library) => shaped.env("ORBIT_PROJECT_LIBRARY", library),
        None => shaped,
    };
    shaped.current_dir(out)
}

#[test]
#[ignore = "needs ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_orbit_spawn_shape_runs_twice_in_one_output_directory() {
    if !ghdl_present() {
        eprintln!("skipping: ghdl is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-orbit-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // gate <- top <- tb, handed over in the order ghdl cannot forgive: the
    // consumer first, so run 1 only completes through the deferral loop.
    write(
        &out.join("top.vhd"),
        "\
entity top is
  port (a : in bit; y : out bit);
end top;
architecture rtl of top is
begin
  u : entity work.gate port map (a, y);
end rtl;
",
    );
    write(
        &out.join("gate.vhd"),
        "\
entity gate is
  port (a : in bit; y : out bit);
end gate;
architecture rtl of gate is
begin
  y <= not a;
end rtl;
",
    );
    write(
        &out.join("tb.vhd"),
        "\
entity tb is
end tb;
architecture sim of tb is
  signal a, y : bit;
begin
  dut : entity work.top port map (a, y);
  process
  begin
    a <= '0';
    wait for 1 ns;
    assert y = '1' report \"gate output wrong\" severity error;
    report \"sim done\";
    wait;
  end process;
end sim;
",
    );
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./top.vhd\nVHDL\twork\t./gate.vhd\nVHDL\twork\t./tb.vhd\n",
    );

    // The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
    // extras on the argv bare (orbit strips its own separator), the
    // working directory at the target's output directory. No
    // `$ORBIT_PROJECT_LIBRARY`: the single-library blueprint answers on
    // its own (every row carries `work`, and the ladder falls to ghdl's
    // default, which is the same library).
    let first = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("ghdl")
            .arg("--vcd=waves.vcd"),
        &out,
        "tb",
        None,
    )
    .output()
    .unwrap();
    assert!(
        first.status.success(),
        "run 1 failed:\n{}{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(
        stdout.contains("3 file(s) analyzed in 2 pass(es)"),
        "the deferral loop should retry the consumer after its provider:\n{stdout}"
    );

    // The second run in the SAME directory: the stale library files must
    // not obsolete the design out from under `ghdl -e`.
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("ghdl")
            .arg("--vcd=waves.vcd"),
        &out,
        "tb",
        None,
    )
    .output()
    .unwrap();
    assert!(
        second.status.success(),
        "run 2 failed:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_ghdl_drive_consumes_two_libraries_end_to_end() {
    if !ghdl_present() {
        eprintln!("skipping: ghdl is not on PATH");
        return;
    }

    let out =
        std::env::temp_dir().join(format!("orbit-drives-ghdl-multilib-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // grlib carries the package; work carries the design and the
    // testbench, which both reference `library grlib;`. The consumer rows
    // come FIRST, so the drive only completes through the deferral loop
    // -- across libraries now, not just within one: pass 1 finds the
    // grlib LIBRARY FILE absent (the cannot-find-resource-library face),
    // pass 1's pkg writes it, pass 2 retires the design, pass 3 the
    // testbench.
    write_multilib(&out);
    write(
        &out.join("blueprint.tsv"),
        "VHDL\twork\t./tb.vhd\nVHDL\twork\t./gate.vhd\nVHDL\tgrlib\t./pkg.vhd\n",
    );

    // The orbit shape: `$ORBIT_PROJECT_LIBRARY` names the working
    // project's library (`work` here; under a real orbit project it is
    // the project's NAME, probed by the next test).
    let orbit_shape = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("ghdl"),
        &out,
        "tb",
        Some("work"),
    )
    .output()
    .unwrap();
    assert!(
        orbit_shape.status.success(),
        "the multi-library run (with orbit's env) failed:\n{}{}",
        String::from_utf8_lossy(&orbit_shape.stdout),
        String::from_utf8_lossy(&orbit_shape.stderr)
    );
    let stdout = String::from_utf8_lossy(&orbit_shape.stdout);
    assert!(
        stdout.contains("3 file(s) analyzed in 3 pass(es)"),
        "the deferral loop should defer across libraries: tb and gate wait on the \
         grlib library file pkg writes in pass 1:\n{stdout}"
    );
    assert!(
        stdout.contains("tb: pass"),
        "the cross-library run should judge clean:\n{stdout}"
    );
    assert!(
        stdout.contains("(report note): SIM PASSED"),
        "the relayed log should carry the testbench's own report:\n{stdout}"
    );

    // The same blueprint, direct invocation (no orbit env): the resolver
    // falls to ghdl's default `work` -- the designated top's own library
    // here -- and the drive still completes, starting from the wiped
    // library files of the run before.
    let direct = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("ghdl"),
        &out,
        "tb",
        None,
    )
    .output()
    .unwrap();
    assert!(
        direct.status.success(),
        "the multi-library run (direct, no env) failed:\n{}{}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );

    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_ghdl_drive_pins_the_projects_own_library() {
    if !ghdl_present() {
        eprintln!("skipping: ghdl is not on PATH");
        return;
    }

    // A project whose own library is its NAME -- orbit's actual default
    // (`get_hdl_library` falls to the project name, not to any tool's
    // `work`): every blueprint row carries `counter`, testbench included.
    // This is the shape that makes the `--work` pin load-bearing on the
    // elaborate and run argv: ghdl's bare-top commands search the WORK
    // library only (`error: cannot find entity or configuration tb`,
    // probed), so the old argv shape would refuse this project.
    let out = std::env::temp_dir().join(format!(
        "orbit-drives-ghdl-projectlib-{}",
        std::process::id()
    ));
    fs::create_dir_all(&out).unwrap();
    write(
        &out.join("pkg.vhd"),
        "\
package pkg is
  constant WIDTH : integer := 8;
end package;
",
    );
    write(
        &out.join("tb.vhd"),
        "\
library counter;
use counter.pkg.all;
entity tb is
end entity;
architecture sim of tb is
begin
  process begin
    assert WIDTH = 8 report \"width drifted\" severity failure;
    report \"SIM PASSED\";
    wait;
  end process;
end architecture;
",
    );
    write(
        &out.join("blueprint.tsv"),
        "VHDL\tcounter\t./tb.vhd\nVHDL\tcounter\t./pkg.vhd\n",
    );

    // With orbit's env -- the planner's voice, the shape every real orbit
    // target carries.
    let orbit_shape = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("ghdl"),
        &out,
        "tb",
        Some("counter"),
    )
    .output()
    .unwrap();
    assert!(
        orbit_shape.status.success(),
        "the project-named-library run (with orbit's env) failed:\n{}{}",
        String::from_utf8_lossy(&orbit_shape.stdout),
        String::from_utf8_lossy(&orbit_shape.stderr)
    );
    let stdout = String::from_utf8_lossy(&orbit_shape.stdout);
    assert!(
        stdout.contains("tb: pass") && stdout.contains("SIM PASSED"),
        "the pinned-library run should judge clean and relay the report:\n{stdout}"
    );

    // Direct invocation, no env: the resolver falls to the blueprint's
    // single carried library -- `counter` again -- and the drive still
    // completes.
    let direct = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("ghdl"),
        &out,
        "tb",
        None,
    )
    .output()
    .unwrap();
    assert!(
        direct.status.success(),
        "the project-named-library run (direct, no env) failed:\n{}{}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );

    fs::remove_dir_all(&out).ok();
}
