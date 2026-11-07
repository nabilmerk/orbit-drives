# orbit-drives

orbit-drives removes the scripting from your orbit targets: orbit plans
your dependencies into a blueprint — the drives execute it, tool by tool,
each carrying its probed failure vocabulary and an honest exit. You write
neither the plan nor the scripts.

The honest-exit target drives for the
[orbit](https://github.com/chaseruskin/orbit) HDL package manager. Each
drive captures the tool's log and gates it on the tool's **probed failure
vocabulary** (faces copied from live runs, never invented) **and** on the
tool's exit code: findings = a face present OR a nonzero exit. Either
witness convicts. The judge exists because the exits lie: bare ghdl exits
0 on a violated `severity error` check -- it prints the face and
continues -- and the exit exists as a witness because a killed simulation
can leave no face at all.

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
- `vivado-build` -- the vivado build face: both languages compiled
  (xvhdl per VHDL file into its own blueprint library with the deferral
  loop, one order-free xvlog into `work`), then per top the GENERATED
  synthesis script (`read_vhdl -library <lib>` per VHDL file) through
  `vivado -mode batch -nojournal -nolog`, the three netlists the payload.
  Requires `--part <part>` BEFORE any drive (exit 2 at the CLI); no
  simulation, nothing judged.
- `vivado` -- the vivado sim face: the same compile, then xelab per top
  (one `-L` per carried library, the top under its own blueprint
  library) and one xsim run per snapshot, every run log gated on
  vivado's probed LINE-START failure vocabulary AND the exit. xsim exits
  0 on BOTH probed runtime failures, so the judge is the load-bearing
  gate here.
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

## The ghdl drive

Two target entries (`ghdl`, `ghdl-build`) share one drive, split by how
far it goes: the build face stops after step 4, the sim face runs on
through the judge. The drive consumes the blueprint's library column (the
§25 multi-library face, probed live on ghdl 6.0.0, 2026-11-07) -- the
same face the nvc drive carries, on ghdl's own resolution:

1. Read the blueprint; every non-VHDL entry (`VLOG`, `SYSV`, a custom
   fileset) is a loud exit-2, never a silent skip.
2. Wipe the stale library state (every `<library>-obj08.cf` in the output
   directory): ghdl's library file is its clock -- one file per analyzed
   library now, named after the standard (`-obj08`) and LOWERCASED by
   ghdl (`--work=MyLib` writes `mylib-obj08.cf`, probed). A second run in
   orbit's persistent output directory would otherwise obsolete its own
   design out from under `ghdl -e`; and a library a previous blueprint
   analyzed but this one no longer names must die too, or a
   `library <other>;` clause would silently resolve against its stale
   units -- the scan, not the blueprint's library list, is what makes the
   wipe honest. Correctness before incrementality in v1 -- every drive
   starts from empty libraries.
3. Analyze the VHDL files in blueprint order with the **deferral loop**,
   each file into the library its own blueprint row declares
   (`ghdl -a --std=08 --work=<entry.library> <file>`): ghdl is
   order-sensitive, so a failing analyze defers its file to a retry after
   the others; a pass that makes no progress stops the loop with every
   residual stderr surfaced verbatim. Cross-library references defer
   across libraries exactly as they defer within one: pass 1's
   `cannot find resource library "<other>"` face is the absent
   `<other>-obj08.cf` file, and the retry resolves once the provider's
   row has written it.
4. Elaborate each top (`ghdl -e --std=08 --work=<the top's library> <top>
   <extras>` on the build face; the extras ride the elab argv there
   because there is no run to carry them). A nonzero elaboration is a
   finding, exit 1.
5. (`ghdl` only) Run each top (`ghdl -r --std=08 --work=<the top's
   library> <top> --assert-level=error <extra args>`; the assert level is
   a simulation option, legal only after the toplevel unit) and capture
   the log.
6. (`ghdl` only) Gate the run: a probed failure face in the log OR a
   nonzero ghdl exit is a finding. Either witness convicts.

The multi-library facts the probes wrote down:

- **No search-path flag exists or is needed.** ghdl resolves a
  consumer's `library <other>;` clause against the sibling
  `<other>-obj08.cf` file in the SAME working directory, automatically --
  `ghdl -a --std=08 --work=work gate.vhd` analyzing clean against
  `grlib-obj08.cf` with no `-P`/`-L` anywhere (probed; move the sibling
  into a subdirectory and the resolution dies: ghdl's library search is
  the cwd alone). Orbit already spawns the drive with the output
  directory as the cwd, where every row's file lands.
- **The elaborate and run argv pin the top's library** (`--work=<lib>`),
  because ghdl's bare-top commands search the WORK library only (`-e tb`
  with the top analyzed into any other library: `error: cannot find
  entity or configuration tb`, exit 1, probed). The resolution, loudest
  voice first: `$ORBIT_PROJECT_LIBRARY` (orbit sets it on every spawned
  target to the working project's library; the designated testbench is a
  working-project file); then the blueprint's single carried library
  (direct invocation without orbit); then ghdl's default `work` -- a
  guess that cannot pass silently, because the wiped `work` library
  cannot hold a stale unit: a wrong guess dies loudly on ghdl's own
  elaboration refusal.
- **A library no blueprint row populates fails loudly at analyze.** A
  design naming a THIRD library (not one the planner carried) dies on
  `cannot find resource library "<other>"` / `unit "<unit>" not found in
  library "<other>"`, the deferral loop exhausts with no progress, and
  the residual stderr relays the faces verbatim, exit 1 -- never a silent
  blackbox degradation. Every library the blueprint carries is analyzed,
  so this face cannot fire for one.
- **The argv shape changed with the multi-library face** (2026-11-07):
  every project's analyze/elab/run now carries `--work=<library>` -- a
  depless single-library project's analyze went from
  `ghdl -a --std=08 f.vhd` to `ghdl -a --std=08 --work=work f.vhd`, its
  elaboration from `ghdl -e --std=08 top` to
  `ghdl -e --std=08 --work=work top`, its run likewise. Nothing about the
  judged faces moved with it; the probed sequences below carry the new
  argv verbatim, and the single-library live tests drive the new shape
  end to end.
- **VHDL only**, like every VHDL drive: the `VLOG`/`SYSV` filesets are
  the loud exit-2 refusal, and ghdl's Verilog face is not carried.

## The nvc drive

Two target entries (`nvc`, `nvc-build`) share one drive with the same
shapes as ghdl's -- blueprint, deferral-loop analysis, elaboration per
top, judged run -- on nvc's probed process and faces, consuming the
blueprint's library column (the §25 multi-library face, probed live on
nvc 1.23.0, 2026-11-07). The differences the probes wrote down:

- **The argv is option-first, the opposite of ghdl.** `--std=2008`,
  `-L .`, and `--work=<library>` are GLOBAL options and must LEAD:
  `nvc --std=2008 -L . --work=grlib -a f.vhd`. After the command, nvc
  answers `** Fatal: unrecognised analysis option --std=2008` (exit 1).
  Run options are legal after the toplevel unit (`nvc --std=2008 -L .
  --work=work -r top --stop-time=1ns`, probed clean), so the extras ride
  there.
- **Multi-library: the library column is load-bearing.** Every analyze
  rides `--work=<entry.library>` -- the blueprint's library column
  consumed verbatim (the §25 ruling: the library name is the project's
  declared name, orbit's planner is its source of truth -- and orbit
  defaults a project's library to the project's NAME, not to any tool's
  `work`). nvc creates one directory per analyzed library in the output
  directory (`grlib/`, `work/`, ...), and EVERY nvc spawn -- analyze,
  elaborate, run -- carries `-L .` so a consumer's `library <other>;`
  clause resolves against them: without it the search path is nvc's
  install dirs alone, and the analyze dies on `** Error: library
  <OTHER> not found`. Libraries analyze in ANY blueprint order -- the
  search path makes cross-library references resolve once both sides are
  analyzed, which is what the deferral loop (unchanged, per-file) rides:
  a consumer defers across libraries exactly as it defers within one.
- **The elaborate and run argv pin the top's library** (`--work=<lib>`),
  because nvc's bare-top commands search the WORK library ONLY (`-e tb`
  with the top analyzed into any other library: `** Fatal: cannot find
  unit WORK.TB in library WORK`, exit 1, probed -- even with `-L .`).
  The resolution, loudest voice first: `$ORBIT_PROJECT_LIBRARY` (orbit
  sets it on every spawned target to the working project's library; the
  designated testbench is a working-project file); then the blueprint's
  single carried library (direct invocation without orbit); then nvc's
  default `work` -- a guess that cannot pass silently, because the wiped
  `work` library cannot hold a stale unit: a wrong guess dies loudly on
  nvc's own elaboration refusal.
