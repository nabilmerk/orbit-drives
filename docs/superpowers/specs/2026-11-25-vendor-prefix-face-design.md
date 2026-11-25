# The vendor-prefix face: drives-side external-library detection

## Intent and why

A zedboard-class design references vendor libraries (`unisim`,
`unisim_retarget`) that no dependency-graph project provides. ghdl
refuses such designs until a compiled vendor library is provisioned;
today that provisioning rides a hand-set `GHDL_PREFIX` (the campaign
recipe, 2026-11-25). This change moves the knowledge and the wiring
into orbit-drives: the drive detects, from the blueprint alone, that
the design references libraries it does not carry, wires a provisioned
prefix onto its spawns, and says so loudly when nothing is provisioned.
No orbit change: the blueprint already carries each file's path and
library, which is everything the detection needs.

## Goals

- The ghdl sim face scans its blueprint's VHDL files for use-clause and
  library-clause references (comment-stripped, case-insensitive),
  diffs them against the blueprint's own libraries plus the predefined
  set (`ieee`, `std`, `work`), and names the remainder the external
  libraries.
- New env face `ORBIT_DRIVES_GHDL_VENDOR_PREFIX`: when set, the value
  rides as `GHDL_PREFIX` on every ghdl spawn (analyze, elaborate, run)
  -- ghdl 6's loader searches `<prefix>/<lib>/v93/<lib>-obj<STD>.cf`,
  probed live 2026-11-25.
- When external libraries are detected, the face is unset, and the
  environment's own `GHDL_PREFIX` is unset too: a loud exit-2 hint
  before any analysis, naming the libraries and pointing at the
  provisioning recipe. The hint can never fire when the environment
  already provides a prefix.
- nvc and vivado need nothing: nvc ships precompiled vendor libraries
  (it resolved unisim natively, probed), vivado rides its own
  precompiled/`xsim.ini` flow.

## Non-goals

- No orbit change (no planner detection, no blueprint format change).
- No vendor-library compilation inside the drives: compiling is a
  once-per-machine vendor setup step (ghdl ships
  `scripts/vendors/compile-xilinx-vivado.sh` for it); the recipe stays
  documented, the drive consumes its product.
- No argv option in `config.toml` target blocks: the corpus-level
  property rides the env-face family (`FLAGS`, `ASSERT_LEVEL`), the
  established shape.

## Anti-goals

- A false detection must never fail a run that would otherwise pass:
  the gate fires only when nothing provisioned the prefix, and the
  provisioning can always be supplied, so a grep-grade false positive
  degrades to a hint that a one-line env silences.
- The scan never rejects a file or reorders the blueprint: it only
  observes.

## Approaches

1. **Drives-side detect-and-consume (picked).** The blueprint scan plus
   the prefix face, per the asymmetry that motivated this: the need is
   per tool (nvc needs nothing, ghdl needs a prefix, vivado has its
   own), and the detection input (files + libraries) is already the
   drive's input. Ships in this repo, on this repo's schedule, against
   any orbit version.
2. **orbit-side detection** (plan-time external-library face on the
   blueprint). Parser-exact and contract-wide, but it costs an orbit
   change (upstream coordination while four PRs sit unmerged) for
   information the drive can derive from its own input.
3. **Drives-side vendor compilation** (`--compile-vendor`). One
   command, but it puts once-per-machine vendor setup logic inside a
   run tool -- the wrong shape; the recipe stays outside.

## Constraints

- std-only Rust, edition 2021; the crate's existing error and
  print-block conventions.
- The scanner is grep-grade by design: comment-stripped lines split on
  whitespace, `use <lib>.` and `library <lib>;` tokens collected
  case-insensitively. Its verdicts gate wiring and hints only, never
  file selection or order.
- The usage-class exit code (2) for the hint, per the contracts face.

## Left to the builder

- The hint's exact wording (family voice, names the libraries, points
  at the recipe).
- Test fixture layout for the scanner (temp dirs keyed by the test
  name -- the parked pid-keyed collision).
- Whether the note/warn line prints on the provisioned path (it
  should: one line naming the prefix and the external set).
