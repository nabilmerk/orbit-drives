# Honest limits


- **The filesets are per tool -- and the shipped targets ride ALLHDL.**
  The VHDL drives (`ghdl`, `nvc`, `cocotb`) carry the `VHDL` fileset
  only; the Verilog drives (`iverilog`, `verilator`, `yosys`) carry
  `VLOG` and `SYSV` (their native filesets); the vivado drives and the
  quartus-build face carry all three -- vivado is the one PROBED tool
  that speaks both languages, quartus-build the documented-only second;
  the quartus sim face carries the `VHDL` fileset only (the documented
  recipe is vcom's alone). Every face additionally routes the custom
  `ALLHDL` fileset BY EXTENSION (`.vhd`/`.vhdl` to the VHDL faces,
  `.v`/`.sv` to the Verilog faces): the shipped `config.toml` target
  entries all carry `fileset.ALLHDL = { patterns = ["**/*.vhd",
  "**/*.vhdl", "**/*.v", "**/*.sv"], recursive = true }` -- the FULL HDL
  file set of every project in the dependency graph riding every
  blueprint REGARDLESS of orbit's reference closure (the owner's
  2026-11-08 ruling). WHY: orbit's blueprint is the top's static
  reference closure, and a DEFAULT component binding (a component
  declaration with no static entity reference -- the GRLIB style) is
  invisible to that walk -- the zedboard demo's multiplier entity
  (techmap/rtl/inferred/mul_inferred.vhd, bound only through mul32's
  component) dropped from the blueprint and xsim black-boxed it (the
  `10-4940` face, the systest's error(2); root-caused at the directory
  level 2026-11-08). The custom-fileset face bypasses the closure
  (probed live on orbit: a dependency project's unreferenced source
  rides the blueprint under ALLHDL where the bare planning drops it),
  and its probed semantics are why the drives route it: a custom
  fileset REPLACES the built-in VHDL/VLOG/SYSV collection in the
  blueprint -- every row rides as ALLHDL -- so a face outside its
  extension is the loud exit-2, never a silent skip, and everything
  else compiles exactly as before.
- **Eight tool names, eight vocabularies: ghdl, nvc, iverilog,
  verilator, yosys, vivado, quartus, cocotb.** Any other tool name is a
  loud error at the CLI, never a guess. Seven vocabularies are probed;
  the eighth (quartus) is the documented-only ModelSim-family
  hypothesis (below).
- **quartus is DOCUMENTED-ONLY -- every sentence about it is a
  hypothesis carrying its first-contact rule.** quartus is not
  installed on the probing box, so none of its faces are probed. The
  hypotheses and their first-contact rules, owner's call on the first
  quartus box (run `cargo test -- --ignored` with `QUARTUS_BIN` set --
  the gated shape tests are the harness):
  - **DEVICE-only sufficiency of the generated `.qsf`** (DEVICE +
    TOP_LEVEL_ENTITY + the file lines): if `quartus_map` refuses the
    minimal file, extend `qsf_script` from the tool's own complaint --
    never invent assignments speculatively.
  - **the Questa sibling derive** (`questa_fse/bin` beside the quartus
    install): if the first install names it differently (a versioned
    or differently-named directory), resolve from the actual layout and
    update `questa_bin`.
  - **vsim's exit honesty** (nonzero on a failing sim): probe with a
    severity-failure testbench BEFORE trusting the exit witness alone;
    if vsim lies the way xsim lies, the vocabulary becomes the
    load-bearing gate and the Vivado drive page records it.
  - **the vocabulary itself** (line-start `Fatal:` / `Error:` /
    `Failure:`, ModelSim-family): probe the real failing faces (severity
    failure, severity error, an elaboration refusal) and rewrite
    `QUARTUS_VOCABULARY` from the live logs -- spellings, anchor, and
    negative pins alike.
  - **the vcom default standard** (`-2008` accepted): probe `vcom
    -2008` on a real file; if the bundled Questa rejects the flag
    spelling, the pin moves to the accepted spelling and the honest
    limit is rewritten.
  - **the `-L`-before-top order on the vsim argv** (`vsim -c -do "run
    -all; quit" -L work work.<top>`): probe the documented argv; if
    vsim wants the `-L` pins after the design (or extra `-L` pins per
    dependency library), the argv shape moves and the Vivado drive page
    records it.
  - **the read/write-settings flags** (`--read_settings_files=off
    --write_settings_files=off`): probe whether the generated `.qsf` is
    actually read under them; if not, the invocation moves to the
    variant that reads it.
  - **the build face's state** (quartus_map's compilation database): a
    stale database could make a re-drive incremental; the first probed
    re-drive decides whether the wipe list grows.