- **The library state is per-library directories, and the wipe takes
  every one of them**: the drive removes each directory in the output
  directory carrying nvc's own `_NVC_LIB` marker (nvc writes it even
  when the analysis into the fresh library fails, probed -- it is nvc's
  own library-recognition rule). The scan, not the blueprint's library
  list, is what makes it honest: a library a PREVIOUS drive analyzed but
  this blueprint no longer names must die too, or a `library <other>;`
  clause would silently resolve against its stale units. Unlike ghdl's,
  the wipe was never load-bearing against nvc itself (nvc tolerates
  stale-library re-analysis, probed) -- with libraries in the blueprint
  it IS load-bearing against the drive's own resolution.
- **A library no blueprint row populates fails loudly at analyze.** A
  design naming a THIRD library (not one the planner carried) dies on
  `** Error: library <OTHER> not found` / `** Fatal: required library
  <OTHER> not found`, the deferral loop exhausts with no progress, and
  the residual stderr relays both faces verbatim, exit 1 -- never a
  silent blackbox degradation. Every library the blueprint carries is
  analyzed, so this face cannot fire for one.
- **No `--assert-level` analogue is pinned.** None is needed: nvc's run
  exits probed honest on every failure face (below), and the judge gates
  the log regardless.
- **The argv shape changed with the multi-library face** (2026-11-07):
  every project's argv now carries `-L .` and `--work=<library>` -- a
  depless single-library project's analyze went from
  `nvc --std=2008 -a f.vhd` to `nvc --std=2008 -L . --work=work -a
  f.vhd`, its elaboration from `nvc --std=2008 -e top` to `nvc
  --std=2008 -L . --work=work -e top`, its run likewise. Nothing about
  the judged faces moved with it; the probed sequences below carry the
  new argv verbatim, and the single-library live tests drive the new
  shape end to end.
- **VHDL only**, like every VHDL drive: the `VLOG`/`SYSV` filesets are
  the loud exit-2 refusal, and nvc's Verilog face is not carried.

## The iverilog drive

Two target entries (`iverilog`, `iverilog-build`) share one drive on
iverilog's probed process. The differences the probes wrote down:

- **No deferral loop, on probed grounds.** iverilog's compile is a single
  invocation over all files and it is order-free: a
  consumer-before-provider pair compiled clean (probed live on Icarus
  Verilog 12.0, 2026-11-07). One compile, no passes -- the machinery the
  stateful VHDL backends need has nothing to do here.
- **One compile, one run.** Every requested `--top` rides a `-s` root
  flag on the single compile, and `vvp` runs the one snapshot once with
  all the roots; multiple testbenches share one simulation. iverilog
  validates every `-s` name against the source at compile time
  (`error: Unable to find the root module "nosuch_top" in the Verilog
  source.`, exit 1), so an unknown top fails the compile loudly on both
  faces.
- **The generation is pinned** (`-g2012`, SystemVerilog-2012): the
  generation where `$fatal`/`$error` are first-class, and the generation
  under which the runtime faces were probed. (Probed honestly: iverilog
  12.0's default generation also accepts `$fatal` at compile time; the
  pin keeps the judged faces from drifting with the default.)
- **The snapshot (`orbit-snapshot.vvp`) is wiped before every drive** --
  the same correctness-before-incrementality stance as the work
  libraries, though probed honestly: a failed compile exits 1 before the
  sim face could run a stale snapshot, so the wipe is symmetry here, not
  load-bearing.
- **The judge is the load-bearing gate.** vvp exits 0 on a `$error` call
  (below) -- the bare-ghdl hole again -- so the exit alone would pass a
  failing run; the face in the log is the only witness. The compile
  needs no judge: iverilog's compile exits are honest.

## The verilator drive

Two target entries (`verilator`, `verilator-build`) share one drive on
verilator's probed process. The differences the probes wrote down:

- **No deferral loop, on probed grounds.** verilator reads all files into
  one elaboration and is order-free: a consumer-before-provider pair
  linted and built clean (probed live on Verilator 5.032,
  2026-11-07). One lint, then per-top builds -- the machinery the
  stateful VHDL backends need has nothing to do here.
- **`--timing` is pinned on every verilator argv.** Without it, a
  delay-carrying file fails the lint
  (`%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or --no-timing ...`,
  exit 1, probed live). The drive never relies on the flag's absence:
  the vocabulary's bare `%Error` face subsumes the coded form either way.
- **One lint, per-top builds, per-top runs.** The lint runs once over all
  files with every requested `--top` a `--top-module` (repeated flags
  probed legal; but verilator honors only the LAST one -- probed: an
  unknown non-last top lints clean, exit 0). Every top is validated by
  its own per-top build on both faces: an unknown top answers
  `%Error: Specified --top-module 'nosuch' was not found in design.`,
  exit 1, at its build (`verilator --binary --build --timing
  --top-module <top>`, landing `obj_dir/V<top>`) -- and -- on the sim
  face -- each top then gets its own judged run: each testbench is its
  own simulation, as on the VHDL faces. The model binaries of different
  tops coexist in the one `obj_dir` (probed side by side).
- **The language is verilator's default** -- no `--language` pin in v1;
  the probed fixtures are plain enough to sit inside it. The pinned flags
  are the probed ones: `--lint-only --timing` on the lint face,
  `--binary --build --timing` on the build face.
- **The model directory (`obj_dir/`) is wiped before every drive** --
  the same correctness-before-incrementality stance, though probed
  honestly: `--build` regenerates the intermediates and a failed build
  exits 1 before the sim face could run a stale model, so the wipe is
  symmetry here, not load-bearing.
- **The exit is the first witness, the judge the second.** Every model
  failure probed dies by SIGABRT -- no exit code at all (the shell
  reports `Aborted`; the drive records -1) -- so the exit convicts on
  every failure seen, and the faces (`%Fatal:`, `%Error:`) convict
  regardless. The lint and build faces need no judge: verilator's lint
  exits are honest -- including on an *enabled warning*
  (`%Warning-WIDTHTRUNC ...` still exits 1 under `-Wall` and even under
  `-Wwarn-`, probed live), so a warning face never rides an exit-0 lint.

## The yosys drive

ONE target entry (`yosys`) drives yosys -- build only, the synthesis
face: yosys is a synthesis engine with no runtime, so there is no sim
face, no run, and the drive's whole output is the per-top netlist and
its exit. The shapes the probes wrote down:

- **No deferral loop, on probed grounds.** yosys reads all files into
  one design and elaborates at `synth` time: a consumer-before-provider
  pair synthesized clean (probed live on Yosys 0.52, 2026-11-07). One
  script per top, no passes -- the machinery the stateful VHDL backends
  need has nothing to do here.
- **One script-driven process per top.** `yosys -p "read_verilog <files>;
  synth -top <top>; write_verilog netlists/<top>.v"` -- yosys runs the
  script left to right and aborts on the first error, so the netlist is
  written only on a fully clean synth. The script validates its top
  (`ERROR: Module \`nosuch' not found!`, exit 1, probed), and each top
  gets a fresh process because `synth` consumes the design it runs on --
  a second top could not synthesize over the first one's mapped netlist.
- **The exit is the first witness, the judge the second.** yosys's exit
  is honest so far as probed: every probed failure exits 1 (a script
  error aborts the whole `-p` script). The judge stays the gate because
  the exit cannot name the face; either witness convicts.
- **The streams split.** yosys writes its whole synthesis log to stdout
  and its failure faces to stderr (probed); the drive joins the streams,
  so the face reads either way. The full log relays only on failure --
  on success the drive reports the netlist, not seventeen kilobytes of
  synthesis log.
- **The netlist directory (`netlists/`) is wiped and recreated before
  every drive** -- the same correctness-before-incrementality stance,
  though probed honestly: a failed synth exits 1 before its netlist
  could be mistaken for fresh, so the wipe is symmetry here, not
  load-bearing.
- **VHDL rides nothing.** yosys is a Verilog synthesis engine; the
  blueprint's `VHDL` entries are the loud not-carried face (exit 2),
  the same door verilator closes.

## The vivado drives

Two target entries (`vivado`, `vivado-build`) share one drive on the
richest probed process in this crate. The shapes the probes wrote down:

### The launch recipe

