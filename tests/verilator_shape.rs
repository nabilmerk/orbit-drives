//! The real verilator spawn shape, pinned end to end: the REAL binary
//! driven the way orbit drives it -- no `--top`, `$ORBIT_TB_NAME`
//! exported, the working directory at the target's output directory --
//! against REAL verilator, across the order-in-spirit blueprint, a
//! judged `$fatal` (the model dies by SIGABRT and the drive exits 1),
//! a second run in the same persistent output directory, the build face
//! that must stop before any run, and the lint failure that must relay
//! verilator's own stderr.
//!
//! Ignored by default -- it needs verilator (and a C++ toolchain for
//! `--build`) on PATH; run it with `cargo test -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn verilator_present() -> bool {
    Command::new("verilator").arg("--version").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider and consumer of the order-in-spirit fixture: verilator
/// reads all files into one elaboration and is order-free (probed live
/// on Verilator 5.032, 2026-11-07), so this pair lints and builds clean
/// in one pass -- the pin is that NO deferral loop is needed here.
fn write_pair(out: &Path) {
    write(
        &out.join("top.sv"),
        "\
`timescale 1ns/1ps
module top(input a, output y);
  wire g;
  gate u_gate(a, g);
  assign y = g;
endmodule
",
    );
    write(
        &out.join("gate.sv"),
        "\
`timescale 1ns/1ps
module gate(input a, output y);
  assign y = ~a;
endmodule
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
#[ignore = "needs verilator (and a C++ toolchain for --build) on PATH; run with `cargo test -- --ignored`"]
fn the_real_verilator_spawn_shape_runs_twice_and_builds_without_a_sim() {
    if !verilator_present() {
        eprintln!("skipping: verilator is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-ver-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // gate <- top <- tb, handed over consumer-first: the order a
    // stateful backend cannot forgive, and verilator's one elaboration
    // does not care about (probed) -- one lint, one build, no deferral
    // loop.
    write_pair(&out);
    write(
        &out.join("tb.sv"),
        "\
`timescale 1ns/1ps
module tb;
  reg a = 0;
  wire y;
  top dut(a, y);
  initial begin
    #1;
    if (y !== 1) $fatal(1, \"gate output wrong\");
    $display(\"sim done\");
    $finish;
  end
endmodule
",
    );
    write(
        &out.join("blueprint.tsv"),
        "SYSV\twork\t./tb.sv\nSYSV\twork\t./top.sv\nSYSV\twork\t./gate.sv\n",
    );

    // The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
    // working directory at the target's output directory.
    let first = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator"),
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
        stdout.contains("linting 3 Verilog file(s) with --timing"),
        "the drive should lint the whole fileset once:\n{stdout}"
    );
    assert!(
        !stdout.contains("pass(es)"),
        "there must be no deferral loop on the order-free lint:\n{stdout}"
    );
    assert!(
        stdout.contains("lint clean"),
        "the lint should pass before any build:\n{stdout}"
    );
    assert!(
        stdout.contains("building tb into obj_dir"),
        "the drive should build the model into obj_dir:\n{stdout}"
    );
    assert!(
        stdout.contains("tb: pass"),
        "the passing run should judge clean:\n{stdout}"
    );
    assert!(
        stdout.contains("sim done") && stdout.contains("Verilog $finish"),
        "the relayed log should carry the clean run verbatim:\n{stdout}"
    );

    // The second run in the SAME directory: the wiped obj_dir keeps the
    // clock clean (and probed honestly: verilator's --build would
    // tolerate the stale directory anyway).
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator"),
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

    // The build face on the same blueprint: lint + the model build,
    // exit honest, and NO run -- there is no `running` line to find.
    let build = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator-build"),
        &out,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        build.status.success(),
        "verilator-build failed:\n{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_out = String::from_utf8_lossy(&build.stdout);
    assert!(
        build_out.contains("tb: built"),
        "the build face should end at the model build:\n{build_out}"
    );
    assert!(
        !build_out.contains(": running"),
        "the build face must not run anything:\n{build_out}"
    );

    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs verilator (and a C++ toolchain for --build) on PATH; run with `cargo test -- --ignored`"]
fn the_real_verilator_drive_judges_a_fatal_model() {
    if !verilator_present() {
        eprintln!("skipping: verilator is not on PATH");
        return;
    }

    // The $fatal testbench, judged FAIL through the whole drive: the
    // model prints the %Fatal face, the %Error $stop line, and
    // `Aborting...`, then dies by SIGABRT -- no exit code (the drive
    // records -1) -- so the exit convicts AND the face convicts.
    let fail = std::env::temp_dir().join(format!("orbit-drives-ver-fail-{}", std::process::id()));
    fs::create_dir_all(&fail).unwrap();
    write(
        &fail.join("tb_fatal.sv"),
        "\
`timescale 1ns/1ps
module tb_fatal;
  reg a = 0;
  wire y;
  top dut(a, y);
  initial begin
    #1;
    $fatal(1, \"the design is wrong\");
  end
endmodule
",
    );
    write_pair(&fail);
    write(
        &fail.join("blueprint.tsv"),
        "SYSV\twork\t./tb_fatal.sv\nSYSV\twork\t./top.sv\nSYSV\twork\t./gate.sv\n",
    );

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator"),
        &fail,
        "tb_fatal",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "the $fatal run must exit 1 even though the model dies by SIGABRT:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    // The %Fatal face drifts by verilator version: the probed 5.032 prints
    // `%Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is
    // wrong`, while distro builds (noble, CI) prepend a `[<time>] ` prefix
    // and scope the failure as `TOP.tb_fatal`. Pin the stable core only.
    assert!(
        stdout.contains("%Fatal: tb_fatal.sv:8: Assertion failed")
            && stdout.contains("the design is wrong"),
        "the relayed log should carry the probed face verbatim:\n{stdout}"
    );
    assert!(
        stdout.contains("%Error: tb_fatal.sv:8: Verilog $stop") && stdout.contains("Aborting..."),
        "the model should have aborted, probed:\n{stdout}"
    );
    assert!(
        stdout.contains("fail (the log carries a probed verilator failure face)"),
        "the judge should convict on the face:\n{stdout}"
    );
    fs::remove_dir_all(&fail).ok();
}

#[test]
#[ignore = "needs verilator (and a C++ toolchain for --build) on PATH; run with `cargo test -- --ignored`"]
fn the_real_verilator_drive_relays_a_lint_failure() {
    if !verilator_present() {
        eprintln!("skipping: verilator is not on PATH");
        return;
    }

    // A lint-broken file, through the whole drive: verilator's own
    // stderr relayed verbatim, exit 1, and no build or run after it.
    let broken =
        std::env::temp_dir().join(format!("orbit-drives-ver-broken-{}", std::process::id()));
    fs::create_dir_all(&broken).unwrap();
    write(
        &broken.join("broken.sv"),
        "\
module broken
  input a;
endmodule
",
    );
    write(&broken.join("blueprint.tsv"), "SYSV\twork\t./broken.sv\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator"),
        &broken,
        "broken",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "a lint failure must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("%Error: broken.sv:2:3: syntax error"),
        "verilator's own lint face should be relayed:\n{stderr}"
    );
    assert!(
        stderr.contains("%Error: Exiting due to 1 error(s)"),
        "the lint's closing face should be relayed:\n{stderr}"
    );
    assert!(
        stderr.contains("the verilator lint failed"),
        "the drive should name the failed step:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.contains(": running") && !stdout.contains("lint clean"),
        "a failed lint must build and run nothing:\n{stdout}"
    );

    // The top-name door: a --top naming no module fails the lint loudly
    // (probed: `%Error: Specified --top-module 'nosuch' was not found in
    // design.`, exit 1).
    write(
        &broken.join("ok.sv"),
        "\
`timescale 1ns/1ps
module ok;
  initial begin $display(\"quiet\"); $finish; end
endmodule
",
    );
    write(&broken.join("blueprint.tsv"), "SYSV\twork\t./ok.sv\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("verilator"),
        &broken,
        "nosuch",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "an unknown top must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("%Error: Specified --top-module 'nosuch' was not found in design."),
        "the unknown-top refusal should be relayed:\n{stderr}"
    );

    fs::remove_dir_all(&broken).ok();
}
