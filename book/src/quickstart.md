# Quick start

## Install

The drives ship as one binary:

```sh
cargo install --path .
```

No dependencies beyond the Rust standard library.

## Wiring into orbit

Include the shipped target fragment into an orbit project configuration:

```sh
orbit config --push include=/path/to/orbit-drives/config.toml
```

Every shipped target entry carries the custom recursive `ALLHDL`
fileset: the FULL HDL file set of every project in the dependency graph
rides the blueprint regardless of orbit's reference closure -- the
default-bound entities a reference walk cannot see ride like any other
file (the why and the probed semantics live in the honest limits'
fileset bullet). The drives route ALLHDL rows by their file extension.

Then run the target:

```sh
orbit test --target ghdl -- -gW=4 --vcd=waves.vcd
```

Orbit spawns the target in the target's output directory with
`ORBIT_BLUEPRINT` pointing at the plan file (the TSV plan: one
`fileset<TAB>library<TAB>filepath` line per collected file),
`ORBIT_TB_NAME` set to the designated testbench, and
`ORBIT_PROJECT_LIBRARY` set to the working project's HDL library (the
voice the VHDL drives' elaborate/run argv resolves first). Orbit consumes
its own `--` separator and appends the extras straight onto the target's
argv, so the drive passes every argument it does not own through to each
`ghdl -r` verbatim.

## Usage

One subcommand per drive -- the subcommands are what orbit's targets
spawn:

```
orbit-drives <SUBCOMMAND> --blueprint <path> --top <name> [extra tool args...]
```

Subcommands (one per orbit target entry):

- `ghdl` -- the ghdl sim face: analyze, elaborate, run, judge.
- `ghdl-build` -- the ghdl build face: analyze, elaborate, exit honest.
  No simulation, no run log, nothing judged: `ghdl -a` and `ghdl -e` exit
  nonzero on a broken design (probed live), so the exit is the only
  witness a build needs.
- `nvc` -- the nvc sim face: the same judged drive on nvc's probed faces.
- `nvc-build` -- the nvc build face: the same honest shape, no simulation,
  nothing judged.
- `iverilog` -- the iverilog sim face: one order-free compile into a
  snapshot, then one judged `vvp` run. vvp exits 0 on a `$error` call
  (probed live), so the vocabulary is the load-bearing gate here.
- `iverilog-build` -- the iverilog build face: the one compile only, exit
  honest. No simulation, no run log, nothing judged: iverilog's compile
  exits are honest (exit 2 on a parse error, exit 1 on an unknown `-s`
  root, probed live), and every `--top` is validated against the source
  at compile time.
- `verilator` -- the verilator sim face: one lint, one model build per
  top, then each model run judged. Every probed model failure dies by
  SIGABRT (no exit code at all), so the exit convicts and the judge is
  the second witness.
- `verilator-build` -- the verilator build face: the lint and the
  per-top model builds only, exit honest. No simulation, nothing judged:
  verilator's lint exits are honest (exit 1 on a lint error and on an
  enabled warning, probed live), and every `--top` is validated by its
  own per-top build -- the lint honors only the LAST `--top-module`
  (probed: an unknown non-last top lints clean, exit 0), and an unknown
  top fails its build loudly.
- `yosys` -- the yosys synthesis face: one script-driven `yosys -p` per
  top (read every blueprint Verilog file, `synth -top`, write
  `netlists/<top>.v`), the log judged on yosys's probed `ERROR:`
  vocabulary and the exit. Build only: yosys has no runtime, there is no
  sim face and nothing runs.
