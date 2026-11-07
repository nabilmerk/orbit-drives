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
`fileset<TAB>library<TAB>filepath` line per collected file) and
`ORBIT_TB_NAME` set to the designated testbench. Orbit consumes its own
`--` separator and appends the extras straight onto the target's argv, so
the drive passes every argument it does not own through to each `ghdl -r`
verbatim.

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
  (xvhdl per VHDL file with the deferral loop, one order-free xvlog),
  then per top the GENERATED synthesis script through
  `vivado -mode batch -nojournal -nolog`, the three netlists the payload.
  Requires `--part <part>` BEFORE any drive (exit 2 at the CLI); no
  simulation, nothing judged.
- `vivado` -- the vivado sim face: the same compile, then xelab per top
  and one xsim run per snapshot, every run log gated on vivado's probed
  LINE-START failure vocabulary AND the exit. xsim exits 0 on BOTH probed
  runtime failures, so the judge is the load-bearing gate here.

Options:

- `--blueprint <path>` -- the TSV blueprint; defaults to `$ORBIT_BLUEPRINT`.
- `--top <name>` -- a top unit to elaborate (and run: `ghdl`); repeatable.
  With no `--top`, the drive runs `$ORBIT_TB_NAME` (orbit exports it; empty
  in all-mode counts as absent). Exit 2 when neither names a top.
- `--part <part>` -- the FPGA part `vivado-build` synthesizes for (e.g.
  `xc7z010clg400-1`). REQUIRED on `vivado-build` -- the gate fires before
  any drive, exit 2 naming the flag (the probed alternative is a ~4s
  synth-then-fail inside the tool) -- and REJECTED on every other face
  (loud exit-2: the sim face consumes no part, and the other tools do not
  speak one).
- anything else -- a tool extra, appended verbatim to the drive's LAST
  tool argv: each `ghdl -r` / `nvc -r` / `vvp` / `xsim` on the sim faces,
  each `ghdl -e` / `nvc -e` / the `iverilog` compile / the `yosys -p`
  synth / `vivado -mode batch` on the build faces (there is no run to
  carry them, and a dropped argument would be a silent degradation; an
  option the elaboration does not speak is the tool's own loud rejection,
  surfaced as an exit-1 failure).

Exits:

| code | meaning |
|------|---------|
| 0 | clean -- every run judged clean and exited zero, or every top elaborated clean |
| 1 | findings -- a log carried a failure face, a run exited nonzero, or ghdl reported a broken design |
| 2 | usage -- bad argv, no top to run, unreadable blueprint, a fileset the drive does not carry |

## The ghdl drive

Two target entries (`ghdl`, `ghdl-build`) share one drive, split by how
far it goes: the build face stops after step 4, the sim face runs on
through the judge.

1. Read the blueprint; every non-VHDL entry (`VLOG`, `SYSV`, a custom
   fileset) is a loud exit-2, never a silent skip.
2. Wipe the stale work library (`work-obj08.cf`): ghdl's library file is
   its clock, and a second run in orbit's persistent output directory
   would otherwise obsolete its own design out from under `ghdl -e`.
   Correctness before incrementality in v1 -- every drive starts from an
   empty library.
3. Analyze the VHDL files in blueprint order with the **deferral loop**:
   ghdl is order-sensitive, so a failing analyze defers its file to a retry
   after the others; a pass that makes no progress stops the loop with
   every residual stderr surfaced verbatim.
4. Elaborate each top (`ghdl -e --std=08 <top> <extras>` on the build
   face; the extras ride the elab argv there because there is no run to
   carry them). A nonzero elaboration is a finding, exit 1.
5. (`ghdl` only) Run each top (`ghdl -r --std=08 <top>
   --assert-level=error <extra args>`; the assert level is a simulation
   option, legal only after the toplevel unit) and capture the log.
6. (`ghdl` only) Gate the run: a probed failure face in the log OR a
   nonzero ghdl exit is a finding. Either witness convicts.

## The nvc drive

Two target entries (`nvc`, `nvc-build`) share one drive with the same
shapes as ghdl's -- blueprint, deferral-loop analysis, elaboration per
top, judged run -- on nvc's probed process and faces. The differences the
probes wrote down:

- **The argv is option-first, the opposite of ghdl.** `--std=2008` is a
  global option and must LEAD: `nvc --std=2008 -a f.vhd`. After the
  command, nvc answers `** Fatal: unrecognised analysis option
  --std=2008` (exit 1). Run options are legal after the toplevel unit
  (`nvc --std=2008 -r top --stop-time=1ns`, probed clean), so the extras
  ride there.
