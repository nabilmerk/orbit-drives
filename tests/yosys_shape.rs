//! The real yosys spawn shape, pinned end to end: the REAL binary driven
//! the way orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the
//! working directory at the target's output directory -- against REAL
//! yosys, across the order-in-spirit blueprint, a judged synth failure
//! (the drive exits 1 and relays the probed face), the unknown-top
//! refusal, the build-only payload (the netlist under `netlists/`), and
//! the VHDL fileset that must be the loud not-carried face.
//!
//! Ignored by default -- it needs yosys on PATH; run it with
//! `cargo test -- --ignored`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn yosys_present() -> bool {
    Command::new("yosys").arg("-V").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The provider and consumer of the order-in-spirit fixture: yosys reads
/// all files into one design and elaborates at `synth` time (probed live
/// on Yosys 0.52, 2026-11-07), so this pair synthesizes clean in one pass
/// -- the pin is that NO deferral loop is needed here.
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
#[ignore = "needs yosys on PATH; run with `cargo test -- --ignored`"]
fn the_real_yosys_spawn_shape_synthesizes_twice_and_leaves_the_netlist() {
    if !yosys_present() {
        eprintln!("skipping: yosys is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-yos-shape-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // gate <- top, handed over consumer-first: the order a stateful
    // backend cannot forgive, and yosys's one design does not care about
    // (probed) -- one synth, no deferral loop.
    write_pair(&out);
    write(
        &out.join("blueprint.tsv"),
        "VLOG\twork\t./top.v\nVLOG\twork\t./gate.v\n",
    );

    // The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
    // working directory at the target's output directory.
    let first = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("yosys"),
        &out,
        "top",
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
        stdout.contains("synthesizing 2 Verilog file(s), order-free"),
        "the drive should synthesize the whole fileset once:\n{stdout}"
    );
    assert!(
        stdout.contains("top: clean (netlists/top.v written)"),
        "the drive should report the netlist payload:\n{stdout}"
    );
    assert!(
        out.join("netlists").join("top.v").exists(),
        "the netlist payload should land under netlists/"
    );

    // The second run in the same persistent output directory: the payload
    // wipe is symmetry (probed honestly), so run 2 must be clean too.
    let second = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("yosys"),
        &out,
        "top",
    )
    .output()
    .unwrap();
    assert!(
        second.status.success(),
        "run 2 failed:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );
}

#[test]
#[ignore = "needs yosys on PATH; run with `cargo test -- --ignored`"]
fn the_real_yosys_drive_judges_a_failed_synth_and_names_the_top() {
    if !yosys_present() {
        eprintln!("skipping: yosys is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-yos-fail-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // The syntax face, probed live on Yosys 0.52 (2026-11-07): the ERROR
    // marker rides mid-line behind the source location, on stderr, and
    // the whole `-p` script aborts with exit 1 -- no netlist written.
    write(
        &out.join("broken.v"),
        "\
module broken(input a, output y);
  assign y = ~a
endmodule
",
    );
    write(&out.join("blueprint.tsv"), "VLOG\twork\t./broken.v\n");

    let drive = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("yosys"),
        &out,
        "broken",
    )
    .output()
    .unwrap();
    assert_eq!(
        drive.status.code(),
        Some(1),
        "a judged synth failure must exit 1:\n{}{}",
        String::from_utf8_lossy(&drive.stdout),
        String::from_utf8_lossy(&drive.stderr)
    );
    let stderr = String::from_utf8_lossy(&drive.stderr);
    assert!(
        stderr.contains("ERROR: syntax error, unexpected"),
        "the probed face must relay verbatim:\n{stderr}"
    );
    assert!(
        stderr.contains("the yosys synth failed for broken"),
        "the error line should name the top:\n{stderr}"
    );
    assert!(
        stderr.contains("carries a probed yosys failure face"),
        "the judge should witness the face:\n{stderr}"
    );
    assert!(
        !out.join("netlists").join("broken.v").exists(),
        "a failed synth must write no netlist"
    );
}

#[test]
#[ignore = "needs yosys on PATH; run with `cargo test -- --ignored`"]
fn the_real_yosys_drive_refuses_an_unknown_top_at_its_synth() {
    if !yosys_present() {
        eprintln!("skipping: yosys is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-yos-top-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // A clean file, a top that names nothing in it: the script's `synth
    // -top` refusal, probed live (`ERROR: Module \`nosuch' not found!`,
    // exit 1). The top is validated by its own synth on this face.
    write(
        &out.join("gate.v"),
        "\
module gate(input a, output y);
  assign y = ~a;
endmodule
",
    );
    write(&out.join("blueprint.tsv"), "VLOG\twork\t./gate.v\n");

    let drive = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("yosys"),
        &out,
        "nosuch",
    )
    .output()
    .unwrap();
    assert_eq!(
        drive.status.code(),
        Some(1),
        "an unknown top must fail its synth:\n{}{}",
        String::from_utf8_lossy(&drive.stdout),
        String::from_utf8_lossy(&drive.stderr)
    );
    let stderr = String::from_utf8_lossy(&drive.stderr);
    assert!(
        stderr.contains("ERROR: Module `nosuch' not found!"),
        "the probed unknown-top face must relay verbatim:\n{stderr}"
    );
    assert!(
        stderr.contains("the yosys synth failed for nosuch"),
        "the error line should name the top:\n{stderr}"
    );
}

#[test]
#[ignore = "needs yosys on PATH; run with `cargo test -- --ignored`"]
fn the_real_yosys_drive_rejects_the_vhdl_fileset_loudly() {
    if !yosys_present() {
        eprintln!("skipping: yosys is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-yos-vhd-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();

    // yosys is a Verilog synthesis engine: VHDL rides nothing here -- the
    // loud not-carried face (exit 2), never a silent skip.
    write(&out.join("gate.vhd"), "entity gate is end entity gate;\n");
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./gate.vhd\n");

    let drive = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("yosys"),
        &out,
        "gate",
    )
    .output()
    .unwrap();
    assert_eq!(
        drive.status.code(),
        Some(2),
        "a not-carried fileset must exit 2:\n{}{}",
        String::from_utf8_lossy(&drive.stdout),
        String::from_utf8_lossy(&drive.stderr)
    );
    let stderr = String::from_utf8_lossy(&drive.stderr);
    assert!(
        stderr.contains("the Verilog drives carry the VLOG and SYSV filesets only"),
        "the not-carried face must name the filesets:\n{stderr}"
    );
    assert!(
        stderr.contains("gate.vhd"),
        "the not-carried face must name the file:\n{stderr}"
    );
}
