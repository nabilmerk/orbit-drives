# nvc drive


Two target entries (`nvc`, `nvc-build`) share one drive with the same
shapes as ghdl's -- blueprint, deferral-loop analysis, elaboration per
top, judged run -- on nvc's probed process and faces, consuming the
blueprint's library column (the §25 multi-library face, probed live on
nvc 1.23.0, 2026-11-07). The differences the probes wrote down:

- **The argv is option-first, the opposite of ghdl.** `--std=2008`,
  `-L .`, and `--work=<library>` are GLOBAL options and must LEAD:
  `nvc --std=2008 -L . --work=grlib -a f.vhd`. After the command, nvc
  answers `** Fatal: unrecognised analysis option --std=2008` (exit 1).
  Run options are legal after the toplevel unit (`nvc --std=2008 -L .
  --work=work -r top --stop-time=1ns`, probed clean), so the extras ride
  there.
- **Multi-library: the library column is load-bearing.** Every analyze
  rides `--work=<entry.library>` -- the blueprint's library column
  consumed verbatim (the §25 ruling: the library name is the project's
  declared name, orbit's planner is its source of truth -- and orbit
  defaults a project's library to the project's NAME, not to any tool's
  `work`). nvc creates one directory per analyzed library in the output
  directory (`grlib/`, `work/`, ...), and EVERY nvc spawn -- analyze,
  elaborate, run -- carries `-L .` so a consumer's `library <other>;`
  clause resolves against them: without it the search path is nvc's
  install dirs alone, and the analyze dies on `** Error: library
  <OTHER> not found`. Libraries analyze in ANY blueprint order -- the
  search path makes cross-library references resolve once both sides are
  analyzed, which is what the deferral loop (unchanged, per-file) rides:
  a consumer defers across libraries exactly as it defers within one.
- **The elaborate and run argv pin the top's library** (`--work=<lib>`),
  because nvc's bare-top commands search the WORK library ONLY (`-e tb`
  with the top analyzed into any other library: `** Fatal: cannot find
  unit WORK.TB in library WORK`, exit 1, probed -- even with `-L .`).
  The resolution, loudest voice first: `$ORBIT_PROJECT_LIBRARY` (orbit
  sets it on every spawned target to the working project's library; the
  designated testbench is a working-project file); then the blueprint's
  single carried library (direct invocation without orbit); then nvc's
  default `work` -- a guess that cannot pass silently, because the wiped
  `work` library cannot hold a stale unit: a wrong guess dies loudly on
  nvc's own elaboration refusal.
- **The library state is per-library directories, and the wipe takes
  every one of them**: the drive removes each directory in the output
  directory carrying nvc's own `_NVC_LIB` marker (nvc writes it even
  when the analysis into the fresh library fails, probed -- it is nvc's
  own library-recognition rule). The scan, not the blueprint's library
  list, is what makes it honest: a library a PREVIOUS drive analyzed but
  this blueprint no longer names must die too, or a `library <other>;`
  clause would silently resolve against its stale units. Unlike ghdl's,
  the wipe was never load-bearing against nvc itself (nvc tolerates
  stale-library re-analysis, probed) -- with libraries in the blueprint
  it IS load-bearing against the drive's own resolution.
- **A library no blueprint row populates fails loudly at analyze.** A
  design naming a THIRD library (not one the planner carried) dies on
  `** Error: library <OTHER> not found` / `** Fatal: required library
  <OTHER> not found`, the deferral loop exhausts with no progress, and
  the residual stderr relays both faces verbatim, exit 1 -- never a
  silent blackbox degradation. Every library the blueprint carries is
  analyzed, so this face cannot fire for one.
- **No `--assert-level` analogue is pinned.** None is needed: nvc's run
  exits probed honest on every failure face (below), and the judge gates
  the log regardless.
- **The argv shape changed with the multi-library face** (2026-11-07):
  every project's argv now carries `-L .` and `--work=<library>` -- a
  depless single-library project's analyze went from
  `nvc --std=2008 -a f.vhd` to `nvc --std=2008 -L . --work=work -a
  f.vhd`, its elaboration from `nvc --std=2008 -e top` to `nvc
  --std=2008 -L . --work=work -e top`, its run likewise. Nothing about
  the judged faces moved with it; the probed sequences below carry the
  new argv verbatim, and the single-library live tests drive the new
  shape end to end.
- **VHDL only**, like every VHDL drive: the `VLOG`/`SYSV` filesets are
  the loud exit-2 refusal, and nvc's Verilog face is not carried.

