//! The real cocotb spawn shape, pinned end to end: the REAL cocotb
//! runner (cocotb 2.1.0 over the real ghdl 6.0.0) driven the way orbit
//! drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the working
//! directory at the target's output directory -- across the clean pass,
//! the FAILING TEST caught by the generated script's honest exit (the
//! probed runner exit is a lie: `ghdl -r` exits 0 on a failed test), the
//! `--test-module` gate, and the loud not-carried face for a Verilog
//! blueprint.
//!
//! Ignored by default -- it needs cocotb (>= 2.0, the `cocotb_tools`
//! runner package) and ghdl on PATH; run it with `cargo test --
//! --ignored`. The fixture is the probed one (/tmp/cocotb-probe): a
//! tiny counter DUT and a python module carrying one `@cocotb.test()`
//! coroutine, plus its assert-fail variant.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cocotb_available() -> bool {
    Command::new("python3")
        .args(["-c", "import cocotb, cocotb_tools.runner"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn ghdl_available() -> bool {
    Command::new("ghdl")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

/// The probed fixture: the counter DUT, the passing python test module,
/// and its assert-fail variant.
fn write_fixture(out: &Path) {
    write(
        &out.join("counter.vhd"),
        "\
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity counter is
  port (clk : in std_logic; count : out unsigned(7 downto 0));
end entity counter;

architecture rtl of counter is
  signal value : unsigned(7 downto 0) := (others => '0');
begin
  process (clk)
  begin
    if rising_edge(clk) then
      value <= value + 1;
    end if;
  end process;
  count <= value;
end architecture rtl;
",
    );
    write(
        &out.join("test_counter.py"),
        "\
import cocotb
from cocotb.clock import Clock
from cocotb.triggers import ReadOnly, RisingEdge


@cocotb.test()
async def counts_up(dut):
    cocotb.log.info(\"testing started\")
    Clock(dut.clk, 2, units=\"ns\").start()
    first = dut.count.value
    for _ in range(3):
        await RisingEdge(dut.clk)
    await ReadOnly()
    assert dut.count.value != first, \"the design is wrong\"
    cocotb.log.info(\"all checks passed\")
",
    );
    write(
        &out.join("test_counter_fail.py"),
        "\
import cocotb
from cocotb.clock import Clock
from cocotb.triggers import ReadOnly, RisingEdge


@cocotb.test()
async def expects_the_wrong_count(dut):
    cocotb.log.info(\"testing started\")
    Clock(dut.clk, 2, units=\"ns\").start()
    first = dut.count.value
    for _ in range(3):
        await RisingEdge(dut.clk)
    await ReadOnly()
    assert dut.count.value == 99, \"the design is wrong\"
    cocotb.log.info(\"unreachable\")
",
    );
    write(
        &out.join("gate.v"),
        "module gate(input a, output y); assign y = ~a; endmodule\n",
    );
}

/// The exact spawn shape: no `--top`, `$ORBIT_TB_NAME` exported, the
/// working directory at the target's output directory.
fn spawn_shape<'a>(command: &'a mut Command, out: &Path, tb: &str) -> &'a mut Command {
    command
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", tb)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .current_dir(out)
}

#[test]
#[ignore = "needs cocotb (2.x) and ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_cocotb_drive_runs_a_clean_pass_and_exits_zero() {
    if !cocotb_available() || !ghdl_available() {
        eprintln!("skipping: no cocotb runner or ghdl on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-coco-clean-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_fixture(&out);
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./counter.vhd\n");

    let clean = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("cocotb")
            .arg("--test-module")
            .arg("test_counter.py"),
        &out,
        "counter",
    )
    .output()
    .unwrap();
    assert!(
        clean.status.success(),
        "the clean pass failed:\n{}{}",
        String::from_utf8_lossy(&clean.stdout),
        String::from_utf8_lossy(&clean.stderr)
    );
    let stdout = String::from_utf8_lossy(&clean.stdout);
    assert!(
        stdout.contains("counter: pass"),
        "the clean pass should report pass:\n{stdout}"
    );
    assert!(
        stdout.contains("cocotb summary: 1 test(s), 0 failed"),
        "the generated script's summary should relay:\n{stdout}"
    );
    assert!(
        stdout.contains("TESTS=1 PASS=1 FAIL=0 SKIP=0"),
        "the runner's clean summary should relay:\n{stdout}"
    );
    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs cocotb (2.x) and ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_cocotb_drive_catches_a_failing_test_on_the_honest_exit() {
    if !cocotb_available() || !ghdl_available() {
        eprintln!("skipping: no cocotb runner or ghdl on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-coco-fail-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_fixture(&out);
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./counter.vhd\n");

    // The probed lie, live: the failed test leaves `ghdl -r` at exit 0,
    // and the generated script's results verdict must fail the drive --
    // exit 1, with the ` FAIL ` row face in the relayed log.
    let failing = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("cocotb")
            .arg("--test-module")
            .arg("test_counter_fail.py"),
        &out,
        "counter",
    )
    .output()
    .unwrap();
    assert_eq!(
        failing.status.code(),
        Some(1),
        "the runner's exit-0 failed test must fail on the script's honest exit:\n{}{}",
        String::from_utf8_lossy(&failing.stdout),
        String::from_utf8_lossy(&failing.stderr)
    );
    let stdout = String::from_utf8_lossy(&failing.stdout);
    assert!(
        stdout.contains("counter: fail"),
        "the drive should report the failing test:\n{stdout}"
    );
    assert!(
        stdout.contains("cocotb summary: 1 test(s), 1 failed"),
        "the generated script's summary should relay:\n{stdout}"
    );
    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs cocotb (2.x) and ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_cocotb_drive_requires_the_test_module_before_any_drive() {
    if !cocotb_available() || !ghdl_available() {
        eprintln!("skipping: no cocotb runner or ghdl on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-coco-gate-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_fixture(&out);
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./counter.vhd\n");

    // No --test-module: the exit-2 gate fires BEFORE any drive.
    let gate = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives")).arg("cocotb"),
        &out,
        "counter",
    )
    .output()
    .unwrap();
    assert_eq!(
        gate.status.code(),
        Some(2),
        "a missing test module must exit 2 before any drive:\n{}{}",
        String::from_utf8_lossy(&gate.stdout),
        String::from_utf8_lossy(&gate.stderr)
    );
    assert!(
        String::from_utf8_lossy(&gate.stderr)
            .contains("no test module to drive: pass --test-module <path.py>"),
        "the gate should name the missing flag:\n{}",
        String::from_utf8_lossy(&gate.stderr)
    );

    // The wrong-face rejection is a parse error: ghdl consumes no test
    // module.
    let wrong = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("ghdl")
            .arg("--test-module")
            .arg("test_counter.py"),
        &out,
        "counter",
    )
    .output()
    .unwrap();
    assert_eq!(
        wrong.status.code(),
        Some(2),
        "--test-module on another face must be a usage error:\n{}{}",
        String::from_utf8_lossy(&wrong.stdout),
        String::from_utf8_lossy(&wrong.stderr)
    );
    assert!(
        String::from_utf8_lossy(&wrong.stderr).contains("--test-module is the cocotb face's flag"),
        "the rejection should name the flag's face:\n{}",
        String::from_utf8_lossy(&wrong.stderr)
    );
    fs::remove_dir_all(&out).ok();
}

// The shim is a POSIX mechanism whole -- a `#!/bin/sh` script, the exec
// bit, and PATH resolving the bare name -- so the witness is unix-scoped;
// windows compiles the test out (its subject, the probe's usage error, is
// platform-neutral and covered by the drive's own unit tests).
#[cfg(unix)]
#[test]
fn a_broken_cocotb_install_is_a_loud_usage_error_before_any_runner_script() {
    // The identity probe's exit is a witness (the README promise: a
    // broken cocotb_tools or a 1.x install answers the probe with a
    // nonzero exit -- a loud usage error, never a mid-drive failure).
    // A fake python3 shim exiting 1 stands in for the broken install;
    // the drive must die at the probe, exit 2, relaying the probe's
    // stderr, BEFORE the runner script is written. Hermetic: no real
    // cocotb needed.
    let out = std::env::temp_dir().join(format!("orbit-drives-coco-probe-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write(&out.join("blueprint.tsv"), "VHDL\twork\t./counter.vhd\n");

    let shim_dir = out.join("shim-bin");
    fs::create_dir_all(&shim_dir).unwrap();
    let shim = shim_dir.join("python3");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'broken cocotb_tools on PYTHONPATH' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();

    let probe_fail = Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
        .arg("cocotb")
        .arg("--test-module")
        .arg("test_counter.py")
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", "counter")
        .env("PATH", &shim_dir)
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .current_dir(&out)
        .output()
        .unwrap();
    assert_eq!(
        probe_fail.status.code(),
        Some(2),
        "a nonzero identity probe must be the loud usage error, never a mid-drive \
         failure:\n{}{}",
        String::from_utf8_lossy(&probe_fail.stdout),
        String::from_utf8_lossy(&probe_fail.stderr)
    );
    let stderr = String::from_utf8_lossy(&probe_fail.stderr);
    assert!(
        stderr.contains("cocotb identity probe"),
        "the usage error should name the identity probe:\n{stderr}"
    );
    assert!(
        stderr.contains("broken cocotb_tools on PYTHONPATH"),
        "the usage error should relay the probe's stderr:\n{stderr}"
    );
    assert!(
        !out.join("orbit-cocotb-counter.py").exists(),
        "the drive must die at the probe, before any runner script write"
    );
    fs::remove_dir_all(&out).ok();
}

#[test]
#[ignore = "needs cocotb (2.x) and ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_cocotb_drive_rejects_the_verilog_fileset_loudly() {
    if !cocotb_available() || !ghdl_available() {
        eprintln!("skipping: no cocotb runner or ghdl on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!("orbit-drives-coco-nc-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    write_fixture(&out);
    write(&out.join("blueprint.tsv"), "VLOG\twork\t./gate.v\n");

    // The ghdl backend compiles VHDL only (probed): a VLOG blueprint is
    // the loud not-carried face, exit 2, before any runner spawn.
    let not_carried = spawn_shape(
        Command::new(env!("CARGO_BIN_EXE_orbit-drives"))
            .arg("cocotb")
            .arg("--test-module")
            .arg("test_counter.py"),
        &out,
        "gate",
    )
    .output()
    .unwrap();
    assert_eq!(
        not_carried.status.code(),
        Some(2),
        "a VLOG blueprint must be the loud not-carried face:\n{}{}",
        String::from_utf8_lossy(&not_carried.stdout),
        String::from_utf8_lossy(&not_carried.stderr)
    );
    assert!(
        String::from_utf8_lossy(&not_carried.stderr)
            .contains("the cocotb drive carries the VHDL fileset"),
        "the not-carried face should name the drive's fileset:\n{}",
        String::from_utf8_lossy(&not_carried.stderr)
    );
    fs::remove_dir_all(&out).ok();
}
