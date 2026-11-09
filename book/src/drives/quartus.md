# quartus drives


Two target entries (`quartus`, `quartus-build`) share one drive -- the
DOCUMENTED-ONLY drive of this crate: quartus is not installed on the
probing box, so nothing here is probed, every CLI face is the documented
hypothesis, and every hypothesis carries its first-contact rule in the
honest limits. On the first quartus box, the owner runs the gated shape
tests (`cargo test -- --ignored` with `QUARTUS_BIN` set) and rewrites
the vocabulary and these rules from the live logs.

### The resolution

The binaries resolve from `QUARTUS_BIN` (the quartus_map path), the
bundled Questa deriving as the `questa_fse/bin` SIBLING of the quartus
install (`<install>/quartus/bin/quartus_map` derives
`<install>/quartus/questa_fse/bin/{vlib,vmap,vcom,vsim}` -- the
documented derive, UNPROBED); a bare PATH name falls back to the PATH
lookup for the whole family.

### The build face (`quartus-build`)

- **`--part` is required BEFORE any drive** (exit 2 at the CLI naming
  the flag): the DEVICE assignment names the fabric the map targets.
  The sim face consumes no part -- the flag on any other face is a loud
  parse error, never a dropped argument.
- **The project settings are GENERATED per top**, written to
  `<top>.qsf`: `set_global_assignment -name DEVICE <part>`,
  `set_global_assignment -name TOP_LEVEL_ENTITY <top>`, and one
  `VHDL_FILE` / `VERILOG_FILE` line per blueprint file (both languages
  ride the build face; a custom fileset is the loud not-carried face).
  HYPOTHESIS: that DEVICE + TOP_LEVEL_ENTITY + the file list suffice for
  a minimal `.qsf` is the first-contact question.
- **The batch argv is the documented one**:
  `quartus_map --read_settings_files=off --write_settings_files=off
  <top> -c <top>` (project = top, revision = top). HYPOTHESIS: the
  read/write-settings flags' exact semantics are unprobed.
- **The exit is hypothesized honest** (nonzero on failure, UNPROBED):
  the build face gates on the exit and never judges the log.

### The sim face (`quartus`)

- **The VHDL fileset only.** The documented recipe is vcom's alone:
  `VLOG` / `SYSV` entries are the loud not-carried face here (exit 2),
  though they ride the build face's `VERILOG_FILE` lines.
- **The library opens documented**: `vlib work`, then `vmap work work`
  (the ModelSim workflow's opening pair, UNPROBED); the drive wipes
  `work/` and `modelsim.ini` before every sim drive -- the
  correctness-before-incrementality stance, DOCUMENTED-ONLY.
- **vcom compiles in blueprint order through the deferral loop** (the
  ghdl-precedent machinery, borrowed a fourth time): the
  order-sensitivity hypothesis of the stateful ModelSim library, so a
  failing file defers to a retry after the others. The standard is
  pinned (`vcom -2008 -work work`, vcom's spelling of the crate-wide
  VHDL-2008 pin; `-93` / `-2002` are the other spellings and cannot be
  selected from the argv yet).
- **vsim runs each top once**:
  `vsim -c -do "run -all; quit" -L work work.<top>`, the `-L work` pin
  before the top -- the documented order, UNPROBED. The extras ride
  this argv verbatim.
- **The gate is double-keyed on hypotheses.** The exit honesty (vsim
  honest) and the vocabulary (line-start `Fatal:` / `Error:` /
  `Failure:`, the ModelSim-family rendering riding vivado's probed
  anchor) are BOTH unprobed -- so neither witness is trusted alone and
  either convicts, the same as everywhere in this crate. The
  vocabulary's negative pins are hypothesized from vivado's probed
  ones: a case drift (`FATAL:`) and a marker mid-line never judge.

