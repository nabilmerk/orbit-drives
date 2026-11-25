# The env faces

The drives carry interim env faces for corpus facts nothing else in the
stack expresses. All are unset-by-default; all ride verbatim.

| env | rides | default |
|---|---|---|
| `ORBIT_STD` | set by orbit (the fork's #24 face): the working project's declared std; inherited by every std-less blueprint row in every drive | — |
| `ORBIT_DRIVES_NVC_STD` | `--std=<value>` on every nvc spawn (below the row std and `ORBIT_STD`) | nvc's own default |
| `ORBIT_DRIVES_NVC_FLAGS` | whitespace-split flags in every nvc spawn's global block, right after the std flag (the fork's option-A face, 2026-11-22) | nothing |
| `ORBIT_DRIVES_NVC_HEAP` | `-H <size>` on every nvc spawn | nvc's default heap |
| `ORBIT_DRIVES_GHDL_FLAGS` | whitespace-split flags on every ghdl spawn | none |
| `ORBIT_DRIVES_GHDL_ASSERT_LEVEL` | `--assert-level=<value>` in the run argv | `error` |
| `ORBIT_DRIVES_GHDL_VENDOR_PREFIX` | sets `GHDL_PREFIX` on every ghdl spawn so a referenced vendor library (`unisim`, `unisim_retarget`, ...) resolves from a precompiled prefix; when the blueprint references external libraries and neither this face nor `GHDL_PREFIX` is set, the drive exits 2 with the hint | unset |
| `ORBIT_PROJECT_LIBRARY` | set by orbit; the drive reads it | — |

## The probed provenance

- `ORBIT_DRIVES_NVC_FLAGS`: vendor-library corpora pin their own recipe.
  The Xilinx unisim class compiles cleanest under nvc's `--relaxed`
  (disable certain pedantic rule checks; existence probed on nvc 1.23.1,
  2026-11-22). The GRLIB zedboard corpus needed no nvc flags — 523/523
  analyzed clean at the drive's `--std=2008` pin (probed 2026-11-22).
- The GRLIB corpus's ghdl recipe, probed end to end 2026-11-22 (GRLIB's
  own `bin/Makefile:492` confirms it):
  `ORBIT_DRIVES_GHDL_FLAGS="-fexplicit --ieee=synopsys --mb-comments
  --warn-no-binding --warn-no-hide"` — and ghdl's declared `std = "93"`
  rides the tool's DEFAULT (the explicit `--std=93` pin walls
  GRLIB-class corpora: the `_tp` test packages' `2**(gAWidth-2)` bounds
  are strict-93-illegal while the default analysis takes them; the
  drives' stdmap maps ghdl's 93 to no flag). 399/411 files analyzed
  clean; the residual 12 are the unisim techmap wrappers starving on
  the vendor `unisim` library, which must be provisioned corpus-side.
- Pin either face through orbit's config env (per user or host):
  `orbit config --push env.ORBIT_DRIVES_GHDL_FLAGS="--fexplicit
  --ieee=synopsys --mb-comments"`. The run-phase extras
  (`--ieee-asserts=disable`) do NOT belong in the flags face — it rides
  every spawn and ghdl rejects run-only options at analysis; pass them
  as extras instead (`orbit test --target ghdl -- --ieee-asserts=disable`).
- `ORBIT_DRIVES_NVC_STD`: the GRLIB corpus is 93-written; nvc's default is
  2008-era semantics and the homograph rule walls the stdlib's duplicate
  HRead declarations (probed 2026-11-08).
- `ORBIT_DRIVES_NVC_HEAP`: the zedboard design's memory models exhaust
  nvc's default heap at simulation init (probed 2026-11-08).
- `ORBIT_DRIVES_GHDL_FLAGS`: the Xilinx unisim tree pins the Synopsys
  packages; ghdl re-checks flag consistency at every LOAD of an analyzed
  unit (probed 2026-11-09).
- `ORBIT_DRIVES_GHDL_ASSERT_LEVEL`: the level-error pin halted the
  zedboard sim at 0 ms on the AHB-to-AXI bridge's tolerated init note
  (probed 2026-11-09); ghdl's duplicate-option precedence is last-wins.


### Provisioning a vendor library for ghdl

A design that reaches into `unisim` needs the library compiled once per
machine -- ghdl ships the flow
(`scripts/vendors/compile-xilinx-vivado.sh --unisim --vhdl93` over the
Vivado installation's `data/vhdl/src/unisims` tree; the GRLIB-class
wrappers also reach the ISE-era primitives Vivado's modern tree dropped,
so the source tree merges the modern files with the ISE set from
`ids_lite/ISE/vhdl/src/unisims`). The compile lands as
`<output>/unisim/v93/unisim-obj93.cf`; ghdl 6's loader searches
`<prefix>/<lib>/v93/<lib>-obj<STD>.cf`, so the output dir rides
`ORBIT_DRIVES_GHDL_VENDOR_PREFIX` verbatim when its layout matches
(`<prefix>/unisim/v93/unisim-obj93.cf`, with ghdl's own `std`/`ieee` and
runtime files symlinked beside it when the prefix replaces ghdl's
default). The `--vhdl93` standard matters: the design rows analyze at
ghdl's 93 default, and a same-std library file avoids the cross-std
suffix mismatch.
