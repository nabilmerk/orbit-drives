# yosys drive


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

