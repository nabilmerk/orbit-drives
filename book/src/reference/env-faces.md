# The env faces

The drives carry interim env faces for corpus facts nothing else in the
stack expresses. All are unset-by-default; all ride verbatim.

| env | rides | default |
|---|---|---|
| `ORBIT_DRIVES_NVC_STD` | `--std=<value>` on every nvc spawn | nvc's own default |
| `ORBIT_DRIVES_NVC_HEAP` | `-H <size>` on every nvc spawn | nvc's default heap |
| `ORBIT_DRIVES_GHDL_FLAGS` | whitespace-split flags on every ghdl spawn | none |
| `ORBIT_DRIVES_GHDL_ASSERT_LEVEL` | `--assert-level=<value>` in the run argv | `error` |
| `ORBIT_PROJECT_LIBRARY` | set by orbit; the drive reads it | — |

## The probed provenance

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
