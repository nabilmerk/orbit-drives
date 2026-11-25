# TODO

- RESOLVED 2026-11-25: the zedboard drive-path close-out — orbit's
  `parse_body` counted a process's body-`begin` toward the primary-end
  nesting, so architectures with processes swallowed every unit
  declared after them in the same file (`mul_inferred.vhd` fell out of
  the closure, starving downstream compiles). Fixed on the fork
  (`fix-parse-nesting`, trunk 296d5452; inline + s14 tests); the drive
  verdict is `pass` at 940,860 ns through the shipped vivado face.
- RESOLVED 2026-11-22: the pass-marker face shipped
  (`ORBIT_DRIVES_PASS_MARKER`, the judge's third witness on all six sim
  faces; the live three-arm proof rides the ghdl shape suite).
- RESOLVED 2026-11-22: the cocotb/quartus ALLHDL raw-filter bug — both
  sim faces route by the row's extension now (probed live on cocotb).
- the ghdl window-clear mis-execution: ghdl-LLVM and ghdl-mcode both
  never terminate the GRLIB loader's register-window clear loop; nvc
  terminates it in 8 iterations. Upstream ghdl issue pending.
  UPDATE 2026-11-25: ghdl 6.0.0 Dunoon did NOT reproduce the hang for
  the zedboard leon3mp systest -- it passed end-to-end at 940,860 ns;
  the upstream probe remains for older ghdl editions.
- the cocotb diagnostics' /tmp literal paths: probe-face text only, but
  worth a path-sep pass.
- the Vivado recipe is Linux-probed (the SuSE ncurses env legs); the
  Windows/macOS Vivado launch recipes are unprobed.
- the cross-platform live-tool matrix: the gated live tests per OS,
  once vendor tools exist on each.

## Parked

- the vivado std face: xvhdl/xvlog std selectors unmapped in v1 (the
  tool is absent on this box; the fork's orbit#24 std rides every other
  drive).
- the pid-keyed shape-test temp dirs collide across parallel tests in
  one process (cargo runs the harness's threads in one process): a
  pre-existing race the cocotb ALLHDL test's timing surfaced — key the
  dirs by test name instead.
- RESOLVED 2026-11-25: the ghdl vendor-unisim completion path -- the
  recipe lives in the campaign record (ghdl's vendors script over the
  merged ISE+modern source tree at `<demo>/ghdl-unisim-src`, cf
  consumed through the `GHDL_PREFIX` mirror at `<demo>/ghdl-prefix`);
  the zedboard ghdl face passes end-to-end. The manual env shipped as
  the `ORBIT_DRIVES_GHDL_VENDOR_PREFIX` face (2026-11-25): the drive
  detects the external libraries from the blueprint and wires the
  prefix itself, exit-2 with a hint when unprovisioned.
- orbit's symbol parser rejects multi-dimensional array type
  constraints (`array (integer range <>, integer range <>) of ...`,
  VHDL-2008) — the vendor `unisim_VPKG.vhd` is unplannable
  ("invalid syntax"); upstream-class finding, recorded for the issue
  queue.

- the honest-limits duplicated run-log bullet (a merge-candidate on the
  next touch).
- the quickstart prose-vs-link polish.
- the mdbook install caching (a future CI pass).
- the action/runner version-bump pass.
- the cocotb PYTHON_BIN Windows face (the loud exit-2 by design).
- RESOLVED 2026-11-22 (the deferred-minors round): the sibling
  same-library relative-dep fixture (s10 case D, "local" now honest),
  the `Manifest::from_file` std rejection through the real load path,
  and the elaborate/run std following the top's owning library (nvc +
  ghdl). The reviewer's custom-fileset std-bypass (M6) closed as a
  FALSE finding empirically: custom filesets match within the
  discovered file set, and ignored files do not ride them.
