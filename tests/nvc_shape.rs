//! The real nvc spawn shape, pinned end to end: the REAL binary driven the
//! way orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the
//! working directory at the target's output directory -- against REAL nvc,
//! across the order-dependent blueprint, a judged severity failure, a
//! judged severity error, a second run in the same persistent output
//! directory, and the build face that must stop before any run.
//!
//! Ignored by default -- it needs nvc on PATH; run it with `cargo test
//! -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn nvc_present() -> bool {
    Command::new("nvc").arg("--version").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider and consumer of the order-dependence fixture: the
/// consumer references the provider's unit, so nvc cannot analyze the
/// consumer until the provider is done.
fn write_pair(out: &Path) {
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
}

/// The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
/// working directory at the target's output directory. `Command::arg`
/// returns `&mut Command`, so the shape chains on the borrow.
fn spawn_shape<'a>(command: &'a mut Command, out: &Path, tb: &str) -> &'a mut Command {
    command
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", tb)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .current_dir(out)
}

#[test]
#[ignore = "needs nvc on PATH; run with `cargo test -- --ignored`"]
fn the_real_nvc_spawn_shape_defers_runs_twice_and_builds_without_a_sim() {
    if !nvc_present() {
        eprintln!("skipping: nvc is not on PATH");
        return;
    }

    let out =
        std::env::temp_dir().join(format!("orbit-contracts-nvc-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // gate <- top <- tb, handed over in the order nvc cannot forgive: the
    // consumer first, so run 1 only completes through the deferral loop.
    write_pair(&out);
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
    // working directory at the target's output directory.
    let first = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("nvc"),
        &out,
        "tb",
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
    assert!(
        stdout.contains("tb: pass"),
        "the passing run should judge clean:\n{stdout}"
    );
    assert!(
        stdout.contains("** Note:") && stdout.contains("sim done"),
        "the relayed log should carry the testbench's own note face:\n{stdout}"
    );

    // The second run in the SAME directory: the wiped work library keeps
    // the clock clean (and probed honestly: nvc, unlike ghdl, would
    // tolerate the stale library anyway).
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("nvc"),
        &out,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        second.status.success(),
        "run 2 failed:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    // The build face on the same blueprint: analyze + elaborate, exit
    // honest, and NO run -- there is no `running` line to find.
    let build = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("nvc-build"),
        &out,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        build.status.success(),
        "nvc-build failed:\n{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_out = String::from_utf8_lossy(&build.stdout);
    assert!(
        build_out.contains("tb: elaborated"),
        "the build face should end at the elaboration:\n{build_out}"
    );
    assert!(
        !build_out.contains(": running"),
        "the build face must not run anything:\n{build_out}"
    );

    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs nvc on PATH; run with `cargo test -- --ignored`"]
fn the_real_nvc_drive_judges_a_severity_failure_and_a_severity_error() {
    if !nvc_present() {
        eprintln!("skipping: nvc is not on PATH");
        return;
    }

    // The severity-failure testbench, judged FAIL through the whole drive:
    // the face in the log AND nvc's honest exit-1, either witness enough.
    let fail =
        std::env::temp_dir().join(format!("orbit-contracts-nvc-fail-{}", std::process::id()));
    fs::create_dir_all(&fail).unwrap();
    write(
        &fail.join("tb_fail.vhd"),
        "\
entity tb_fail is
end tb_fail;
architecture sim of tb_fail is
begin
  process
  begin
    assert false report \"the design is wrong\" severity failure;
    wait;
  end process;
end sim;
",
    );
    write(&fail.join("blueprint.tsv"), "VHDL\twork\t./tb_fail.vhd\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("nvc"),
        &fail,
        "tb_fail",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "the severity-failure run must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("** Failure: 0ms+0: the design is wrong"),
        "the relayed log should carry the probed face verbatim:\n{stdout}"
    );
    assert!(
        stdout.contains("fail (the log carries a probed nvc failure face)"),
        "the judge should convict on the face:\n{stdout}"
    );
    fs::remove_dir_all(&fail).ok();

    // The severity-error testbench, judged FAIL too: nvc continues the
    // simulation but STILL exits 1 (probed live on nvc 1.23.0,
    // 2026-11-07 -- unlike bare ghdl's exit-0 hole, nvc's exit convicts
    // on its own, and the face is the second witness).
    let err = std::env::temp_dir().join(format!("orbit-contracts-nvc-err-{}", std::process::id()));
    fs::create_dir_all(&err).unwrap();
    write(
        &err.join("tb_err.vhd"),
        "\
entity tb_err is
end tb_err;
architecture sim of tb_err is
begin
  process
  begin
    assert false report \"check failed\" severity error;
    report \"simulation continued\";
    wait;
  end process;
end sim;
",
    );
    write(&err.join("blueprint.tsv"), "VHDL\twork\t./tb_err.vhd\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("nvc"),
        &err,
        "tb_err",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "the severity-error run must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("** Error: 0ms+0: check failed"),
        "the relayed log should carry the probed face verbatim:\n{stdout}"
    );
    assert!(
        stdout.contains("** Note: 0ms+0: simulation continued"),
        "nvc should have continued past the severity error:\n{stdout}"
    );
    assert!(
        stdout.contains("fail (the log carries a probed nvc failure face)"),
        "the judge should convict on the face:\n{stdout}"
    );
    fs::remove_dir_all(&err).ok();
}