- **The library state is a `work/` directory**, not ghdl's clock file,
  and the drive wipes it before every analysis -- the same
  correctness-before-incrementality stance as ghdl, though probed
  honestly: nvc tolerates re-analysis in a stale library without ghdl's
  obsoleted-design refusal, so the wipe is symmetry here, not
  load-bearing.
- **No `--assert-level` analogue is pinned.** None is needed: nvc's run
  exits probed honest on every failure face (below), and the judge gates
  the log regardless.
- **One `work` library**, like ghdl's v1: the blueprint's library column
  is carried, not mapped.

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
a process-wide `LD_LIBRARY_PATH` poisons other tools' backends, probed in
the hdlm project):

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
- **xvhdl is order-sensitive within the work library**
  (`ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'`,
  exit 1, probed live on 2025.2.1), so the VHDL files compile ONE
  INVOCATION PER FILE in blueprint order through the deferral loop (the
  ghdl-precedent machinery, borrowed a third time): a failing file
  defers to a retry after the others; a pass that makes no progress
  stops the loop with every residual log relayed.
- **xvlog is order-free** (a consumer-before-provider pair compiled
  clean, probed live), so the Verilog files compile in ONE invocation.
- **No std flag is pinned.** The tool default compiles GRLIB-scale
  designs green; `--2008` breaks them (the `10-3083` homograph walls,
  probed in the hdlm project). The default is the design, not an
  oversight.
- **The compile gates on the exit and is never judged.** Every probed
  compile failure exits 1, and the faces are UPPERCASE `ERROR:` on
  STDOUT -- stderr is empty on every stage-binary drive, error or clean
  (probed) -- deliberately outside the vocabulary below: case-exactness
  keeps the compile faces out of the judge, and the honest exit makes
  them unnecessary there.

### The build face (`vivado-build`)

- **`--part` is required BEFORE any drive** (exit 2 at the CLI naming
  the flag): the probed alternative is a ~4s synth-then-fail inside the
  tool. The sim face consumes no part -- the flag on any other face is a
  loud parse error, never a dropped argument.
- **The synthesis script is GENERATED** per top, written to
  `orbit-synth-<top>.tcl`: one `read_vhdl` / `read_verilog` per file
  (the batch `read_*` commands take one file per command, and read
  order is irrelevant -- probed on 2025.2.1), then `synth_design -top
  <top> -part <part> -mode out_of_context`, then the three netlist
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

- **xelab pins the library and names the top**
  (`xelab -L work work.<top> -s <top>`); the snapshot lands as
  `xsim.dir/<top>` and the elaboration validates the top (an unknown top
  answers `ERROR: [XSIM 43-3225] Cannot find design unit ...`, exit 1,
  probed).
- **xsim runs each snapshot once** (`xsim <top> -R`), and THE RUN EXIT
  IS A LIE: xsim exits 0 on a violated `assert ... severity failure`
  AND on a violated `severity error` (both probed live -- the face
  prints, the simulation continues or stops, and the process exits 0).
  The log is the only truth, and the judge is the load-bearing gate --
  the only drive in this crate where the exit cannot convict a probed
  failure. The exit stays the crash-door witness: a missing snapshot
  exits 1 with an uppercase face the vocabulary does not carry.