Every vivado-family spawn needs the recipe env, and the drive sets all
three legs on every spawn itself (`Command::env`, scoped to the spawn --
a process-wide `LD_LIBRARY_PATH` poisons other tools' backends, probed):
the ghdl LLVM backend fails on Vivado's bundled libstdc++.

- **`LC_ALL`** -- the launcher aborts without a UTF-8 locale it can name.
  The environment's own `LC_ALL` wins; the probed `en_US.UTF-8` default
  rides otherwise.
- **`LOCPATH`** -- where that locale lives when it is not
  system-installed. The environment's own wins; `$HOME/.local/locale`
  (the no-root `localedef` recipe) otherwise. Omitted when neither
  exists.
- **`LD_LIBRARY_PATH`** -- the SuSE ncurses5 leg the main wrapper's full
  init needs. The environment's own wins; the leg derived from the
  resolved install (`<prefix>/lib/lnx64.o/SuSE`) otherwise. Omitted when
  neither exists.

The binaries resolve from `VIVADO_BIN` (the install path, e.g.
`/opt/2025.2.1/Vivado/bin/vivado`); the stage binaries (xvhdl, xvlog,
xelab, xsim) ride beside it as siblings of the resolved path, and a bare
PATH name falls back to the PATH lookup for the whole family. Every
family binary answers `-version` (single dash) with exit 0 -- the
spawnability probe.

### The compile

- **Both languages ride.** vivado is the one carried tool that speaks
  both: `VHDL` entries compile through xvhdl, `VLOG` and `SYSV` entries
  through xvlog, a custom fileset is the loud not-carried face (exit 2).
- **xvhdl is order-sensitive within a library**
  (`ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'`,
  exit 1, probed live on 2025.2.1), so the VHDL files compile ONE
  INVOCATION PER FILE in blueprint order through the deferral loop (the
  ghdl-precedent machinery, borrowed a third time): a failing file
  defers to a retry after the others; a pass that makes no progress
  stops the loop with every residual log relayed.
- **xvlog is order-free** (a consumer-before-provider pair compiled
  clean, probed live), so the Verilog files compile in ONE invocation
  into `work`.
- **No std flag is pinned.** The tool default compiles GRLIB-scale
  designs green; `--2008` breaks them (the `10-3083` homograph walls,
  probed live). The default is the design, not an
  oversight.
- **The compile gates on the exit and is never judged.** Every probed
  compile failure exits 1, and the faces are UPPERCASE `ERROR:` on
  STDOUT -- stderr is empty on every stage-binary drive, error or clean
  (probed) -- deliberately outside the vocabulary below: case-exactness
  keeps the compile faces out of the judge, and the honest exit makes
  them unnecessary there.

### The library column (the §25 multi-library face)

The drive consumes the blueprint's library column on BOTH faces, probed
live on Vivado 2025.2.1 (2026-11-07) -- the same §25 face the ghdl and
nvc drives carry, on vivado's own one-state-directory resolution:

- **Every xvhdl ride compiles into its own blueprint library**
  (`xvhdl --work=<entry.library> <file>` -- the §25 ruling: the library
  name is the project's declared name, orbit's planner is its source of
  truth, and orbit defaults a project's library to the project's NAME,
  not to any tool's `work`). Vivado's flow keeps ONE state directory:
  every named library lands as `xsim.dir/<library>/` inside the working
  directory's one `xsim.dir/` (probed: `--work=grlib` and `-work work`
  land as siblings there), so libraries compile in ANY blueprint order
  and the deferral loop (unchanged, per-file) defers a consumer across
  the library boundary exactly as within one -- the drive's
  two-library blueprint runs `3 VHDL file(s) compiled in 3 pass(es)`,
  probed through the drive.
- **The state wipe covers the multi-library face for free.** The drive
  wipes the whole `xsim.dir/` before every drive, and that one
  directory holds EVERY named library beside the snapshots -- so a
  library a previous drive analyzed but this blueprint no longer names
  dies with it, and no stale unit stays resolvable (the same stance as
  the ghdl and nvc scans, here from vivado's own nested layout: no
  per-library directories sit beside the state directory for a scan to
  miss).
- **xvlog carries no library column.** Verilog speaks no library
  clauses: every `VLOG`/`SYSV` file lands in `work` (`xvlog -work
  work`), whatever its row declares, and the generated script's
  `read_verilog` lines stay plain. Documented, not consumed.
- **xelab makes every carried library visible and names the top's own**
  (`xelab -L <lib> ... <the top's library>.<top> -s <top>`): one `-L`
  per distinct carried VHDL library plus `work` and the resolved top
  library -- each `-L` makes a library visible for binding (probed: the
  cross-library reference in a `work` testbench resolves through
  `xelab -L grlib -L work`; a `-L` naming a library that holds nothing
  is accepted silently, exit 0 -- over-naming costs nothing), and the
  design-unit prefix must be the top's OWN library (`work.<top>` against
  a top compiled into a named library answers
  `ERROR: [XSIM 43-3225] Cannot find design unit ...`, exit 1). The
  prefix resolves loudest voice first: `$ORBIT_PROJECT_LIBRARY` (orbit's
  planner sets it on every spawned target to the working project's
  library; the designated testbench is a working-project file); then
  the blueprint's single carried VHDL library (direct invocation
  without orbit); then vivado's default `work` -- a guess that cannot
  pass silently, because the wiped `xsim.dir/` cannot hold a stale
  unit: a wrong guess dies loudly on xelab's refusal.
- **The generated script reads each file out of its own library**
  (`read_vhdl -library <entry.library> <file>` -- the compile's mirror:
  the file xvhdl analyzed into `--work=<lib>` is the file the script
  reads back out of `<lib>`; a bare `read_vhdl` would land it in `work`
  where its consumers cannot see it. `-library` on `read_vhdl` is the
  GRLIB flow's own shape, probed in hdlm's vivado arc and driven live
  through the gated tests). `synth_design -top` is unchanged.
- **The argv shape changed with the multi-library face** (2026-11-07):
  a single-`work` blueprint -- every legacy shape -- now rides the SAME
  library explicitly: `xvhdl -work work f.vhd` became
  `xvhdl --work=work f.vhd`, the script's `read_vhdl f.vhd` became
  `read_vhdl -library work f.vhd`, and the xelab argv is UNCHANGED
  (`-L work work.<top> -s <top>`: the ladder's fallback IS vivado's
  default, so the old pin is the new argv there). Nothing about the
  judged faces moved with it; the probed sequences below carry the new
  argv verbatim, and the single-`work` live tests drive the new shape
  end to end.

### The build face (`vivado-build`)

- **`--part` is required BEFORE any drive** (exit 2 at the CLI naming
  the flag): the probed alternative is a ~4s synth-then-fail inside the
  tool. The sim face consumes no part -- the flag on any other face is a
  loud parse error, never a dropped argument.
- **The synthesis script is GENERATED** per top, written to
  `orbit-synth-<top>.tcl`: one `read_vhdl -library <entry.library>` /
  `read_verilog` per file (the library face is the library column's,
  above; the batch `read_*` commands take one file per command, and
  read order is irrelevant -- probed on 2025.2.1), then `synth_design
  -top <top> -part <part> -mode out_of_context`, then the three netlist
  writers into `netlists/` (`write_checkpoint -force` .dcp,
  `write_verilog -force` .v, `write_edif -force` .edf).
- **The catch-guard is the probed trap.** The whole body sits inside ONE
  guard that exits 1 -- and the guard tests `catch`'s RETURN CODE, never
  the caught string: Tcl's `catch {script} var` writes the script's
  NORMAL RETURN VALUE into `var`, and the netlist writers return the
  written filename, so an emptiness guard FALSE-FAILS A CLEAN SYNTH
  (probed live: exit 1 with all three netlists written). The guard also
  makes the batch fail-fast -- unguarded batch mode continues past a
  failed `synth_design` (probed on 2025.2.1), and a guardless catch
  would write nothing after the failure.
- **The batch exit is honest.** Exit 0 clean with the payload landed,
  exit 1 fail-fast through the guard. The drive never judges the batch
  log.

### The sim face (`vivado`)

