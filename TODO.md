# TODO

- the pass-marker face: a corpus-declared success phrase as the judge's
  third witness (the GRLIB zedboard systest halts by a designed severity
  failure; the spec lives in the campaign ledger).
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