- `vivado-build` -- the vivado build face: both languages compiled into
  `work` (the shared single-work compile), then per top the GENERATED
  synthesis script (`read_vhdl -library <lib>` per VHDL file, the
  blueprint's library column) through `vivado -mode batch -nojournal
  -nolog`, the three netlists the payload. Requires `--part <part>`
  BEFORE any drive (exit 2 at the CLI); no simulation, nothing judged.
- `vivado` -- the vivado sim face: the SINGLE-WORK face (every file
  compiles into `work`, the blueprint's named libraries mapped through
  the drive's `xsim.ini`), then xelab per top (the bare top) and one
  xsim run per snapshot, every run log gated on vivado's probed
  LINE-START failure vocabulary AND the exit. xsim exits 0 on BOTH
  probed runtime failures, so the judge is the load-bearing gate here.
- `quartus-build` -- the quartus build face, DOCUMENTED-ONLY: per top a
  GENERATED `.qsf` (DEVICE from `--part`, TOP_LEVEL_ENTITY, one
  `VHDL_FILE` / `VERILOG_FILE` line per blueprint file) and one batch
  `quartus_map --read_settings_files=off --write_settings_files=off
  <top> -c <top>`, exit honest. Nothing on this face is probed -- quartus
  is not installed on the probing box; every hypothesis carries its
  first-contact rule in the honest limits.
- `quartus` -- the quartus sim face, DOCUMENTED-ONLY: the bundled Questa
  -- `vlib work`, `vmap work work`, `vcom -2008 -work work` per VHDL file
  through the deferral loop, then per top one
  `vsim -c -do "run -all; quit" -L work work.<top>`, every run log gated
  on the hypothesized ModelSim-family LINE-START vocabulary
  (`Fatal:` / `Error:` / `Failure:`, UNPROBED) AND the exit (vsim honest,
  UNPROBED). Neither witness is trusted alone.
- `cocotb` -- the cocotb test face, LIVE: cocotb's own python runner
  (`cocotb_tools.runner` over ghdl -- the runner IS the compile) driven
  by a GENERATED runner script per top, which reads results.xml and
  exits honest -- the load-bearing exit, probed: the runner exits 0 on a
  failed test (the finding lands only in results.xml). Every run log is
  gated on cocotb's probed failure vocabulary (the space-padded
  ` FAIL ` row column, the missing-results face) AND the script's exit.
  Carries the VHDL fileset only (the ghdl backend compiles VHDL only,
  probed).

Options:

- `--blueprint <path>` -- the TSV blueprint; defaults to `$ORBIT_BLUEPRINT`.
- `--top <name>` -- a top unit to elaborate (and run: `ghdl`); repeatable.
  With no `--top`, the drive runs `$ORBIT_TB_NAME` (orbit exports it; empty
  in all-mode counts as absent). Exit 2 when neither names a top.
- `--part <part>` -- the FPGA part the build faces synthesize for (e.g.
  `xc7z010clg400-1`). REQUIRED on `vivado-build` (the probed
  alternative: a ~4s synth-then-fail inside the tool) and on
  `quartus-build` (the documented alternative: an unmapped-fabric
  refusal inside `quartus_map`, UNPROBED) -- the gate fires before any
  drive, exit 2 naming the flag -- and REJECTED on every other face
  (loud exit-2: the sim faces consume no part, and the other tools do
  not speak one).
- `--test-module <path>` -- the python test module the `cocotb` face
  drives (a path to a `.py` file carrying `@cocotb.test()`
  coroutines). REQUIRED on `cocotb` -- the gate fires before any drive,
  exit 2 naming the flag -- and REJECTED on every other face (loud
  exit-2: no other drive imports a python module). The file stem names
  the module the runner imports; its directory rides the spawned
  python's `PYTHONPATH`.
- anything else -- a tool extra, appended verbatim to the drive's LAST
  tool argv: each `ghdl -r` / `nvc -r` / `vvp` / `xsim` / `vsim` on the
  sim faces, each `ghdl -e` / `nvc -e` / the `iverilog` compile / the
  `yosys -p` synth / `vivado -mode batch` / `quartus_map` on the build
  faces, and on cocotb the runner's `test_args` (the backing simulator's
  run argv). There is no later step to carry them, and a dropped
  argument would be a silent degradation; an option the tool does not
  speak is the tool's own loud rejection, surfaced as an exit-1
  failure.

Exits:

| code | meaning |
|------|---------|
| 0 | clean -- every run judged clean and exited zero, or every top elaborated clean |
| 1 | findings -- a log carried a failure face, a run exited nonzero, or the tool reported a broken design |
| 2 | usage -- bad argv, no top to run, unreadable blueprint, a fileset the drive does not carry |