- **xelab names every carried library and the top's own**
  (`xelab -L <lib> ... <the top's library>.<top> -s <top>`, the library
  column's face above); the snapshot lands as `xsim.dir/<top>` and the
  elaboration validates the top (an unknown top answers
  `ERROR: [XSIM 43-3225] Cannot find design unit ...`, exit 1,
  probed).
- **xsim runs each snapshot once** (`xsim <top> -R`), and THE RUN EXIT
  IS A LIE: xsim exits 0 on a violated `assert ... severity failure`
  AND on a violated `severity error` (both probed live -- the face
  prints, the simulation continues or stops, and the process exits 0).
  The log is the only truth, and the judge is the load-bearing gate --
  the only drive in this crate where the exit cannot convict a probed
  failure. The exit stays the crash-door witness: a missing snapshot
  exits 1 with an uppercase face the vocabulary does not carry.
- **The vocabulary is line-anchored, the only PROBED one in this
  crate.** The faces are `Fatal:` / `Error:` / `Failure:` at LINE START,
  case-exact: a case drift (`FATAL:`) and a marker mid-line (a
  testbench's own report note, an echoed source line) never judge.
  `Warning:` and `Note:` lines never judge.

## The quartus drives

Two target entries (`quartus`, `quartus-build`) share one drive -- the
DOCUMENTED-ONLY drive of this crate: quartus is not installed on the
probing box, so nothing here is probed, every CLI face is the documented
hypothesis, and every hypothesis carries its first-contact rule in the
honest limits. On the first quartus box, the owner runs the gated shape
tests (`cargo test -- --ignored` with `QUARTUS_BIN` set) and rewrites
the vocabulary and these rules from the live logs.

### The resolution

The binaries resolve from `QUARTUS_BIN` (the quartus_map path), the
bundled Questa deriving as the `questa_fse/bin` SIBLING of the quartus
install (`<install>/quartus/bin/quartus_map` derives
`<install>/quartus/questa_fse/bin/{vlib,vmap,vcom,vsim}` -- the
documented derive, UNPROBED); a bare PATH name falls back to the PATH
lookup for the whole family.

### The build face (`quartus-build`)

- **`--part` is required BEFORE any drive** (exit 2 at the CLI naming
  the flag): the DEVICE assignment names the fabric the map targets.
  The sim face consumes no part -- the flag on any other face is a loud
  parse error, never a dropped argument.
- **The project settings are GENERATED per top**, written to
  `<top>.qsf`: `set_global_assignment -name DEVICE <part>`,
  `set_global_assignment -name TOP_LEVEL_ENTITY <top>`, and one
  `VHDL_FILE` / `VERILOG_FILE` line per blueprint file (both languages
  ride the build face; a custom fileset is the loud not-carried face).
  HYPOTHESIS: that DEVICE + TOP_LEVEL_ENTITY + the file list suffice for
  a minimal `.qsf` is the first-contact question.
- **The batch argv is the documented one**:
  `quartus_map --read_settings_files=off --write_settings_files=off
  <top> -c <top>` (project = top, revision = top). HYPOTHESIS: the
  read/write-settings flags' exact semantics are unprobed.
- **The exit is hypothesized honest** (nonzero on failure, UNPROBED):
  the build face gates on the exit and never judges the log.

### The sim face (`quartus`)

- **The VHDL fileset only.** The documented recipe is vcom's alone:
  `VLOG` / `SYSV` entries are the loud not-carried face here (exit 2),
  though they ride the build face's `VERILOG_FILE` lines.
- **The library opens documented**: `vlib work`, then `vmap work work`
  (the ModelSim workflow's opening pair, UNPROBED); the drive wipes
  `work/` and `modelsim.ini` before every sim drive -- the
  correctness-before-incrementality stance, DOCUMENTED-ONLY.
- **vcom compiles in blueprint order through the deferral loop** (the
  ghdl-precedent machinery, borrowed a fourth time): the
  order-sensitivity hypothesis of the stateful ModelSim library, so a
  failing file defers to a retry after the others. The standard is
  pinned (`vcom -2008 -work work`, vcom's spelling of the crate-wide
  VHDL-2008 pin; `-93` / `-2002` are the other spellings and cannot be
  selected from the argv yet).
- **vsim runs each top once**:
  `vsim -c -do "run -all; quit" -L work work.<top>`, the `-L work` pin
  before the top -- the documented order, UNPROBED. The extras ride
  this argv verbatim.
- **The gate is double-keyed on hypotheses.** The exit honesty (vsim
  honest) and the vocabulary (line-start `Fatal:` / `Error:` /
  `Failure:`, the ModelSim-family rendering riding vivado's probed
  anchor) are BOTH unprobed -- so neither witness is trusted alone and
  either convicts, the same as everywhere in this crate. The
  vocabulary's negative pins are hypothesized from vivado's probed
  ones: a case drift (`FATAL:`) and a marker mid-line never judge.

## The cocotb drive

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

## The probed facts

Every pattern in a vocabulary and every drive fact below was probed live
on this box, 2026-11-07, on assert- and report-driven testbenches, both
bare and through the drive. Each row carries the argv that produced the
face.

### ghdl 6.0.0

| face / fact | argv that wrote it | what ghdl did |
|------|-----|------|
| `(assertion failure)`, `./<top>:error: assertion failed`, `./<top>:error: simulation failed` | `ghdl -r --std=08 tb_fail` (same through the drive, which adds `--assert-level=error` after the top) | a violated `assert ... severity failure` stops the run, exit 1 |
| `(assertion error)` | bare `ghdl -r --std=08 tb_err` | a violated `assert ... severity error`: prints the face and **continues, exit 0** -- bare ghdl's hole. The drive pins `--assert-level=error`, which stops the run (exit 1), and the judge reads the face either way |
| `(assertion warning)`, `(assertion note)` | `ghdl -r --std=08 tb_warn` | the gentler severities on a continuing run -- continuations, **never judged** |
| `(report failure)`, `./<top>:error: report failed` | `report ... severity failure;` statement, `ghdl -r --std=08 tb_rep_fail` | a report statement carrying `severity failure` stops the run, exit 1 -- caught by `:error:` |
| `(report error)` | `report ... severity error;` statement: bare `ghdl -r --std=08 tb_rep_err` | bare ghdl prints and continues, exit 0, **no judged face** (bare-ghdl-only, see limits). Through the drive, `--assert-level=error` stops the run with the `./<top>:error: report failed` face, which judges |
| `ghdl:error: unknown command option '--assert-level=error'` | `ghdl -r --std=08 --assert-level=error tb` | the assert level is a SIMULATION option: after the toplevel unit only |
| `ghdl:error: too many unit names for command '-e'` | `ghdl -e --std=08 gate top tb` | `-e` takes ONE unit name. Two names are misread as primary + secondary (`error: cannot find entity or configuration gate`) |
| `ghdl:error: unknown command '--std=08'` | `ghdl --std=08 -a f.vhd` | ghdl's argv is command-first: `ghdl -a --std=08 f.vhd` |
| `error: architecture "rtl" of "top" is obsoleted by entity "gate"` | the drive's second run in one output directory, consumer-first blueprint | the stale work library clock: re-analysis obsoletes dependents, `ghdl -e` refuses the unchanged design -- the wipe exists for this |
| `simulation stopped @0ms` | `ghdl -r --std=08 tb_stop` on a testbench calling `std.env.stop` | a legitimate stop exits 0 -- clean, not a crash-door finding |
| `cannot find resource library "grlib"` (+ `unit "grlib_pkg" not found in library "grlib"`, `entity "tb" was not analysed`, `ghdl:error: compilation error`) | `ghdl -a --std=08 --work=work tb.vhd` with no `grlib-obj08.cf` in the cwd | the absent-sibling-library face, exit 1 -- four markers on ONE failed spawn; the retry resolves once the provider's row writes `grlib-obj08.cf` (the cross-library deferral face: the drive's multi-library blueprint runs `3 file(s) analyzed in 3 pass(es)`, probed through the drive) |
| exit 0 end to end, `(report note): SIM PASSED` relayed, two libraries | per-file `ghdl -a --std=08 --work=work tb.vhd` / `--work=work gate.vhd` / `--work=grlib pkg.vhd` (consumers FIRST); then `ghdl -e --std=08 --work=work tb`; then `ghdl -r --std=08 --work=work tb` (same through the drive) | **the multi-library face**: one `<library>-obj08.cf` per `--work` library in the cwd; the consumers' `library grlib;` resolves against the sibling `grlib-obj08.cf` AUTOMATICALLY -- no `-P`/`-L` flag exists or is needed; elab/run pin the top's library and resolve the other library through the sibling file |
| `error: cannot find entity or configuration tb` (+ `ghdl:error: compilation error`) | `ghdl -e --std=08 tb` with tb analyzed into `--work=counter` -- bare, and pinned to the wrong library (`--work=work`) alike | **the bare-top WORK-only search**, exit 1: even with the library's sibling file in the cwd, a bare (or wrongly pinned) top is searched in the named library alone -- why the drive pins `--work=<the top's library>` on every elaborate and run |
| exit 0, `(report note): SIM PASSED` | `ghdl -e --std=08 --work=counter tb; ghdl -r --std=08 --work=counter tb` (tb analyzed into `--work=counter`) | the pinned-library elab/run: the top resolves in its OWN library, what it references through the sibling file |
| `--work=MyLib` writes `mylib-obj08.cf` | `ghdl -a --std=08 --work=MyLib pkg.vhd` | ghdl LOWERCASES the library name in the state file's name, and `library MyLib;` clauses still resolve against it -- why the drive's wipe keys on the `-obj08.cf` suffix alone |

Case-exactness is part of the contract: ghdl spells its failures in
lowercase, so `:ERROR:`, `(ASSERTION FAILURE)`, `(ASSERTION ERROR)`, and
`FATAL:` are not its faces and never judge a log -- an uppercase drift of a
real failure log judges clean, and so does a clean log that merely mentions
"ERROR" mid-text (a filename, an echoed banner). That is deliberate: the
retired gate this replaces counted `error):`/`ERROR` substrings, matched
nothing ghdl writes, and false-failed clean logs.

### nvc 1.23.0

| face / fact | argv that wrote it | what nvc did |
|------|-----|------|
| `** Failure: 0ms+0: the design is wrong` (then `   Process :tb_fail:_p0 at tb_fail.vhd:5`) | `nvc --std=2008 -a tb_fail.vhd; nvc --std=2008 -e tb_fail; nvc --std=2008 -r tb_fail` (same through the drive) | a violated `assert ... severity failure` stops the run, exit 1 -- honest |
| `** Error: 0ms+0: check failed` | `nvc --std=2008 -r tb_err` on a `severity error` assert | nvc prints the face, **continues** (`** Note: 0ms+0: simulation continued` follows), and **still exits 1** -- unlike bare ghdl's exit-0 hole, nvc's exit convicts on its own; the face is the second witness |
| `** Warning: 0ms+0: ...`, `** Note: 0ms+0: ...` | `nvc --std=2008 -r tb_warn` | the gentler severities on a continuing run that **exits 0** -- continuations, **never judged** |
| `** Failure: 0ms+0: stop now` / `** Error: 0ms+0: flagged` | `report ... severity failure;` / `report ... severity error;` statements, `nvc --std=2008 -r ...` | report statements ride the same faces: failure stops (exit 1), error continues and **exits 1** -- the bare-ghdl report hole does not exist on nvc |
| `** Error: unexpected end while parsing port clause, ...` above a `> broken.vhd:3` caret block | `nvc --std=2008 -a broken.vhd` | the analysis-error face (same `** Error:` marker as runtime), exit 1; the analyze step gates on the exit and is never judged |
| `** Error: design unit GATE not found in library WORK` | `nvc --std=2008 -a top.vhd` with `gate.vhd` unanalyzed | the order-dependence face: the consumer cannot analyze before its provider, exit 1 -- and the failed analysis does not poison the library: provider then consumer both analyze after (the deferral loop's shape, probed live) |
| `** Fatal: cannot find unit WORK.TB_NOSUCH in library WORK` | `nvc --std=2008 -e tb_nosuch` | the elaboration refusal when the top names no analyzed unit, exit 1 |
| `** Fatal: unrecognised analysis option --std=2008` | `nvc -a --std=2008 f.vhd` | nvc's argv is option-FIRST: the global `--std` leads and the command follows -- the opposite of ghdl's command-first shape |
| exit 0 end to end, `** Note: 0ms+0: SIM PASSED` relayed, two libraries | per-file `nvc --std=2008 -L . --work=grlib -a pkg.vhd` / `--work=work -a gate.vhd` / `--work=work -a tb.vhd` (consumers FIRST, in any order); then `nvc --std=2008 -L . --work=work -e tb`; then `nvc --std=2008 -L . --work=work -r tb` | **the multi-library face**: one directory per `--work` library in the cwd; the consumers' `library grlib;` resolves at analyze against `grlib/` because `-L .` puts the cwd on the search path; elab/run pin the top's library and resolve the other library's units through the search path |
| `** Error: library GRLIB not found` (+ caret block, `= Note: library search path contains: ., ...`) then `** Fatal: required library GRLIB not found` (+ the `add additional directories ... -L` note) | `nvc --std=2008 --work=work -a gate.vhd` with no `-L` on a `library grlib;` consumer; `nvc --std=2008 -e tb` with `grlib/` in the cwd but unsearched | **the missing-search-path face**, two markers on ONE failed spawn, exit 1: the reader's `** Error:` and the process-level `** Fatal:`; the note lists the search path it tried and names the `-L` fix -- why every drive spawn carries `-L .` |
| `** Error: library GRLIB not found` on the consumer-first analyze, then provider-then-consumer all exit 0 | blueprint order tb, gate (both `library grlib;`), pkg (into `--work=grlib`): pass 1 fails both consumers on the ABSENT `grlib/` directory while pkg creates it; passes 2-3 retire gate then tb | **the cross-library deferral face**: a library directory that does not exist yet reads as the library-not-found face, and the retry resolves -- the deferral loop needs no change to defer across libraries (the drive's multi-library blueprint runs `3 file(s) analyzed in 3 pass(es)`, probed through the drive) |
| `** Fatal: cannot find unit WORK.TB in library WORK` | `nvc --std=2008 -L . -e tb` with tb analyzed into `--work=grlib` | **the bare-top WORK-only search**, exit 1: even with the library's directory on the search path, a bare top is searched in WORK alone -- why the drive pins `--work=<the top's library>` on every elaborate and run |
| exit 0, `** Note: 0ms+0: SIM PASSED` | `nvc --std=2008 -L . --work=grlib -e tb; nvc --std=2008 -L . --work=grlib -r tb` (tb analyzed into grlib, nothing in work) | the pinned-library elab/run: the top resolves in its OWN library, what it references through the search path |
| `<lib>/_NVC_LIB` written even on a failed analyze | `nvc --std=2008 --work=freshlib -a broken.vhd` (syntax error, exit 1); `nvc --std=2008 --work=optfail -a --bogus broken.vhd` (option error, exit 1) | every directory nvc makes a work library carries the `_NVC_LIB` marker file (nvc's own recognition rule refuses a directory without it) -- the drive's wipe keys on it |
| re-analysis, elaboration and run all exit 0 | a second drive in one output directory: provider then consumer re-analyzed, `-e`, `-r` | nvc tolerates stale-library re-analysis without ghdl's obsoleted-design refusal -- against nvc itself the wipe is symmetry, not load-bearing; on a multi-library blueprint it IS load-bearing (a library this blueprint no longer analyzes must not stay resolvable) |

nvc's markers are case-exact capitals: a lowercase drift (`** error:`,
`** failure:`) is not one of its faces and never judges a log. And as with
ghdl, a testbench's own report note that embeds an nvc face shape
(`** Error: retrying`) convicts the run -- keep report notes free of
nvc-face shapes.

### iverilog 12.0 (stable) / vvp 12.0

| face / fact | argv that wrote it | what iverilog/vvp did |
|------|-----|------|
| `FATAL: tb_fatal.v:4: the design is wrong` (+ `       Time: 1  Scope: tb_fatal` beneath, on stdout) | `iverilog -g2012 -o f.vvp tb_fatal.v && vvp f.vvp` (same through the drive) | a `$fatal(1, "...")` call stops the run, exit 1 -- honest |
| `ERROR: tb_error.v:4: check failed` (+ Time/Scope, on stdout) | `iverilog -g2012 -o e.vvp tb_error.v && vvp e.vvp` | a `$error("...")` call: vvp prints the face and **continues** -- the testbench's own `$display` output and `tb_error.v:6: $finish called at 1 (1s)` follow -- and **exits 0**: the bare-ghdl hole again, the judge load-bearing |
| `WARNING: tb_warn.v:4: check flagged` (+ Time/Scope) | `vvp w.vvp` on a `$warning` call | the gentler severity on a continuing run that **exits 0** -- never judged |
| `tb_stop.v:5: $stop called at 1 (1s)`, `** VVP Stop(0) **`, `> ** Continue **` | `vvp s.vvp` on a `$stop` call | a `$stop` is an interactive stop, exit 0 -- a legitimate stop, clean, not a crash-door finding (a verilator model's `$stop` aborts instead: below) |
| `broken.v:2: syntax error` then `I give up.` (stderr) | `iverilog -o snap_broken.vvp broken.v` | the parse-error face, **exit 2**; the compile gates on the exit and is never judged, the face is recorded |
| `error: Unable to find the root module "nosuch_top" in the Verilog source.` then `1 error(s) during elaboration.` | `iverilog -g2012 -s nosuch_top -o x.vvp tb.v` | the unknown-`-s`-root refusal, **exit 1**: top names are validated at compile time, so the build face carries the check for free |
| consumer-first compile exits 0 | `iverilog -g2012 -o order.vvp top.v gate.v tb.v` (consumer before provider) | **order-free compile** -- one invocation, no deferral loop; repeated `-s` flags are legal (exit 0) and multiple roots share the one snapshot and the one `vvp` run |
| `$fatal` accepted at compile, exit 0 | `iverilog -o nog2012.vvp tb_fatal.v` (no `-g`) | iverilog 12.0's default generation also accepts `$fatal` -- the `-g2012` pin keeps the judged faces pinned, not the acceptance |

vvp's runtime failure faces are case-exact capitals on stdout: a
lowercase drift (`fatal:`, `error:`) is not one of its faces and never
judges a log. As with the VHDL tools, a testbench's own `$display` that
embeds a face shape (`ERROR: retrying`) convicts the run -- keep
`$display` text free of face shapes.

### verilator 5.032

| face / fact | argv that wrote it | what verilator / the model did |
|------|-----|------|
| `%Error: broken.v:2:3: syntax error, unexpected input, expecting ';'` (+ caret block, then `%Error: Exiting due to 1 error(s)`) | `verilator --lint-only broken.v` (same through the drive, which pins `--timing`) | the lint-error face, exit 1 -- the lint gates on the exit and is never judged, the face is recorded |
| `%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or --no-timing to specify how delays should be handled` | `verilator --lint-only tb.sv` (no `--timing`) on a delay-carrying file | the coded lint face, exit 1 -- why the drive pins `--timing` on every argv; the bare `%Error` vocabulary face subsumes the coded form |
| `%Error: Specified --top-module 'nosuch' was not found in design.` | `verilator --lint-only --timing --top-module nosuch tb.sv ...` | the refusal of an unknown top, exit 1 -- on the lint it fires only when the unknown name is the LAST `--top-module` (the honored one; a non-last one lints clean, exit 0), and every top is validated by its own per-top build on both faces |
| `%Warning-WIDTHTRUNC: ...` still exit 1 | `verilator --lint-only --timing -Wall widthy.sv` (and even under `-Wwarn-WIDTHTRUNC`) | an *enabled warning* fails the lint -- there is no exit-0 warning face in lint land; a warning face never needs judging |
| `%Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is wrong`, then `%Error: tb_fatal.sv:8: Verilog $stop`, then `Aborting...` (stdout) | `verilator --binary --build --timing --top-module tb_fatal ... && ./obj_dir/Vtb_fatal` (same through the drive) | a model `$fatal(1, "...")`: the faces land, the model dies by **SIGABRT** -- no exit code at all (shell: `Aborted`; the drive records -1) -- the exit convicts, the face is the second witness |
| `%Error: tb_err.sv:8: Assertion failed in tb_err: check failed` (+ `%Error: ... Verilog $stop`, `Aborting...`) | `./obj_dir/Vtb_err` on a `$error("...")` call | a model `$error`: the same faces, the same SIGABRT -- unlike vvp's continue-and-exit-0 `$error`, the built model has no interactive continue |
| `%Error: tb_stop.sv:9: Verilog $stop` (+ `Aborting...`) | `./obj_dir/Vtb_stop` on a plain `$stop` | even a `$stop` aborts the model (SIGABRT) -- a finding on both witnesses; contrast vvp's interactive `$stop` (exit 0, clean) |
| `[1000] %Warning: tb_vwarn.sv:8: tb_vwarn: check flagged` | `./obj_dir/Vtb_vwarn` on a `$warning` call | the gentler severity in the model, continuing run, **exit 0** -- never judged |
| `- S i m u l a t i o n   R e p o r t: Verilator 5.032 ...`, `- Verilator: $finish at 1ns ...` | `./obj_dir/Vtb` on a clean run | the clean model's stdout banner -- continuations, never judged |
| consumer-first lint and build exit 0; repeated and differing `--top-module` flags exit 0; `obj_dir/Vtb` and `obj_dir/Vtb_err` coexist | `verilator --lint-only --timing --top-module tb --top-module tb_err tb.sv tb_err.sv top.sv gate.sv`; `verilator --binary --build --timing --top-module tb top.sv gate.sv tb.sv` | **order-free lint and build** -- one lint, per-top builds into the shared `obj_dir`, no deferral loop; multiple roots share one lint |

verilator's markers are case-exact capitals: a lowercase drift
(`%error:`, `%fatal:`) is not one of its faces and never judges a log.
And as with the other tools, a testbench's own `$display` that embeds a
verilator face shape (`%Error: retrying`) convicts the run -- keep
`$display` text free of face shapes.

### yosys 0.52

| face / fact | argv that wrote it | what yosys did |
|------|-----|------|
| `broken.v:3: ERROR: syntax error, unexpected TOK_ENDMODULE` (stderr) | `yosys -p "read_verilog broken.v; synth"` (same through the drive) | the frontend's syntax face, exit 1 -- the `ERROR:` marker rides mid-line behind the source location |
| `ERROR: Module \`nosuch' not found!` (stderr) | `yosys -p "read_verilog top.v gate.v; synth -top nosuch; write_verilog netlists/nosuch.v"` | the script's refusal of an unknown top, exit 1 -- the `-p` script aborts and no netlist is written |
| `ERROR: No such command: frobnicate (type 'help' for a command overview)` (stderr) | `yosys -p "read_verilog gate.v; frobnicate"` | the unknown-`-p`-command face, exit 1 |
| `ERROR: Can't open output file \`netlists-missing/x.v' for writing: No such file or directory` (stderr) | `yosys -p "...; write_verilog netlists-missing/x.v"` | the failed-write face, exit 1 -- a late failure still convicts |
| `ERROR: Module \`\gate' referenced in module `\top' in cell `\u_gate' is not part of the design.` (stderr) | `yosys -p "read_verilog top.v; synth -top top; ..."` with `gate.v` never read | the missing-provider face, exit 1 -- `synth`'s hierarchy check catches an instantiation whose provider was never read; no silent blackbox degradation |
| log on stdout, exit 0, `netlists/top.v` written | `yosys -p "read_verilog gate.v; synth; write_verilog netlists/top.v"` | the clean synth: the whole synthesis log rides stdout, the netlist lands |
| consumer-first synth exits 0 | `yosys -p "read_verilog top.v gate.v; synth -top top; write_verilog ..."` (consumer before provider) | **order-free** -- yosys reads all files into one design and elaborates at `synth` time; no deferral loop |

yosys's marker is case-exact uppercase (`ERROR:`), line-start on the
script faces and mid-line behind the source location on the frontend's
syntax face; a lowercase drift (`error:`) is not one of its faces and
never judges a log. The log rides stdout and the failure faces stderr;
the drive joins the streams, so the face reads either way.

### vivado 2025.2.1

| face / fact | argv that wrote it | what the tool did |
|------|-----|------|
| `Failure: the design is wrong` (+ the `Time: ... File: ...` line, on stdout) | `xvhdl --work=work tb_fatal.vhd && xelab -L work work.tb_fatal -s sim_fail && xsim sim_fail -R` (same through the drive) | a violated `assert ... severity failure` stops the check -- and **xsim EXITS 0**: the run exit is a lie, the line-start face is the only witness |
| `Error: check failed` (+ the continuation `Note:` line, on stdout) | `xsim sim_err -R` on a `severity error` assert | the face prints, the simulation **continues** (`Note: continued past the error` follows), and **xsim still exits 0** -- the same lie; the face convicts |
| `Note: sim done`, the kernel `INFO:` lines | `xsim sim_clean -R` on a passing testbench | the clean run exits 0 -- continuations, **never judged** |
| `ERROR: [VRFC 10-4982] syntax error near 'end' [...:9]` (+ `ERROR: [VRFC 10-2989]`, then the `10-8704` ignored line, on **stdout**) | `xvhdl --work=work broken.vhd` (same through the drive, which joins the streams) | the compile-error face, **exit 1**; stderr is EMPTY on every stage-binary drive, error or clean; the compile gates on the exit and is never judged -- the uppercase face is deliberately outside the mixed-case vocabulary |
| `ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'` | `xvhdl --work=work tb_clean.vhd top.vhd gate.vhd` (consumer before provider) | **order-sensitive within a library**, exit 1 -- the deferral loop's reason to exist here; even the tb-gate-top order fails (`gate` before `top` is required) |
| `INFO: [VRFC 10-163] Analyzing VHDL file ".../pkg.vhd" into library grlib`, exit 0, and `xsim.dir/grlib/` appears in the cwd | `xvhdl --work=grlib pkg.vhd`, beside a `-work work` compile: `xsim.dir/` then holds `grlib/` and `work/` as siblings | **the library column's compile face**: `--work=<lib>` is accepted with the equals spelling, and every named library lands INSIDE the one `xsim.dir/` of the working directory -- so libraries compile in any order and the wholesale state wipe covers them all at once |
| `INFO: [VRFC 10-2263] Analyzing Verilog file ...` , exit 0 | `xvlog vtop.v vgate.v` (consumer before provider) | **order-free** -- xvlog compiles in one invocation, no deferral loop; `-work work` pins the library (Verilog speaks no library clauses: the blueprint's library column has no xvlog face) |
| `Compiling package grlib.grlib_pkg` ... `Built simulation snapshot tb`, exit 0; then `Note: SIM PASSED`, exit 0 | `xelab -L grlib -L work work.tb -s tb && xsim tb -R` (the package compiled into `--work=grlib`, the design and tb into `work`) | **the library column's bind face**: one `-L` per carried library makes each visible for binding and the cross-library reference resolves; the top rides its own library's prefix (`work.tb`) |
| exit 0, `Built simulation snapshot tb2` | `xelab -L grlib -L work grlib.tb -s tb` with EVERYTHING compiled into `--work=grlib` (`xsim.dir/` holds no `work/` at all), and `xelab -L grlib -L nosuchlib grlib.tb -s tb` | the single-library collapse: an explicit non-`work` prefix works, and a `-L` naming an absent library is accepted **silently** -- over-naming costs nothing, under-naming strands the binding |
| `ERROR: [XSIM 43-3225] Cannot find design unit work.tb_nosuch in library work located at xsim.dir/work.` | `xelab -L work work.tb_nosuch -s sim_nosuch` | the elaboration refusal of an unknown top, **exit 1** -- honest, the elaboration gates on the exit |
| `ERROR: Please check the snapshot name ... does not exist` | `xsim nosuch_snap -R` | the missing-snapshot face, **exit 1** -- the crash door; uppercase, outside the vocabulary, the exit convicts |
| `SYNTH FAILED: ERROR: [Common 17-69] Command failed: Vivado Synthesis failed`, **exit 1** | `vivado -mode batch -nojournal -nolog -source synth.tcl` with a syntax-broken `read_vhdl` inside the catch-guard | the fail-fast batch face: the return-code catch-guard stops the script and exits 1 (unguarded batch **continues past a failed synth_design**, probed) |
| **exit 1 with all three netlists written** | the same script whose guard tested `$synth_err` emptiness instead of catch's return code | **the probed Tcl trap**: `catch {script} var` writes the script's NORMAL RETURN VALUE into `var`, and `write_verilog`/`write_checkpoint` return the written filename -- an emptiness guard false-fails a CLEAN synth |
| exit 0, `netlists/top.dcp` + `top.v` + `top.edf` written | `vivado -mode batch -nojournal -nolog -source synth.tcl`, guard on catch's return code, `synth_design -top top -part xc7z010clg400-1 -mode out_of_context` | the clean synth face; the echo of the sourced commands rides stdout, the synth errors ride **stderr**, exit honest |
| exit 0, the netlists written, a TWO-library closure (`read_vhdl -library grlib ./pkg.vhd` + `read_vhdl -library work ./gate.vhd` + `read_vhdl -library work ./tb.vhd` in the sourced script) | the generated script driven through `vivado-build` (`cargo test -- --ignored`, the gated harness) | **the library column's script face**: each VHDL read names the file's own library -- the compile's mirror, the GRLIB flow's own shape (probed in hdlm's vivado arc); `synth_design -top` finds the top across the named libraries unchanged |
| `-version` exits 0 on every family binary | `vivado -version`, `xvhdl -version`, `xvlog -version`, `xelab -version`, `xsim -version` | the identity face (single dash), the drive's spawnability probe |

vivado's run vocabulary is line-start mixed-case and case-exact: an
uppercase drift (`FATAL:`, `ERROR:`) is not one of xsim's run faces and
never judges a log -- and the uppercase `ERROR:` faces of the compile and
elaboration stages are deliberately outside the vocabulary, because those
stages gate on their honest exits. A testbench's own report note that
LEADS its line with a face shape (`Failure: retrying`) convicts the run;
mid-line prose never does. The recipe env (above) rides every spawn, set
by the drive; `.Xil/` and the stage logs (`xvhdl.log`, `xvlog.log`,
`xsim.log`, `xsim.jou`) land in the working directory and are the tools'
own scratch.

### cocotb 2.1.0 (+ ghdl 6.0.0, the backing simulator)

| face / fact | argv that wrote it | what cocotb/the runner did |
|------|-----|------|
| `** test_counter.reads_the_first_count   PASS ...`, then `** TESTS=1 PASS=1 FAIL=0 SKIP=0` | `get_runner("ghdl")`; `build(sources=[counter.vhd], hdl_toplevel="counter", always=True)`; `test(test_module="test_counter", hdl_toplevel="counter")` | the clean pass: the regression table's PASS row and the clean summary, **exit 0** through the generated script |
| `** test_counter_fail.expects_the_wrong_count   FAIL ...`, then `** TESTS=1 PASS=0 FAIL=1 SKIP=0` (+ the `AssertionError: the design is wrong` traceback above) | the same runner call, `test_module="test_counter_fail"` | a failed assert: the row column reads FAIL, the summary counts it -- and **`ghdl -r` EXITS 0**: the finding lands only in results.xml, the generated script's `get_results` verdict is the load-bearing exit (exit 1) |
| `ERROR: Simulation terminated abnormally. Results file ... not found.` | `test_module` naming a module that fails to import (or carries no tests: `RuntimeError: No tests were discovered in any module: ...`) | the sim dies before results.xml is written; `cocotb_tools.check_results.get_results` raises, the generated script relays the message, **exit 1** -- the crash door |
| `Simulation failed: <rc>` (the runner's log), then the script exits with the sim's code | the runner's own `_execute` on a nonzero simulator exit | a SIMULATOR-level failure exits the runner nonzero -- honest; only the failed-TEST exit lies |
| `python -m cocotb.runner` does not exist; `TOPLEVEL`/`SIM` env vars do not exist | the 2.x runner face | the runner is `cocotb_tools.runner.get_runner(...)`, the env vars renamed (`COCOTB_TOPLEVEL`, `COCOTB_TEST_MODULES`, `COCOTB_RESULTS_FILE`), all set by the runner itself |
| `ValueError: Ghdl only supports VHDL. '<file>' cannot be compiled.` | the runner's own `_build_command` on a non-VHDL source | the ghdl backend compiles VHDL only -- the drive's fileset door, probed shut |
| the runner unlinks `results.xml` before each test; `build_dir` defaults to `sim_build` | the runner's source, probed behavior | the stale-results door is closed by the runner; the drive's own `sim_build/` wipe is symmetry |

cocotb's row column is uppercase `FAIL`, space-padded on both sides --
case-exact, and the padding is load-bearing: the clean summary's own
`FAIL=0` counter (and bare `FAIL` prose) shares no shape with ` FAIL `,
so it never judges a clean run. The faces are cocotb's OWN aggregates:
the backing simulator's lowercase ghdl faces belong to the ghdl
vocabulary, never this one.

## Honest limits

- **The filesets are per tool.** The VHDL drives (`ghdl`, `nvc`,
  `cocotb`) carry the `VHDL` fileset only; the Verilog drives
  (`iverilog`, `verilator`, `yosys`) carry `VLOG` and `SYSV` (their
  native filesets) and reject `VHDL` and custom filesets with a loud
  exit-2 ("not yet carried"), never a silent skip. The vivado drives
  and the quartus-build face carry all three (`VHDL` + `VLOG` + `SYSV`)
  and reject only custom filesets -- vivado is the one PROBED tool that
  speaks both languages, quartus-build the documented-only second; the
  quartus sim face carries the `VHDL` fileset only (the documented
  recipe is vcom's alone).
- **Eight tool names, eight vocabularies: ghdl, nvc, iverilog,
  verilator, yosys, vivado, quartus, cocotb.** Any other tool name is a
  loud error at the CLI, never a guess. Seven vocabularies are probed;
  the eighth (quartus) is the documented-only ModelSim-family
  hypothesis (below).
- **quartus is DOCUMENTED-ONLY -- every sentence about it is a
  hypothesis carrying its first-contact rule.** quartus is not
  installed on the probing box, so none of its faces are probed. The
  hypotheses and their first-contact rules, owner's call on the first
  quartus box (run `cargo test -- --ignored` with `QUARTUS_BIN` set --
  the gated shape tests are the harness):
  - **DEVICE-only sufficiency of the generated `.qsf`** (DEVICE +
    TOP_LEVEL_ENTITY + the file lines): if `quartus_map` refuses the
    minimal file, extend `qsf_script` from the tool's own complaint --
    never invent assignments speculatively.
  - **the Questa sibling derive** (`questa_fse/bin` beside the quartus
    install): if the first install names it differently (a versioned
    or differently-named directory), resolve from the actual layout and
    update `questa_bin`.
  - **vsim's exit honesty** (nonzero on a failing sim): probe with a
    severity-failure testbench BEFORE trusting the exit witness alone;
    if vsim lies the way xsim lies, the vocabulary becomes the
    load-bearing gate and the README records it.
  - **the vocabulary itself** (line-start `Fatal:` / `Error:` /
    `Failure:`, ModelSim-family): probe the real failing faces (severity
    failure, severity error, an elaboration refusal) and rewrite
    `QUARTUS_VOCABULARY` from the live logs -- spellings, anchor, and
    negative pins alike.
  - **the vcom default standard** (`-2008` accepted): probe `vcom
    -2008` on a real file; if the bundled Questa rejects the flag
    spelling, the pin moves to the accepted spelling and the honest
    limit is rewritten.
  - **the `-L`-before-top order on the vsim argv** (`vsim -c -do "run
    -all; quit" -L work work.<top>`): probe the documented argv; if
    vsim wants the `-L` pins after the design (or extra `-L` pins per
    dependency library), the argv shape moves and the README records
    it.
  - **the read/write-settings flags** (`--read_settings_files=off
    --write_settings_files=off`): probe whether the generated `.qsf` is
    actually read under them; if not, the invocation moves to the
    variant that reads it.
  - **the build face's state** (quartus_map's compilation database): a
    stale database could make a re-drive incremental; the first probed
    re-drive decides whether the wipe list grows.
- **The library column is load-bearing on the VHDL drives and on
  vivado; carried on the rest.** The nvc drive consumes the blueprint's
  library column verbatim -- every file analyzed into
  `--work=<entry.library>` (the §25 multi-library face, probed live on
  nvc 1.23.0, 2026-11-07), every spawn carrying `-L .`, the wipe taking
  every `_NVC_LIB`-marked directory, and the elaborate/run argv pinning
  the top's library (resolved from `$ORBIT_PROJECT_LIBRARY`, then the
  blueprint's single carried library, then nvc's default `work`). The
  ghdl drive consumes the same column on its own resolution (probed
  live on ghdl 6.0.0, the same day): per-file `--work=`, the
  cwd-sibling `-obj08.cf` files doing the resolution no search flag
  needed, the wipe taking every `<library>-obj08.cf`, and the same
  three-voice ladder on the elaborate/run argv. The vivado drive
  consumes it on BOTH faces (probed live on Vivado 2025.2.1, the same
  day): every xvhdl ride into `--work=<entry.library>`, xelab carrying
  one `-L` per distinct library and naming the top under the same
  three-voice ladder, and the generated script's `read_vhdl -library`
  lines reading each file back out of its own library -- while the
  Verilog legs stay single-library (`xvlog -work work`, `read_verilog`
  plain: Verilog speaks no library clauses, documented). Every OTHER
  drive still maps nothing: iverilog compiles into its one snapshot,
  verilator lints and builds one fileset, and the quartus faces compile
  their filesets as given (`VHDL_FILE` / `VERILOG_FILE` lines, no
  library) -- a multi-library orbit project on quartus stays out of
  scope for v1.
- **The standards are pinned where the tools pin them** (`--std=08` for
  ghdl, `--std=2008` for nvc, `-g2012` for iverilog, `-2008` for the
  quartus sim face's vcom -- vcom's spelling of the same VHDL-2008 pin;
  verilator's pinned flags are `--timing` and the face flags -- its
  language setting stays verilator's default in v1; cocotb's backing
  simulator compiles under ghdl's runner default, no std pin in v1) and
  cannot be overridden from the argv yet; the extras reach the drive's
  last step only (`-r` on the VHDL sim faces, `vvp` on iverilog's, the
  model argv on verilator's, the `yosys -p` synth argv on yosys's, the
  runner's `test_args` on cocotb's; the elaboration / compile / build
  argv on the build faces).
- **The state is run-scoped, not incremental.** Every drive wipes its
  clock first (nvc's: every `_NVC_LIB`-marked directory in the output
  directory -- the whole per-library set, not a named one; ghdl's: every
  `<library>-obj08.cf` file in the output directory -- the same
  whole-per-library set, the name lowercased by ghdl; iverilog's
  snapshot; verilator's
  `obj_dir`; yosys's `netlists/`; vivado's `xsim.dir/` -- the WHOLE
  state directory, which on a multi-library blueprint is the whole
  per-library set for free, because vivado nests every named library
  inside that one directory -- and, on the build face, its
  `netlists/`; the quartus sim face's `work/` +
  `modelsim.ini`, DOCUMENTED-ONLY; cocotb's `sim_build/`). A large
  design pays a full re-analysis per drive; correctness before
  incrementality is the v1 stance. (Probed honestly: ghdl's wipe is
  load-bearing -- a stale library obsoletes its own design -- and with
  libraries in the blueprint it is load-bearing against the drive's own
  resolution too, where a library the blueprint no longer analyzes must
  not stay resolvable; nvc's
  is load-bearing on a multi-library blueprint for the same reason, even
  though
  nvc itself tolerates re-analysis in a stale library; vivado's is
  load-bearing on a multi-library blueprint for the same reason -- the
  wholesale `xsim.dir/` wipe takes every named library with it, so no
  stale unit stays resolvable; iverilog's
  snapshot cannot be run past a
  failed compile, verilator's `--build` regenerates its intermediates, a
  failed yosys synth exits before its netlist could be mistaken for
  fresh, a failed vivado compile or elaboration exits before the sim
  face could run a stale snapshot, and cocotb's runner unlinks the
  results file before each test and always rebuilds -- so those wipes
  are symmetry.)
- **Bare ghdl and bare vvp have holes; bare xsim is worse; the cocotb
  runner lies by omission; the shipped paths catch them.** A
  `report ... severity error` statement slips a bare `ghdl -r` (face
  `(report error)`, exit 0, no judged face): the drive pins
  `--assert-level=error`, which stops the run and writes the
  `./<top>:error: report failed` face. A `$error` call slips a bare vvp
  (face `ERROR: ...`, simulation continued, **exit 0**): there the
  judge's face gate is the only witness, and it is load-bearing. xsim is
  the extreme -- BOTH probed runtime failures exit 0 (a severity failure
  and a severity error) -- so the vivado drive's judge is the
  load-bearing gate on every run, and the line-start vocabulary is the
  only witness on them. The cocotb runner's lie is by omission: `ghdl -r`
  exits 0 on a FAILED test (the finding lands only in results.xml), so
  the generated script reads the results and exits honest -- the
  load-bearing exit there. nvc, verilator, and yosys have no such hole
  probed: nvc's severity-error runs continue the simulation but still
  exit 1, every verilator model failure dies by SIGABRT -- no exit code
  at all -- and every probed yosys failure exits 1, so their exits
  convict.
- **A face shape mid-text judges fail -- except on vivado (and
  quartus, riding vivado's anchor on the documented hypothesis).** The
  substring vocabularies cannot tell your prose from the tool's: a
  testbench's own report note that embeds a ghdl face shape
  (`bus:error: retrying`), an nvc face shape (`** Error: retrying`), a
  vvp face shape (`ERROR: retrying`), a verilator face shape
  (`%Error: retrying`), or a yosys face shape (`ERROR: retrying`)
  convicts the run. vivado is the deliberate probed exception: its
  faces are line-anchored, so only a line LEADING with the marker
  convicts and mid-text prose never does. cocotb's ` FAIL ` row face is
  a substring face with one guard: the space padding keeps the
  summary's own `FAIL=0` counter (and bare `FAIL` prose) from judging,
  but a TEST NAME carrying the space-padded shape (`tb FAIL demo`)
  would still convict a clean run -- keep test names free of face
  shapes. Keep report notes and `$display` text free of face shapes
  anyway.
- **Verilator builds need a C++ toolchain.** `--binary --build` shells
  out to `make` and the system C++ compiler (probed on this box with
  both on PATH); a box without them fails the build step loudly,
  surfaced as an exit-1 build failure with the toolchain's own output
  relayed.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **The tools resolve on PATH, vivado and quartus from their env
  hooks.** The orbit target contract carries no binary-path field: every
  drive but those two pins the bare name; vivado resolves from
  `$VIVADO_BIN` (the install path) with the bare name on PATH as the
  fallback, and its stage binaries ride beside the resolved path (or on
  PATH); quartus resolves from `$QUARTUS_BIN` the same way, its bundled
  Questa deriving as the `questa_fse/bin` sibling (DOCUMENTED-ONLY).
  The cocotb drive pins `python3` and needs cocotb (>= 2.0: the
  `cocotb_tools` runner package -- a 1.x install is a loud usage error
  at the identity probe) and the backing simulator (ghdl) on PATH; the
  runner resolves the simulator itself and names it loudly when absent.
  The vivado recipe env rides the drive's own spawn scoping -- see the
  launch recipe.
- **Blueprint paths with spaces are a v1 limit on the script faces.**
  yosys's `-p` script and vivado's generated synth script carry the file
  paths verbatim, split on whitespace by the tool's own parser: a path
  with a space would split and fail the read loudly.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends. (yosys's synthesis log
  relays only on failure -- on success the drive reports the netlist,
  not kilobytes of synthesis log.)
- **License.** The Cargo.toml declares `MIT OR Apache-2.0`; the LICENSE
  texts land before any `cargo publish` -- never publish without them
  (the maintainer conversation's standing rule). Orbit itself is
  GPL-3 -- irrelevant here: a target is a spawned process, not a derived
  work of orbit.

## Development

```sh
cargo test            # the contract, drive, and argv suites
cargo test -- --ignored   # + the real-spawn tests: the real binary, real
                          # ghdl, nvc, iverilog, verilator, yosys, and
                          # vivado, the real orbit spawn shape,
                          # order-dependent blueprints through the
                          # deferral loop, the nvc two-library blueprint
                          # judged end to end (and a project whose
                          # library is its own name, pinned on the elab
                          # and run argv), judged severity failures and
                          # $fatal/$error calls, twice in one output
                          # directory, the build faces, and the vivado
                          # lying-exit gate. The quartus and cocotb
                          # shapes gate on their toolchains: quartus
                          # needs QUARTUS_BIN (the first-contact
                          # harness for the documented-only faces),
                          # cocotb needs cocotb + ghdl on PATH
```

The contract suite (`tests/contracts.rs`) ports the aquila contract tests
fixture-for-fixture (upstream: `github.com/chaseruskin/aquila`,
`tests/test_contracts.py`) and extends them with the probed faces of the
other tools: the real failing logs judge fail; the clean logs (including
the one mentioning "ERROR" mid-text) judge pass; the uppercase (ghdl) and
lowercase (nvc, vvp, verilator, yosys, cocotb) drifts never judge;
vivado's pins run both ways -- its faces judge at line start only, and a
case drift or a marker mid-line never judges; the severity-error,
`$error`, model-abort, and lying-xsim-exit faces judge fail; cocotb's
`FAIL=0` summary counter and bare `FAIL` prose stay clean while the
space-padded row face judges; the warning/note faces stay clean; and the
registry pins exactly the carried tools. The drive suites pin the
deferral loop (a consumer-before-provider pair must retry), the
no-progress stop, the TSV blueprint parse, the nvc global argv block
(`--std`, `-L .`, `--work=<library>`, pinned verbatim), the top's
library resolution ladder (orbit's env, the single carried library,
nvc's default), the `_NVC_LIB`-scoped wipe, the crash door (a clean log
does not green-light a nonzero exit), the Tcl catch-guard shape, the
generated runner-script shape, and the judge gate on fake severity-error
logs for the VHDL tools.
