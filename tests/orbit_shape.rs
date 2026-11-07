//! The real spawn shape, pinned end to end: the REAL binary driven the way
//! orbit drives it -- no `--top`, `$ORBIT_TB_NAME` exported, the user's
//! extras riding the argv bare with orbit's `--` separator stripped --
//! against REAL ghdl, twice in a row in the same persistent output
//! directory. Both defects that bounced this crate in review lived exactly
//! here, unpinned: a target that could not run through `orbit test` at
//! all, and a second run that obsoleted its own design out from under
//! `ghdl -e`.
//!
//! Ignored by default -- it needs ghdl on PATH; run it with `cargo test
//! -- --ignored`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn ghdl_present() -> bool {
    Command::new("ghdl").arg("--version").output().is_ok()
}

fn write(path: &PathBuf, text: &str) {
    fs::write(path, text).unwrap();
}

#[test]
#[ignore = "needs ghdl on PATH; run with `cargo test -- --ignored`"]
fn the_real_orbit_spawn_shape_runs_twice_in_one_output_directory() {
    if !ghdl_present() {
        eprintln!("skipping: ghdl is not on PATH");
        return;
    }

    let out = std::env::temp_dir().join(format!(
        "orbit-contracts-orbit-shape-{}",
        std::process::id()
    ));
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
    // working directory at the target's output directory.
    let mut command = Command::new(env!("CARGO_BIN_EXE_orbit-contracts"));
    command
        .args(["ghdl", "--vcd=waves.vcd"])
        .env("ORBIT_BLUEPRINT", out.join("blueprint.tsv"))
        .env("ORBIT_TB_NAME", "tb")
        .env_remove("ORBIT_BLUEPRINT_PLAN")
        .current_dir(&out);

    let first = command.output().unwrap();
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

    // The second run in the SAME directory: the stale work library must
    // not obsolete the design out from under `ghdl -e`.
    let second = command.output().unwrap();
    assert!(
        second.status.success(),
        "run 2 failed:\n{}{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    fs::remove_dir_all(&out).ok();
}
