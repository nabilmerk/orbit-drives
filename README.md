# orbit-contracts

The probed failure vocabularies and the honest-exit target drives for the
[orbit](https://github.com/chaseruskin/orbit) HDL package manager.

Orbit's only contract with a target is the exit code: nonzero fails the
build. That contract only means something if the target's exit is honest --
so `orbit-contracts` never trusts a tool's exit alone. Each drive captures
the tool's log and gates it on the tool's **probed failure vocabulary**:
faces copied from live runs, never invented, matched as case-exact
substrings with no line anchors (the tools prefix their failure lines with
file paths and image names, not severities). The judge exists because the
exits lie: ghdl exits **0** on a violated `severity error` check -- it
prints the face and continues -- so the face in the log is the only witness.

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
orbit test --target ghdl -- -gWIDTH=8 --vcd=waves.vcd
```

Orbit spawns the target in the target's output directory with
`ORBIT_BLUEPRINT` pointing at the plan file (the TSV plan: one
`fileset<TAB>library<TAB>filepath` line per collected file) and appends
everything after `--` to the target's command, where the drive passes it
verbatim to each `ghdl -r`.

## Direct usage

```
orbit-contracts ghdl --blueprint <path> --top <name> [--] [extra ghdl args]
```

- `--blueprint <path>` -- the TSV blueprint; defaults to `$ORBIT_BLUEPRINT`.
- `--top <name>` -- a top unit to elaborate and run; repeatable, required.
- `--` -- everything after is appended to each `ghdl -r` argv.

Exits:

| code | meaning |
|------|---------|
| 0 | clean -- every run log judged clean |
| 1 | findings -- a log carried a failure face, or ghdl reported a broken design |
| 2 | usage -- bad argv, unreadable blueprint, a fileset the drive does not carry |

## The ghdl drive

1. Read the blueprint; every non-VHDL entry (`VLOG`, `SYSV`, a custom
   fileset) is a loud exit-2, never a silent skip.
2. Analyze the VHDL files in blueprint order with the **deferral loop**:
   ghdl is order-sensitive, so a failing analyze defers its file to a retry
   after the others; a pass that makes no progress stops the loop with every
   residual stderr surfaced verbatim.
3. Elaborate and run each `--top` (`ghdl -r --std=08 <top>
   --assert-level=error <extra args>`; the assert level is a simulation
   option, legal only after the toplevel unit) and capture the log.
4. Judge the log. The judge -- not the exit code -- is the gate.

## The probed facts

Every pattern in the vocabulary is a face read off a live run. All faces
below were probed on **ghdl 6.0.0**, verified by the controller and
reproduced locally, **2026-11-07** (`ghdl -r --std=08` on assert-driven
testbenches; a non-static assert condition -- a statically-false
`assert false report` folds into a report face):

| face | what wrote it | what ghdl did |
|------|---------------|---------------|
| `(assertion failure)` | a violated `assert ... severity failure` | stopped the run, exit 1 |
| `./<top>:error:` (`assertion failed`, `simulation failed`) | the run-level stop of the same | exit 1 |
| `(assertion error)` | a violated `assert ... severity error` | printed and **continued, exit 0** -- the hole the judge exists for |
| `(assertion warning)`, `(assertion note)` | the gentler severities on a continuing run | continuations, **never judged** |
| `(report failure)` | a statically-false `assert false report ... severity failure` (folded to a report) | stopped the run, exit 1 through `./<top>:error: report failed` -- caught by `:error:` |
| `(report error)` | `report ... severity error` | printed and continued, exit 0, **no judged face** -- see limits |
| `--assert-level=error` | a simulation option | legal only AFTER the toplevel unit; before it: `ghdl:error: unknown command option` |
| `ghdl -e` | elaboration | exactly one unit name per invocation (`too many unit names for command '-e'`) |
| `ghdl <command> [options]` | argv shape | command first: `ghdl -a --std=08 <file>`; options before the command are `unknown command` |

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
  extra `--` args reach only the `ghdl -r` step.
- **A `report ... severity error` statement** (a report, not an assert)
  prints `(report error)`, continues, exits 0, and carries no probed face:
  v1's gate does not catch it. Assert-driven testbenches are the contract.
- **The exit stays secondary.** A nonzero ghdl exit with a clean log (a
  crash face) does not flip the verdict; it is printed as an advisory note.
  The vocabulary carries only probed faces.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **ghdl resolves on PATH.** The orbit target contract carries no
  binary-path field, so the drive pins no path.

## Development

```sh
cargo test
```

The contract suite (`tests/contracts.rs`) ports the aquila contract tests
fixture-for-fixture: the real failing log judges fail; the clean logs
(including the one mentioning "ERROR" mid-text) judge pass; the uppercase
drifts never judge; the severity-error face judges fail; the warning/note
faces stay clean; and the registry pins exactly one carried tool. The drive
suite pins the deferral loop (a consumer-before-provider pair must retry),
the no-progress stop, the TSV blueprint parse, and the judge gate on a fake
severity-error log with exit 0.