- **The library column is load-bearing on the VHDL drives and on
  vivado's BUILD face; carried on the rest.** The nvc drive consumes the
  blueprint's library column verbatim -- every file analyzed into
  `--work=<entry.library>` (the §25 multi-library face, probed live on
  nvc 1.23.0, 2026-11-07), every spawn carrying `-L .`, the wipe taking
  every `_NVC_LIB`-marked directory, and the elaborate/run argv pinning
  the top's library (resolved from `$ORBIT_PROJECT_LIBRARY`, then the
  blueprint's single carried library, then nvc's default `work`). The
  ghdl drive consumes the same column on its own resolution (probed
  live on ghdl 6.0.0, the same day): per-file `--work=`, the
  cwd-sibling `-obj<STD>.cf` files doing the resolution no search flag
  needed, the wipe taking the whole `<library>-obj<STD>.cf` family, and the same
  three-voice ladder on the elaborate/run argv. The vivado drive's
  column rides where its engines serve it (the owner's 2026-11-08
  ruling): the BUILD face's generated script reads each VHDL file back
  out of its own row's library (`read_vhdl -library <entry.library>` --
  synthesis binds cross-library fine, probed through the drive's
  two-library synth), while the SIM face is SINGLE-WORK -- xsim's
  default component binding is LRM-strict (it searches the component
  declaration's own library alone) and argv-incurable (probed
  exhaustively: no `-L`, `-relax`, order, or mapping lifts it), so the
  ruling merges every file into one `work` library with the mapped
  `xsim.ini` carrying the blueprint's named libraries for clause
  resolution -- the same single-physical-library convention GRLIB's own
  xsim flows use. The Verilog legs stay single-library at every face
  (`xvlog -work work`, `read_verilog` plain: Verilog speaks no library
  clauses, documented). One wall rides with the vivado sim face (probed
  2026-11-08): the merge resolves CLAUSES, not default component
  bindings -- a component reached through a mapped alias still
  black-boxes at elaboration (the 10-4940 face, exit 1); designs that
  need cross-library default components ride explicit `entity <lib>.<name>`
  instantiations written against the merged library. Every OTHER
  drive still maps nothing: iverilog compiles into its one snapshot,
  verilator lints and builds one fileset, and the quartus faces compile
  their filesets as given (`VHDL_FILE` / `VERILOG_FILE` lines, no
  library) -- a multi-library orbit project on quartus stays out of
  scope for v1.
- **The standards are pinned where the tools pin them** (`--std=2008`
  for nvc, `-g2012` for iverilog, `-2008` for the
  quartus sim face's vcom -- vcom's spelling of the same VHDL-2008 pin;
  verilator's pinned flags are `--timing` and the face flags -- its
  language setting stays verilator's default in v1; cocotb's backing
  simulator compiles under ghdl's runner default, no std pin in v1;
  ghdl pins NONE -- the drive rides ghdl's own default, VHDL-93 on ghdl
  6.0.0, since the 2026-11-08 std face: see the ghdl drive) and
  cannot be overridden from the argv yet; the extras reach the drive's
  last step only (`-r` on the VHDL sim faces, `vvp` on iverilog's, the
  model argv on verilator's, the `yosys -p` synth argv on yosys's, the
  runner's `test_args` on cocotb's; the elaboration / compile / build
  argv on the build faces).
