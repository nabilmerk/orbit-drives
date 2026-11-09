# verilator drive


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

