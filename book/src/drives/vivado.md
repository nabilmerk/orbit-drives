# vivado drives


Two target entries (`vivado`, `vivado-build`) share one drive on the
richest probed process in this crate. The shapes the probes wrote down:

### The launch recipe

Every vivado-family spawn needs the recipe env, and the drive sets all
three legs on every spawn itself (`Command::env`, scoped to the spawn --
a process-wide `LD_LIBRARY_PATH` poisons other tools' backends, probed):
the ghdl LLVM backend fails on Vivado's bundled libstdc++.

- **`LC_ALL`** -- the launcher aborts without a UTF-8 locale it can name.
  The environment's own `LC_ALL` wins; the probed `en_US.UTF-8` default
  rides otherwise.
- **`LOCPATH`** -- where that locale lives when it is not
  system-installed. The environment's own wins; `$HOME/.local/locale`
  (the no-root `localedef` recipe) otherwise. Omitted when neither
  exists.
- **`LD_LIBRARY_PATH`** -- the SuSE ncurses5 leg the main wrapper's full
  init needs. The environment's own wins; the leg derived from the
  resolved install (`<prefix>/lib/lnx64.o/SuSE`) otherwise. Omitted when
  neither exists.

The binaries resolve from `VIVADO_BIN` (the install path, e.g.
`/opt/2025.2.1/Vivado/bin/vivado`); the stage binaries (xvhdl, xvlog,
xelab, xsim) ride beside it as siblings of the resolved path, and a bare
PATH name falls back to the PATH lookup for the whole family. Every
family binary answers `-version` (single dash) with exit 0 -- the
spawnability probe.

### The compile

- **Both languages ride.** vivado is the one carried tool that speaks
  both: `VHDL` entries compile through xvhdl, `VLOG` and `SYSV` entries
  through xvlog, a custom fileset is the loud not-carried face (exit 2).
- **xvhdl is order-sensitive within the work library**
  (`ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'`,
  exit 1, probed live on 2025.2.1), so the VHDL files compile ONE
  INVOCATION PER FILE in blueprint order through the deferral loop (the
  ghdl-precedent machinery, borrowed a third time): a failing file
  defers to a retry after the others; a pass that makes no progress
  stops the loop with every residual log relayed.
- **xvlog is order-free** (a consumer-before-provider pair compiled
  clean, probed live), so the Verilog files compile in ONE invocation
  into `work`.
- **The compile gates on the exit and is never judged.** Every probed
  compile failure exits 1, and the faces are UPPERCASE `ERROR:` on
  STDOUT -- stderr is empty on every stage-binary drive, error or clean
  (probed) -- deliberately outside the vocabulary below: case-exactness
  keeps the compile faces out of the judge, and the honest exit makes
  them unnecessary there.

### The libraries (the single-work sim face, the per-library build face)

The two faces split the blueprint's library column, per the owner's
ruling (2026-11-08) -- the probed xsim binding rule drove it:

- **The SIM face rides SINGLE-WORK.** Every file -- VHDL and Verilog
  alike -- compiles into `work` (`xvhdl --work=work <file>`, the
  explicit pin; `xvlog -work work` unchanged), and xelab rides the BARE
  top (`xelab -L work <top> -s <top>` -- a bare design unit is
  implicitly `work.`-qualified, probed: for a top compiled into a named
  library the refusal reads `ERROR: [XSIM 43-3225] Cannot find design
  unit work.tb in library work`, xelab's own error naming the implicit
  prefix). WHY: xsim's default component binding is LRM-strict --
  a component instance with no binding indication searches the library
  the COMPONENT DECLARATION lives in, and nothing else -- and it is
  argv-incurable (probed exhaustively on 2025.2.1, 2026-11-08: the
  `-L` set does not extend it -- xelab's own help scopes the `-L` search
  face to "a Verilog or Mixed language design" -- `-relax` does not, no
  `-L` order does, no binding option exists in xelab's help at all, and
  even the mapped-library face below does not lift it). GRLIB-style
  designs rely on cross-library default binding, and GRLIB's own xsim
  flows keep ONE physical library for exactly this reason; the
  single-work merge co-locates every component declaration with every
  entity, and the drive's two-library live fixture passes through it.
- **The mapped-library face: the drive writes the cwd `xsim.ini`.**
  Merging the compile would otherwise strand the sources' named
  `library grlib; use grlib.grlib_pkg.all;` clauses (probed: the same
  source compiles with the mapping and dies on the dangling clause
  without it). So when the blueprint names libraries beyond `work`, the
  drive writes `xsim.ini` -- one `grlib=xsim.dir/work` line per distinct
  blueprint library, the GRLIB `vmap` convention (many logical names,
  one physical library). Probed live: with the mapping the clauses
  resolve; the tool's own std/ieee defaults stay intact beside the
  partial ini; `work` never maps to itself; a single-`work` blueprint
  writes no file at all. Like the quartus sim face's `modelsim.ini`,
  this file is the drive's state: the stale one dies with the wipe and
  a fresh one is written per drive. HONEST LIMIT (probed): the mapping
  resolves CLAUSES, not default bindings -- a component reached through
  a mapped alias still black-boxes on the 10-4940 face; designs that
  need that face ride explicit `entity <lib>.<name>` instantiations
  written against the merged library (`entity work.gate`), and the
  failure is never silent when they do not.
- **The BUILD face keeps the library column.** The generated script
  reads each VHDL file back out of its own row's library
  (`read_vhdl -library <entry.library> <file>` -- the GRLIB synth flow's
  own shape, probed live on the GRLIB flow): synthesis binds
  cross-library fine (probed through the drive's two-library synth --
  the netlists land). The per-library reads and the single-work sim
  compile coexist because the script re-reads the SOURCES: the compile
  is the build face's pre-flight gate, not the synth design's input.
- **xvlog carries no library column at any face.** Verilog speaks no
  library clauses: every `VLOG`/`SYSV` file lands in `work`, whatever
  its row declares, and the script's `read_verilog` lines stay plain.
  Documented, not consumed.
- **The state wipe covers both faces.** The drive wipes the whole
  `xsim.dir/` before every drive -- the merged work library and the
  snapshots die together, so no stale unit stays resolvable -- and the
  stale `xsim.ini` dies with it (a library a previous blueprint mapped
  cannot leak into this one). Probed honestly: the wipes are otherwise
  symmetry -- a failed compile or elaboration exits before anything
  stale could be mistaken for fresh.
- **No std flag is pinned** (unchanged): the tool default compiles
  GRLIB-scale designs green; `--2008` breaks them (the `10-3083`
  homograph walls, probed live).
- **The argv shapes, dated honestly.** (2026-11-07, one day-lived): the
  compile and xelab briefly rode per-library `--work=<entry.library>`
  pins and a `<the top's library>.<top>` design unit -- REFUTED as a
  binding cure and superseded by the single-work ruling the next day.
  (2026-11-08, the ruling): the sim compile reverts to the explicit
  `xvhdl --work=work f.vhd`, the xelab argv drops the qualification
  (`xelab -L work <top> -s <top>` -- the bare top being work-qualified
  is exactly what the single-work face wants), the script keeps
  `read_vhdl -library <entry.library> f.vhd`, and the mapped `xsim.ini`
  joins the drive's state. Nothing about the judged faces moved with
  it; the probed sequences below carry the current argv verbatim, and
  the live tests drive both faces end to end.

### The build face (`vivado-build`)

- **`--part` is required BEFORE any drive** (exit 2 at the CLI naming
  the flag): the probed alternative is a ~4s synth-then-fail inside the
  tool. The sim face consumes no part -- the flag on any other face is a
  loud parse error, never a dropped argument.
- **The synthesis script is GENERATED** per top, written to
  `orbit-synth-<top>.tcl`: one `read_vhdl -library <entry.library>` /
  `read_verilog` per file (the library face is the library column's,
  above; the batch `read_*` commands take one file per command, and
  read order is irrelevant -- probed on 2025.2.1), then `synth_design
  -top <top> -part <part> -mode out_of_context`, then the three netlist
  writers into `netlists/` (`write_checkpoint -force` .dcp,
  `write_verilog -force` .v, `write_edif -force` .edf).
- **The catch-guard is the probed trap.** The whole body sits inside ONE
  guard that exits 1 -- and the guard tests `catch`'s RETURN CODE, never
  the caught string: Tcl's `catch {script} var` writes the script's
  NORMAL RETURN VALUE into `var`, and the netlist writers return the
  written filename, so an emptiness guard FALSE-FAILS A CLEAN SYNTH
  (probed live: exit 1 with all three netlists written). The guard also
  makes the batch fail-fast -- unguarded batch mode continues past a
  failed `synth_design` (probed on 2025.2.1), and a guardless catch
  would write nothing after the failure.
- **The batch exit is honest.** Exit 0 clean with the payload landed,
  exit 1 fail-fast through the guard. The drive never judges the batch
  log.

### The sim face (`vivado`)

- **xelab rides the bare top over `work`**
  (`xelab -L work <top> -s <top>`, the single-work face's argv above --
  a bare design unit is implicitly `work.`-qualified, and everything IS
  in `work`); the snapshot lands as `xsim.dir/<top>` and the
  elaboration validates the top (an unknown top answers
  `ERROR: [XSIM 43-3225] Cannot find design unit ...`, exit 1,
  probed).
- **xsim runs each snapshot once** (`xsim <top> -R`), and THE RUN EXIT
  IS A LIE: xsim exits 0 on a violated `assert ... severity failure`
  AND on a violated `severity error` (both probed live -- the face
  prints, the simulation continues or stops, and the process exits 0).
  The log is the only truth, and the judge is the load-bearing gate --
  the only drive in this crate where the exit cannot convict a probed
  failure. The exit stays the crash-door witness: a missing snapshot
  exits 1 with an uppercase face the vocabulary does not carry.
- **The vocabulary is line-anchored, the only PROBED one in this
  crate.** The faces are `Fatal:` / `Error:` / `Failure:` at LINE START,
  case-exact: a case drift (`FATAL:`) and a marker mid-line (a
  testbench's own report note, an echoed source line) never judge.
  `Warning:` and `Note:` lines never judge.

