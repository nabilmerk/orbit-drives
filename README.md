# orbit-contracts

The probed failure vocabularies and the honest-exit target drives for the
[orbit](https://github.com/chaseruskin/orbit) HDL package manager.

Orbit's only contract with a target is the exit code: nonzero fails the
build. That contract only means something if the target's exit is honest --
so `orbit-contracts` never trusts a single witness. Each drive captures the
tool's log and gates it on the tool's **probed failure vocabulary** (faces
copied from live runs, never invented) **and** on the tool's exit code:
findings = a face present OR a nonzero exit. Either witness convicts. The
judge exists because the exits lie: bare ghdl exits 0 on a violated
`severity error` check -- it prints the face and continues -- and the exit
exists as a witness because a killed simulation can leave no face at all.

## Install

```sh
cargo install --path .
```

No dependencies beyond the Rust standard library.

## Wiring into orbit

Include the shipped target fragment into an orbit project configuration:

```sh
orbit config --push include=/path/to/orbit-contracts/config.toml
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

```
orbit-contracts <SUBCOMMAND> --blueprint <path> --top <name> [extra tool args...]
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

Options:

- `--blueprint <path>` -- the TSV blueprint; defaults to `$ORBIT_BLUEPRINT`.
- `--top <name>` -- a top unit to elaborate (and run: `ghdl`); repeatable.
  With no `--top`, the drive runs `$ORBIT_TB_NAME` (orbit exports it; empty
  in all-mode counts as absent). Exit 2 when neither names a top.
- anything else -- a tool extra, appended verbatim to the drive's LAST
  tool argv: each `ghdl -r` / `nvc -r` on the sim faces, each `ghdl -e` /
  `nvc -e` on the build faces (there is no run to carry them, and a
  dropped argument would be a silent degradation; an option the
  elaboration does not speak is the tool's own loud rejection, surfaced
  as an exit-1 elaborate failure).

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

## Honest limits

- **VHDL only.** `VLOG`, `SYSV`, and custom filesets are a loud exit-2
  ("not yet carried"), never a silent skip. Mixed-language designs stay out
  of the contract.
- **Two vocabularies: ghdl and nvc.** Both are VHDL simulators. Any other
  tool name is a loud error at the CLI, never a guess.
- **The library column is carried, not mapped.** Every file analyzes into
  the tool's `work` library (ghdl's `work-obj08.cf` clock, nvc's `work/`
  directory); a multi-library orbit project is out of scope for v1.
- **The standards are pinned** (`--std=08` for ghdl, `--std=2008` for nvc)
  and cannot be overridden from the argv yet; the extras reach the drive's
  last step only (`-r` on the sim faces, `-e` on the build faces).
- **The state is run-scoped, not incremental.** Every drive wipes the work
  library first. A large design pays a full re-analysis per drive;
  correctness before incrementality is the v1 stance. (Probed honestly:
  ghdl's wipe is load-bearing -- a stale library obsoletes its own design;
  nvc tolerates the stale library, and its wipe is symmetry.)
- **Bare ghdl only:** a `report ... severity error` statement slips a bare
  `ghdl -r` (face `(report error)`, exit 0, no judged face). The shipped
  path catches it: the drive pins `--assert-level=error`, which stops the
  run and writes the `./<top>:error: report failed` face. nvc has no such
  hole probed: its severity-error runs continue the simulation but still
  exit 1.
- **A face shape mid-text judges fail.** The vocabularies are case-exact
  substrings and cannot tell your prose from the tool's: a testbench's own
  report note that embeds a ghdl face shape (`bus:error: retrying`) or an
  nvc face shape (`** Error: retrying`) convicts the run. Keep report notes
  free of face shapes.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **The tools resolve on PATH.** The orbit target contract carries no
  binary-path field, so the drives pin no path.
- **License.** The Cargo.toml declares `MIT OR Apache-2.0`; the license
  texts are held until the maintainer conversation. Orbit itself is
  GPL-3 -- irrelevant here: a target is a spawned process, not a derived
  work of orbit.

## Development

```sh
cargo test            # the contract, drive, and argv suites
cargo test -- --ignored   # + the real-spawn tests: the real binary, real
                          # ghdl and real nvc, the real orbit spawn shape,
                          # order-dependent blueprints through the deferral
                          # loop, judged severity failures, twice in one
                          # output directory, and the build faces
```

The contract suite (`tests/contracts.rs`) ports the aquila contract tests
fixture-for-fixture (upstream: `github.com/chaseruskin/aquila`,
`tests/test_contracts.py`) and extends them with nvc's probed faces: the
real failing logs judge fail; the clean logs (including the one mentioning
"ERROR" mid-text) judge pass; the uppercase (ghdl) and lowercase (nvc)
drifts never judge; the severity-error faces judge fail; the warning/note
faces stay clean; and the registry pins exactly the two carried tools. The
drive suites pin the deferral loop (a consumer-before-provider pair must
retry), the no-progress stop, the TSV blueprint parse, the crash door (a
clean log does not green-light a nonzero exit), and the judge gate on fake
severity-error logs for both tools.
