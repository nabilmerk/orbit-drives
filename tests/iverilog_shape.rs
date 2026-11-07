//! The real iverilog spawn shape, pinned end to end: the REAL binary
//! driven the way orbit drives it -- no `--top`, `$ORBIT_TB_NAME`
//! exported, the working directory at the target's output directory --
//! against REAL iverilog and REAL vvp, across the order-in-spirit
//! blueprint, a judged `$fatal`, the probed `$error` hole (vvp exits 0,
//! the face convicts), a second run in the same persistent output
//! directory, the build face that must stop before any run, and the
//! compile failure that must relay iverilog's own stderr.
//!
//! Ignored by default -- it needs iverilog and vvp on PATH; run it with
//! `cargo test -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn iverilog_present() -> bool {
    Command::new("iverilog").arg("--version").output().is_ok()
        && Command::new("vvp").arg("-V").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider and consumer of the order-in-spirit fixture: iverilog's
/// compile is order-free (probed live on Icarus Verilog 12.0,
/// 2026-11-07), so this pair compiles clean in one pass -- the pin is
/// that NO deferral loop is needed here.
fn write_pair(out: &Path) {
    write(
        &out.join("top.v"),
        "\
module top(input a, output y);
  wire g;
  gate u_gate(a, g);
  assign y = g;
endmodule
",
    );
    write(
        &out.join("gate.v"),
        "\
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
#[ignore = "needs iverilog and vvp on PATH; run with `cargo test -- --ignored`"]
fn the_real_iverilog_spawn_shape_runs_twice_and_builds_without_a_sim() {
    if !iverilog_present() {
        eprintln!("skipping: iverilog is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-contracts-iv-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // gate <- top <- tb, handed over consumer-first: the order a
    // stateful backend cannot forgive, and iverilog's one compile does
    // not care about (probed) -- one pass, no deferral loop.
    write_pair(&out);
    write(
        &out.join("tb.v"),
        "\
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
        "VLOG\twork\t./tb.v\nVLOG\twork\t./top.v\nVLOG\twork\t./gate.v\n",
    );

    // The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
    // working directory at the target's output directory.
    let first = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
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
        stdout.contains("compiling 3 Verilog file(s) into one snapshot"),
        "the drive should compile the whole fileset once:\n{stdout}"
    );
    assert!(
        !stdout.contains("pass(es)"),
        "there must be no deferral loop on the order-free compile:\n{stdout}"
    );
    assert!(
        stdout.contains("orbit-snapshot.vvp compiled"),
        "the compile should name its snapshot:\n{stdout}"
    );
    assert!(
        stdout.contains("tb: pass"),
        "the passing run should judge clean:\n{stdout}"
    );
    assert!(
        stdout.contains("sim done") && stdout.contains("$finish called"),
        "the relayed log should carry the clean run verbatim:\n{stdout}"
    );

    // The second run in the SAME directory: the stale snapshot must not
    // leak into the verdict (every drive compiles a fresh one).
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
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

    // The build face on the same blueprint: the one compile, exit
    // honest, and NO run -- there is no `running` line to find.
    let build = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog-build"),
        &out,
        "tb",
    )
    .output()
    .unwrap();
    assert!(
        build.status.success(),
        "iverilog-build failed:\n{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let build_out = String::from_utf8_lossy(&build.stdout);
    assert!(
        build_out.contains("orbit-snapshot.vvp compiled"),
        "the build face should end at the compile:\n{build_out}"
    );
    assert!(
        !build_out.contains(": running"),
        "the build face must not run anything:\n{build_out}"
    );

    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs iverilog and vvp on PATH; run with `cargo test -- --ignored`"]
fn the_real_iverilog_drive_judges_a_fatal_and_the_error_hole() {
    if !iverilog_present() {
        eprintln!("skipping: iverilog is not on PATH");
        return;
    }

    // The $fatal testbench, judged FAIL through the whole drive: the
    // face in the log AND vvp's honest exit-1, either witness enough.
    let fail = std::env::temp_dir().join(format!("orbit-contracts-iv-fail-{}", std::process::id()));
    fs::create_dir_all(&fail).unwrap();
    write(
        &fail.join("tb_fatal.v"),
        "\
module tb_fatal;
  initial begin
    #1;
    $fatal(1, \"the design is wrong\");
  end
endmodule
",
    );
    write(&fail.join("blueprint.tsv"), "VLOG\twork\t./tb_fatal.v\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
        &fail,
        "tb_fatal",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "the $fatal run must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("FATAL: ./tb_fatal.v:4: the design is wrong"),
        "the relayed log should carry the probed face verbatim (the \
         snapshot records the blueprint's `./` path form):\n{stdout}"
    );
    assert!(
        stdout.contains("fail (the log carries a probed iverilog failure face)"),
        "the judge should convict on the face:\n{stdout}"
    );
    fs::remove_dir_all(&fail).ok();

    // THE HOLE: a `$error` call. vvp prints the ERROR face, continues
    // the simulation, and EXITS 0 (probed live on vvp 12.0,
    // 2026-11-07) -- the same hole bare ghdl has. The drive must exit 1
    // on the face alone: this is the judge's reason to exist here.
    let err = std::env::temp_dir().join(format!("orbit-contracts-iv-err-{}", std::process::id()));
    fs::create_dir_all(&err).unwrap();
    write(
        &err.join("tb_error.v"),
        "\
module tb_error;
  initial begin
    #1;
    $error(\"check failed\");
    $display(\"simulation continued\");
    $finish;
  end
endmodule
",
    );
    write(&err.join("blueprint.tsv"), "VLOG\twork\t./tb_error.v\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
        &err,
        "tb_error",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "the $error run must exit 1 even though vvp exits 0:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("ERROR: ./tb_error.v:4: check failed"),
        "the relayed log should carry the probed face verbatim (the \
         snapshot records the blueprint's `./` path form):\n{stdout}"
    );
    assert!(
        stdout.contains("simulation continued"),
        "vvp should have continued past the $error:\n{stdout}"
    );
    assert!(
        stdout.contains("fail (the log carries a probed iverilog failure face)"),
        "the judge should convict on the face:\n{stdout}"
    );
    fs::remove_dir_all(&err).ok();
}

#[test]
#[ignore = "needs iverilog and vvp on PATH; run with `cargo test -- --ignored`"]
fn the_real_iverilog_drive_relays_a_compile_failure_and_guards_the_filesets() {
    if !iverilog_present() {
        eprintln!("skipping: iverilog is not on PATH");
        return;
    }

    // A syntax-broken file, through the whole drive: iverilog's own
    // stderr relayed verbatim, exit 1 (the drive's honest findings exit,
    // whatever iverilog's own exit was), and no judged run.
    let broken =
        std::env::temp_dir().join(format!("orbit-contracts-iv-broken-{}", std::process::id()));
    fs::create_dir_all(&broken).unwrap();
    write(
        &broken.join("broken.v"),
        "\
module broken
  input a;
endmodule
",
    );
    write(&broken.join("blueprint.tsv"), "VLOG\twork\t./broken.v\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
        &broken,
        "broken",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(1),
        "a compile failure must exit 1:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("broken.v:2: syntax error"),
        "iverilog's own parse face should be relayed:\n{stderr}"
    );
    assert!(
        stderr.contains("the iverilog compile failed"),
        "the drive should name the failed step:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        !stdout.contains(": running"),
        "a failed compile must not run anything:\n{stdout}"
    );
    fs::remove_dir_all(&broken).ok();

    // The fileset door: a VHDL blueprint is a loud exit-2, never a
    // silent skip.
    let foreign =
        std::env::temp_dir().join(format!("orbit-contracts-iv-foreign-{}", std::process::id()));
    fs::create_dir_all(&foreign).unwrap();
    write(&foreign.join("gate.vhd"), "entity gate is\nend gate;\n");
    write(&foreign.join("blueprint.tsv"), "VHDL\twork\t./gate.vhd\n");

    let run = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-contracts")).arg("iverilog"),
        &foreign,
        "gate",
    )
    .output()
    .unwrap();
    assert_eq!(
        run.status.code(),
        Some(2),
        "a foreign fileset must exit 2:\n{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("the Verilog drives carry the VLOG and SYSV filesets only"),
        "the fileset door should name what IS carried:\n{stderr}"
    );
    fs::remove_dir_all(&foreign).ok();
}
