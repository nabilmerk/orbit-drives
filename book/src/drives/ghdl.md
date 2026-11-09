# ghdl drive


Two target entries (`ghdl`, `ghdl-build`) share one drive, split by how
far it goes: the build face stops after step 4, the sim face runs on
through the judge. The drive consumes the blueprint's library column (the
§25 multi-library face, probed live on ghdl 6.0.0, 2026-11-07) -- the
same face the nvc drive carries, on ghdl's own resolution:

1. Read the blueprint; every non-VHDL entry (`VLOG`, `SYSV`, a custom
   fileset) is a loud exit-2, never a silent skip.
2. Wipe the stale library state (every `<library>-obj<STD>.cf` in the
   output directory -- the whole FAMILY, every std tag): ghdl's library
   file is its clock -- one file per analyzed library, tagged with the
   standard it was analyzed under (`-obj93` under the tool default the
   drive rides, `-obj08` under a pinned 08) and LOWERCASED by ghdl
   (`--work=MyLib` writes `mylib-obj93.cf`, probed). A second run in
   orbit's persistent output directory would otherwise obsolete its own
   design out from under `ghdl -e`; and a library a previous blueprint
   analyzed but this one no longer names must die too, or a
   `library <other>;` clause would silently resolve against its stale
   units -- the scan, not the blueprint's library list, is what makes the
   wipe honest. Correctness before incrementality in v1 -- every drive
   starts from empty libraries.
3. Analyze the VHDL files in blueprint order with the **deferral loop**,
   each file into the library its own blueprint row declares
   (`ghdl -a --work=<entry.library> <file>` -- NO std flag: the drive
   rides ghdl's own default, see the argv bullet below): ghdl is
   order-sensitive, so a failing analyze defers its file to a retry after
   the others; a pass that makes no progress stops the loop with every
   residual stderr surfaced verbatim. Cross-library references defer
   across libraries exactly as they defer within one: pass 1's
   `cannot find resource library "<other>"` face is the absent
   `<other>-obj93.cf` file, and the retry resolves once the provider's
   row has written it.
4. Elaborate each top (`ghdl -e --work=<the top's library> <top>
   <extras>` on the build face; the extras ride the elab argv there
   because there is no run to carry them). A nonzero elaboration is a
   finding, exit 1.
5. (`ghdl` only) Run each top (`ghdl -r --work=<the top's
   library> <top> --assert-level=error <extra args>`; the assert level is
   a simulation option, legal only after the toplevel unit) and capture
   the log.
6. (`ghdl` only) Gate the run: a probed failure face in the log OR a
   nonzero ghdl exit is a finding. Either witness convicts.

The multi-library facts the probes wrote down:

- **No search-path flag exists or is needed.** ghdl resolves a
  consumer's `library <other>;` clause against the sibling
  `<other>-obj93.cf` file in the SAME working directory, automatically --
  `ghdl -a --work=work gate.vhd` analyzing clean against
  `grlib-obj93.cf` with no `-P`/`-L` anywhere (probed; move the sibling
  into a subdirectory and the resolution dies: ghdl's library search is
  the cwd alone). Orbit already spawns the drive with the output
  directory as the cwd, where every row's file lands.
- **The elaborate and run argv pin the top's library** (`--work=<lib>`),
  because ghdl's bare-top commands search the WORK library only (`-e tb`
  with the top analyzed into any other library: `error: cannot find
  entity or configuration tb`, exit 1, probed). The resolution, loudest
  voice first: `$ORBIT_PROJECT_LIBRARY` (orbit sets it on every spawned
  target to the working project's library; the designated testbench is a
  working-project file); then the blueprint's single carried library
  (direct invocation without orbit); then ghdl's default `work` -- a
  guess that cannot pass silently, because the wiped `work` library
  cannot hold a stale unit: a wrong guess dies loudly on ghdl's own
  elaboration refusal.
- **A library no blueprint row populates fails loudly at analyze.** A
  design naming a THIRD library (not one the planner carried) dies on
  `cannot find resource library "<other>"` / `unit "<unit>" not found in
  library "<other>"`, the deferral loop exhausts with no progress, and
  the residual stderr relays the faces verbatim, exit 1 -- never a silent
  blackbox degradation. Every library the blueprint carries is analyzed,
  so this face cannot fire for one.
- **The argv shape changed twice.** (2026-11-07, the multi-library
  face): every project's analyze/elab/run carries `--work=<library>` --
  a depless single-library project's analyze went from
  `ghdl -a f.vhd` to `ghdl -a --work=work f.vhd`, its elaboration and
  run likewise. (2026-11-08, the std face): the `--std=08` pin DROPPED
  from every argv -- the drive rides ghdl's own default (VHDL-93 on
  ghdl 6.0.0, probed). WHY: the pin walled 93-written corpora -- the
  zedboard GRLIB graph died on `redeclaration of procedure "HRead"`
  (stdio.vhd's legal-in-93 overloads homograph under 08), probed live:
  the same file compiles clean under the default and `--std=93`, and
  GRLIB's own ghdl flow passes no std flag at all. Nothing about the
  judged faces moved with it; the probed sequences below carry the
  current argv verbatim, and the ghdl live tests drive the shape end to
  end.
- **VHDL only**, like every VHDL drive: the `VLOG`/`SYSV` filesets are
  the loud exit-2 refusal, and ghdl's Verilog face is not carried.