- **The vocabulary is line-anchored, the only one in this crate.** The
  faces are `Fatal:` / `Error:` / `Failure:` at LINE START, case-exact:
  a case drift (`FATAL:`) and a marker mid-line (a testbench's own
  report note, an echoed source line) never judge. `Warning:` and
  `Note:` lines never judge.

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
| re-analysis, elaboration and run all exit 0 | a second drive in one output directory: provider then consumer re-analyzed, `-e`, `-r` | nvc tolerates stale-library re-analysis without ghdl's obsoleted-design refusal -- the wipe is symmetry here, not load-bearing |

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
| `Failure: the design is wrong` (+ the `Time: ... File: ...` line, on stdout) | `xvhdl -work work tb_fatal.vhd && xelab -L work work.tb_fatal -s sim_fail && xsim sim_fail -R` (same through the drive) | a violated `assert ... severity failure` stops the check -- and **xsim EXITS 0**: the run exit is a lie, the line-start face is the only witness |
| `Error: check failed` (+ the continuation `Note:` line, on stdout) | `xsim sim_err -R` on a `severity error` assert | the face prints, the simulation **continues** (`Note: continued past the error` follows), and **xsim still exits 0** -- the same lie; the face convicts |
| `Note: sim done`, the kernel `INFO:` lines | `xsim sim_clean -R` on a passing testbench | the clean run exits 0 -- continuations, **never judged** |
| `ERROR: [VRFC 10-4982] syntax error near 'end' [...:9]` (+ `ERROR: [VRFC 10-2989]`, then the `10-8704` ignored line, on **stdout**) | `xvhdl -work work broken.vhd` (same through the drive, which joins the streams) | the compile-error face, **exit 1**; stderr is EMPTY on every stage-binary drive, error or clean; the compile gates on the exit and is never judged -- the uppercase face is deliberately outside the mixed-case vocabulary |
| `ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'` | `xvhdl -work work tb_clean.vhd top.vhd gate.vhd` (consumer before provider) | **order-sensitive within work**, exit 1 -- the deferral loop's reason to exist here; even the tb-gate-top order fails (`gate` before `top` is required) |
| `INFO: [VRFC 10-2263] Analyzing Verilog file ...` , exit 0 | `xvlog vtop.v vgate.v` (consumer before provider) | **order-free** -- xvlog compiles in one invocation, no deferral loop; `-work work` pins the library |
| `ERROR: [XSIM 43-3225] Cannot find design unit work.tb_nosuch in library work located at xsim.dir/work.` | `xelab -L work work.tb_nosuch -s sim_nosuch` | the elaboration refusal of an unknown top, **exit 1** -- honest, the elaboration gates on the exit |
| `ERROR: Please check the snapshot name ... does not exist` | `xsim nosuch_snap -R` | the missing-snapshot face, **exit 1** -- the crash door; uppercase, outside the vocabulary, the exit convicts |
| `SYNTH FAILED: ERROR: [Common 17-69] Command failed: Vivado Synthesis failed`, **exit 1** | `vivado -mode batch -nojournal -nolog -source synth.tcl` with a syntax-broken `read_vhdl` inside the catch-guard | the fail-fast batch face: the return-code catch-guard stops the script and exits 1 (unguarded batch **continues past a failed synth_design**, probed) |
| **exit 1 with all three netlists written** | the same script whose guard tested `$synth_err` emptiness instead of catch's return code | **the probed Tcl trap**: `catch {script} var` writes the script's NORMAL RETURN VALUE into `var`, and `write_verilog`/`write_checkpoint` return the written filename -- an emptiness guard false-fails a CLEAN synth |
| exit 0, `netlists/top.dcp` + `top.v` + `top.edf` written | `vivado -mode batch -nojournal -nolog -source synth.tcl`, guard on catch's return code, `synth_design -top top -part xc7z010clg400-1 -mode out_of_context` | the clean synth face; the echo of the sourced commands rides stdout, the synth errors ride **stderr**, exit honest |
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

## Honest limits

- **The filesets are per tool.** The VHDL drives (`ghdl`, `nvc`) carry
  the `VHDL` fileset only; the Verilog drives (`iverilog`, `verilator`,
  `yosys`) carry `VLOG` and `SYSV` (their native filesets) and reject
  `VHDL` and custom filesets with a loud exit-2 ("not yet carried"),
  never a silent skip. The vivado drives carry all three (`VHDL` +
  `VLOG` + `SYSV`) and reject only custom filesets -- vivado is the one
  carried tool that speaks both languages.
- **Six tool names, six vocabularies: ghdl, nvc, iverilog, verilator,
  yosys, vivado.** Any other tool name is a loud error at the CLI, never
  a guess.
- **The library column is carried, not mapped.** Every file analyzes
  into the tool's `work` library (ghdl's `work-obj08.cf` clock, nvc's
  `work/` directory), compiles into iverilog's one snapshot, or lints
  and builds as one verilator fileset; a multi-library orbit project is
  out of scope for v1.
- **The standards are pinned where the tools pin them** (`--std=08` for
  ghdl, `--std=2008` for nvc, `-g2012` for iverilog; verilator's pinned
  flags are `--timing` and the face flags -- its language setting stays
  verilator's default in v1) and cannot be overridden from the argv yet;
  the extras reach the drive's last step only (`-r` on the VHDL sim
  faces, `vvp` on iverilog's, the model argv on verilator's, the
  `yosys -p` synth argv on yosys's; the elaboration / compile / build
  argv on the build faces).
- **The state is run-scoped, not incremental.** Every drive wipes its
  clock first (the work libraries; iverilog's snapshot; verilator's
  `obj_dir`; yosys's `netlists/`; vivado's `xsim.dir/` and, on the build
  face, its `netlists/`). A large design pays a full re-analysis per
  drive; correctness before incrementality is the v1 stance. (Probed
  honestly: ghdl's wipe is load-bearing -- a stale library obsoletes its
  own design; nvc tolerates the stale library, iverilog's snapshot
  cannot be run past a failed compile, verilator's `--build` regenerates
  its intermediates, a failed yosys synth exits before its netlist could
  be mistaken for fresh, and a failed vivado compile or elaboration
  exits before the sim face could run a stale snapshot -- so those wipes
  are symmetry.)