- **The state is run-scoped, not incremental.** Every drive wipes its
  clock first (nvc's: every `_NVC_LIB`-marked directory in the output
  directory -- the whole per-library set, not a named one; ghdl's: every
  `<library>-obj08.cf` file in the output directory -- the same
  whole-per-library set, the name lowercased by ghdl; iverilog's
  snapshot; verilator's
  `obj_dir`; yosys's `netlists/`; vivado's `xsim.dir/` -- the WHOLE
  state directory, the merged work library and the snapshots together,
  plus the `xsim.ini` mapped-library file (regenerated per drive, so a
  library a previous blueprint mapped cannot leak into this one) -- and,
  on the build face, its
  `netlists/`; the quartus sim face's `work/` +
  `modelsim.ini`, DOCUMENTED-ONLY; cocotb's `sim_build/`). A large
  design pays a full re-analysis per drive; correctness before
  incrementality is the v1 stance. (Probed honestly: ghdl's wipe is
  load-bearing -- a stale library obsoletes its own design -- and with
  libraries in the blueprint it is load-bearing against the drive's own
  resolution too, where a library the blueprint no longer analyzes must
  not stay resolvable; nvc's
  is load-bearing on a multi-library blueprint for the same reason, even
  though
  nvc itself tolerates re-analysis in a stale library; iverilog's
  snapshot cannot be run past a
  failed compile, verilator's `--build` regenerates its intermediates, a
  failed yosys synth exits before its netlist could be mistaken for
  fresh, a failed vivado compile or elaboration exits before the sim
  face could run a stale snapshot, and cocotb's runner unlinks the
  results file before each test and always rebuilds -- so those wipes
  are symmetry.)
- **Bare ghdl and bare vvp have holes; bare xsim is worse; the cocotb
  runner lies by omission; the shipped paths catch them.** A
  `report ... severity error` statement slips a bare `ghdl -r` (face
  `(report error)`, exit 0, no judged face): the drive pins
  `--assert-level=error`, which stops the run and writes the
  `./<top>:error: report failed` face. A `$error` call slips a bare vvp
  (face `ERROR: ...`, simulation continued, **exit 0**): there the
  judge's face gate is the only witness, and it is load-bearing. xsim is
  the extreme -- BOTH probed runtime failures exit 0 (a severity failure
  and a severity error) -- so the vivado drive's judge is the
  load-bearing gate on every run, and the line-start vocabulary is the
  only witness on them. The cocotb runner's lie is by omission: `ghdl -r`
  exits 0 on a FAILED test (the finding lands only in results.xml), so
  the generated script reads the results and exits honest -- the
  load-bearing exit there. nvc, verilator, and yosys have no such hole
  probed: nvc's severity-error runs continue the simulation but still
  exit 1, every verilator model failure dies by SIGABRT -- no exit code
  at all -- and every probed yosys failure exits 1, so their exits
  convict.
- **A face shape mid-text judges fail -- except on vivado (and
  quartus, riding vivado's anchor on the documented hypothesis).** The
  substring vocabularies cannot tell your prose from the tool's: a
  testbench's own report note that embeds a ghdl face shape
  (`bus:error: retrying`), an nvc face shape (`** Error: retrying`), a
  vvp face shape (`ERROR: retrying`), a verilator face shape
  (`%Error: retrying`), or a yosys face shape (`ERROR: retrying`)
  convicts the run. vivado is the deliberate probed exception: its
  faces are line-anchored, so only a line LEADING with the marker
  convicts and mid-text prose never does. cocotb's ` FAIL ` row face is
  a substring face with one guard: the space padding keeps the
  summary's own `FAIL=0` counter (and bare `FAIL` prose) from judging,
  but a TEST NAME carrying the space-padded shape (`tb FAIL demo`)
  would still convict a clean run -- keep test names free of face
  shapes. Keep report notes and `$display` text free of face shapes
  anyway.
- **Verilator builds need a C++ toolchain.** `--binary --build` shells
  out to `make` and the system C++ compiler (probed on this box with
  both on PATH); a box without them fails the build step loudly,
  surfaced as an exit-1 build failure with the toolchain's own output
  relayed.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends.
- **The tools resolve on PATH, vivado and quartus from their env
  hooks.** The orbit target contract carries no binary-path field: every
  drive but those two pins the bare name; vivado resolves from
  `$VIVADO_BIN` (the install path) with the bare name on PATH as the
  fallback, and its stage binaries ride beside the resolved path (or on
  PATH); quartus resolves from `$QUARTUS_BIN` the same way, its bundled
  Questa deriving as the `questa_fse/bin` sibling (DOCUMENTED-ONLY).
  The cocotb drive pins `python3` and needs cocotb (>= 2.0: the
  `cocotb_tools` runner package -- a 1.x install is a loud usage error
  at the identity probe) and the backing simulator (ghdl) on PATH; the
  runner resolves the simulator itself and names it loudly when absent.
  The vivado recipe env rides the drive's own spawn scoping -- see the
  launch recipe.
- **Blueprint paths with spaces are a v1 limit on the script faces.**
  yosys's `-p` script and vivado's generated synth script carry the file
  paths verbatim, split on whitespace by the tool's own parser: a path
  with a space would split and fail the read loudly.
- **The run log is captured, then relayed**, not streamed; a long
  simulation's output appears when the run ends. (yosys's synthesis log
  relays only on failure -- on success the drive reports the netlist,
  not kilobytes of synthesis log.)
- **License.** The Cargo.toml declares `MIT OR Apache-2.0`; the LICENSE
  texts land before any `cargo publish` -- never publish without them
  (the maintainer conversation's standing rule). Orbit itself is
  GPL-3 -- irrelevant here: a target is a spawned process, not a derived
  work of orbit.

