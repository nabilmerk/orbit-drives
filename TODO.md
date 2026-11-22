# TODO

- RESOLVED 2026-11-22: the pass-marker face shipped
  (`ORBIT_DRIVES_PASS_MARKER`, the judge's third witness on all six sim
  faces; the live three-arm proof rides the ghdl shape suite).
- RESOLVED 2026-11-22: the cocotb/quartus ALLHDL raw-filter bug — both
  sim faces route by the row's extension now (probed live on cocotb).
- the ghdl window-clear mis-execution: ghdl-LLVM and ghdl-mcode both
  never terminate the GRLIB loader's register-window clear loop; nvc
  terminates it in 8 iterations. Upstream ghdl issue pending.
- the cocotb diagnostics' /tmp literal paths: probe-face text only, but
  worth a path-sep pass.
- the Vivado recipe is Linux-probed (the SuSE ncurses env legs); the
  Windows/macOS Vivado launch recipes are unprobed.
- the cross-platform live-tool matrix: the gated live tests per OS,
  once vendor tools exist on each.

## Parked

- the cocotb and quartus sim faces' raw `fileset != "VHDL"` filter
  (cocotb.rs / quartus.rs): it bypasses the ALLHDL-by-extension routing
  every shipped target rides (`is_vhdl_row`), so an ALLHDL blueprint's
  `.vhd` rows are refused exit-2 through those two faces -- surfaced by
  the fork campaign's code map, 2026-11-22.
- the vivado std face: xvhdl/xvlog std selectors unmapped in v1 (the
  tool is absent on this box; the fork's orbit#24 std rides every other
  drive).
- the pid-keyed shape-test temp dirs collide across parallel tests in
  one process (cargo runs the harness's threads in one process): a
  pre-existing race the cocotb ALLHDL test's timing surfaced — key the
  dirs by test name instead.
- the ghdl vendor-unisim completion path, probed 2026-11-22 on Vivado
  2025.2.1: the declarations ride (`unisim` project, VCOMPONENTS only),
  the techmap wrappers need the primitive entities, whose recipe is
  `--std=08 -fsynopsys -frelaxed` — but this Vivado's `vital2000`
  sources are absent and `unisim_VITAL/SMODEL` were dropped, so the
  full primitive set cannot compile here; obtain vital2000 (ghdl-src
  ships a copy) or use Vivado's precompiled xsim flow (which works and
  is the corpus's reference run).
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
