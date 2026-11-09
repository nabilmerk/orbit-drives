# cocotb drive


ONE target entry (`cocotb`) drives cocotb -- the test face only: there
is no build face and no compile step of this drive's own, because
cocotb's own runner IS the compile. The shapes the probes wrote down
(cocotb 2.1.0 + ghdl 6.0.0, 2026-11-07):

- **The runner is a python API, not a module invocation.**
  `python -m cocotb.runner` does not exist in cocotb 2.x (the module
  was retired with the 2.0 split, and the 1.x Makefile flow with it --
  no `TOPLEVEL` / `SIM` env vars; 2.x renames them `COCOTB_TOPLEVEL` /
  `COCOTB_TEST_MODULES`, set by the runner itself). The runner lives in
  the separate `cocotb_tools` distribution package:
  `cocotb_tools.runner.get_runner("ghdl")`, then `runner.build(...)` and
  `runner.test(...)`. A 1.x install answers the drive's identity probe
  (`python3 -c "import cocotb, cocotb_tools.runner"`) with a nonzero
  exit -- a loud usage error, never a mid-drive failure.
- **The runner invocation is GENERATED per top** -- the
  vivado-generated-synth-script precedent, in python: the drive writes
  `orbit-cocotb-<top>.py` (get_runner("ghdl"), `runner.build(sources=...,
  hdl_toplevel=<top>, build_dir="sim_build", always=True)`,
  `runner.test(test_module=<stem>, hdl_toplevel=<top>, ...)`) and spawns
  it once per top. The file paths ride the script as escaped python
  string literals -- machine-written python carries them EXACTLY, so
  the whitespace-split v1 limit the other script faces document does
  not exist here.
- **The exit contract is probed, and the runner lies by omission:**
  `ghdl -r` exits 0 on a FAILED cocotb test -- the finding lands only
  in results.xml -- so the generated script reads the results itself
  (`cocotb_tools.check_results.get_results`) and exits honest: nonzero
  on any failed test. That script exit is the load-bearing exit
  witness, probed into existence by the runner's own dishonesty; the
  vocabulary is the second witness on the same log. A simulator-level
  crash DOES exit the runner nonzero (`Simulation failed: <rc>`), and
  the missing-results face
  (`ERROR: Simulation terminated abnormally. Results file ... not
  found.`) relays through the script -- a test module that fails to
  import, or carries no tests, lands there.
- **The vocabulary is cocotb's own aggregate faces, never the backing
  simulator's markers.** The probed faces: the space-padded ` FAIL `
  row column of the runner's regression summary table (the padding is
  load-bearing -- the clean summary's own `FAIL=0` counter shares no
  shape with it, so the summary never judges a clean run), and the
  missing-results face above. A cocotb log is a ghdl log, and ghdl's
  lowercase faces belong to the ghdl drive; cocotb aggregates.
- **The test module rides the argv, not the blueprint.**
  `--test-module <path.py>` is REQUIRED (exit 2 before any drive,
  naming the flag; rejected on every other face). The file stem names
  the module the runner imports; its directory rides the spawned
  python's `PYTHONPATH`, because the runner passes its own `sys.path`
  down to the simulation. The `PYTHONPATH` join is POSIX (`:`) -- v1
  drives POSIX boxes. The extras ride the script's `test_args` --
  the runner's documented channel to the backing simulator's run argv.
- **The fileset door is probed shut.** The ghdl backend compiles VHDL
  only -- the runner itself refuses every other source (`ValueError:
  Ghdl only supports VHDL. ...` in its own `_build_command`), so the
  drive carries the `VHDL` fileset alone and every other entry is the
  loud not-carried face (exit 2).
- **The build directory (`sim_build/`) is wiped before every drive** --
  the same correctness-before-incrementality stance, though probed
  honestly: the wipe is symmetry here (the runner unlinks the results
  file before each test and `always=True` rebuilds).

