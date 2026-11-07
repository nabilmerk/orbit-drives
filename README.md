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

- `ghdl` -- the sim face: analyze, elaborate, run, judge.
- `ghdl-build` -- the build face: analyze, elaborate, exit honest. No
  simulation, no run log, nothing judged: `ghdl -a` and `ghdl -e` exit
  nonzero on a broken design (probed live), so the exit is the only
  witness a build needs.

Options:

- `--blueprint <path>` -- the TSV blueprint; defaults to `$ORBIT_BLUEPRINT`.
- `--top <name>` -- a top unit to elaborate (and run: `ghdl`); repeatable.
  With no `--top`, the drive runs `$ORBIT_TB_NAME` (orbit exports it; empty
  in all-mode counts as absent). Exit 2 when neither names a top.
- anything else -- a tool extra, appended verbatim to the drive's LAST
  tool argv: each `ghdl -r` on the sim face, each `ghdl -e` on the build
  face (there is no run to carry them, and a dropped argument would be a
  silent degradation; an option the elaboration does not speak is ghdl's
  own loud rejection, surfaced as an exit-1 elaborate failure).

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

## The probed facts

Every pattern in the vocabulary and every drive fact below was probed on
**ghdl 6.0.0 on this box, 2026-11-07**, on assert- and report-driven
testbenches, both bare and through the drive. Each row carries the argv
that produced the face.

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

## Honest limits

- **VHDL only.** `VLOG`, `SYSV`, and custom filesets are a loud exit-2
  ("not yet carried"), never a silent skip. Mixed-language designs stay out
  of the contract.
- **One vocabulary.** Only ghdl is carried. An unknown tool name is a loud
  error at the CLI, never a guess.
- **The library column is carried, not mapped.** Every file analyzes into
  ghdl's `work` library; a multi-library orbit project is out of scope for
  v1.
- **`--std=08` is pinned** and cannot be overridden from the argv yet; the
  extras reach the drive's last step only (`ghdl -r` on the sim face,
  `ghdl -e` on the build face).
- **The state is run-scoped, not incremental.** Every drive wipes
  `work-obj08.cf` first. A large design pays a full re-analysis per drive;
  correctness before incrementality is the v1 stance.
- **Bare ghdl only:** a `report ... severity error` statement slips a bare
  `ghdl -r` (face `(report error)`, exit 0, no judged face). The shipped
  path catches it: the drive pins `--assert-level=error`, which stops the
  run and writes the `./<top>:error: report failed` face.
- **A lowercase face shape mid-text judges fail.** A testbench's own report
  note that embeds a lowercase ghdl face (`bus:error: retrying`) convicts
  the run: the vocabulary is case-exact lowercase substrings and cannot
  tell your prose from ghdl's. Keep report notes free of ghdl-face shapes.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **ghdl resolves on PATH.** The orbit target contract carries no
  binary-path field, so the drive pins no path.
- **License.** The Cargo.toml declares `MIT OR Apache-2.0`; the license
  texts are held until the maintainer conversation. Orbit itself is
  GPL-3 -- irrelevant here: a target is a spawned process, not a derived
  work of orbit.

## Development

```sh
cargo test            # the contract, drive, and argv suites
cargo test -- --ignored   # + the real-spawn test: the real binary, real
                          # ghdl, the real orbit spawn shape, twice in one
                          # output directory
```

The contract suite (`tests/contracts.rs`) ports the aquila contract tests
fixture-for-fixture (upstream: `github.com/chaseruskin/aquila`,
`tests/test_contracts.py`): the real failing log judges fail; the clean
logs (including the one mentioning "ERROR" mid-text) judge pass; the
uppercase drifts never judge; the severity-error face judges fail; the
warning/note faces stay clean; and the registry pins exactly one carried
tool. The drive suite pins the deferral loop (a consumer-before-provider
pair must retry), the no-progress stop, the TSV blueprint parse, the crash
door (a clean log does not green-light a nonzero exit), and the judge gate
on a fake severity-error log.