- **Bare ghdl and bare vvp have holes; bare xsim is worse; the shipped
  paths catch them.** A `report ... severity error` statement slips a
  bare `ghdl -r` (face `(report error)`, exit 0, no judged face): the
  drive pins `--assert-level=error`, which stops the run and writes the
  `./<top>:error: report failed` face. A `$error` call slips a bare vvp
  (face `ERROR: ...`, simulation continued, **exit 0**): there the
  judge's face gate is the only witness, and it is load-bearing. xsim is
  the extreme -- BOTH probed runtime failures exit 0 (a severity failure
  and a severity error) -- so the vivado drive's judge is the
  load-bearing gate on every run, and the line-start vocabulary is the
  only witness on them. nvc, verilator, and yosys have no such hole
  probed: nvc's severity-error runs continue the simulation but still
  exit 1, every verilator model failure dies by SIGABRT -- no exit code
  at all -- and every probed yosys failure exits 1, so their exits
  convict.
- **A face shape mid-text judges fail -- except on vivado.** The
  substring vocabularies cannot tell your prose from the tool's: a
  testbench's own report note that embeds a ghdl face shape
  (`bus:error: retrying`), an nvc face shape (`** Error: retrying`), a
  vvp face shape (`ERROR: retrying`), a verilator face shape
  (`%Error: retrying`), or a yosys face shape (`ERROR: retrying`)
  convicts the run. vivado is the deliberate exception: its faces are
  line-anchored, so only a line LEADING with the marker convicts and
  mid-text prose never does. Keep report notes and `$display` text free
  of face shapes anyway.
- **Verilator builds need a C++ toolchain.** `--binary --build` shells
  out to `make` and the system C++ compiler (probed on this box with
  both on PATH); a box without them fails the build step loudly,
  surfaced as an exit-1 build failure with the toolchain's own output
  relayed.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **The tools resolve on PATH, vivado from its env hook.** The orbit
  target contract carries no binary-path field: every drive but vivado
  pins the bare name; vivado resolves from `$VIVADO_BIN` (the install
  path) with the bare name on PATH as the fallback, and its stage
  binaries ride beside the resolved path (or on PATH). The vivado recipe
  env rides the drive's own spawn scoping -- see the launch recipe.
- **Blueprint paths with spaces are a v1 limit on the script faces.**
  yosys's `-p` script and vivado's generated synth script carry the file
  paths verbatim, split on whitespace by the tool's own parser: a path
  with a space would split and fail the read loudly.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends. (yosys's synthesis log
  relays only on failure -- on success the drive reports the netlist,
  not kilobytes of synthesis log.)
- **License.** The Cargo.toml declares `MIT OR Apache-2.0`; the license
  texts are held until the maintainer conversation. Orbit itself is
  GPL-3 -- irrelevant here: a target is a spawned process, not a derived
  work of orbit.

## Development

```sh
cargo test            # the contract, drive, and argv suites
cargo test -- --ignored   # + the real-spawn tests: the real binary, real
                          # ghdl, nvc, iverilog, verilator, yosys, and
                          # vivado, the real orbit spawn shape,
                          # order-dependent blueprints through the
                          # deferral loop, judged severity failures and
                          # $fatal/$error calls, twice in one output
                          # directory, the build faces, and the vivado
                          # lying-exit gate
```

The contract suite (`tests/contracts.rs`) ports the aquila contract tests
fixture-for-fixture (upstream: `github.com/chaseruskin/aquila`,
`tests/test_contracts.py`) and extends them with the probed faces of the
other tools: the real failing logs judge fail; the clean logs (including
the one mentioning "ERROR" mid-text) judge pass; the uppercase (ghdl) and
lowercase (nvc, vvp, verilator, yosys) drifts never judge; vivado's pins
run both ways -- its faces judge at line start only, and a case drift or
a marker mid-line never judges; the severity-error, `$error`,
model-abort, and lying-xsim-exit faces judge fail; the warning/note
faces stay clean; and the registry pins exactly the carried tools. The
drive suites pin the deferral loop (a consumer-before-provider pair must
retry), the no-progress stop, the TSV blueprint parse, the crash door (a
clean log does not green-light a nonzero exit), the Tcl catch-guard
shape, and the judge gate on fake severity-error logs for the VHDL
tools.
