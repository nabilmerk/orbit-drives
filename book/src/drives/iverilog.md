# iverilog drive


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

