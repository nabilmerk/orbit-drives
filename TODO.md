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
- RESOLVED 2026-11-25 (falsified): the ghdl window-clear
  "mis-execution" -- the Nov 22 probe ran under the starved-analysis
  era (287 duplicate-row-starved files, the std-pin wall, unprovisioned
  vendor entities: partial sets elaborate undriven models, and a loader
  loop fed undriven data never exits). The SAME ghdl binary
  (/usr/local/bin/ghdl, built 2026-03-07) now passes the systest at the
  reference figure on complete closures. No ghdl bug; no upstream
  issue. Re-open only if a hang resurfaces on a verified-complete
  analysis set.
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

- RESOLVED 2026-11-25 (the polish batch): the honest-limits duplicate
  bullet merged, the quickstart's prose pointers linked to the honest
  limits, the pages workflow caches cargo builds (mdbook installs only
  on dependency changes), and every pinned action bumped to its
  current major (probed via the releases API: checkout v7,
  configure-pages v6, upload-pages-artifact v5, deploy-pages v5,
  login-action v4, setup-buildx-action v4, build-push-action v7).
- the cocotb PYTHON_BIN Windows face (the loud exit-2 by design).
- RESOLVED 2026-11-22 (the deferred-minors round): the sibling
  same-library relative-dep fixture (s10 case D, "local" now honest),
  the `Manifest::from_file` std rejection through the real load path,
  and the elaborate/run std following the top's owning library (nvc +
  ghdl). The reviewer's custom-fileset std-bypass (M6) closed as a
  FALSE finding empirically: custom filesets match within the
  discovered file set, and ignored files do not ride them.
