# The probed facts


Every pattern in a vocabulary and every drive fact below was probed live
on this box, 2026-11-07, on assert- and report-driven testbenches, both
bare and through the drive. Each row carries the argv that produced the
face.

### ghdl 6.0.0

| face / fact | argv that wrote it | what ghdl did |
|------|-----|------|
| `(assertion failure)`, `./<top>:error: assertion failed`, `./<top>:error: simulation failed` | `ghdl -r tb_fail` (same through the drive, which adds `--assert-level=error` after the top) | a violated `assert ... severity failure` stops the run, exit 1 |
| `(assertion error)` | bare `ghdl -r tb_err` | a violated `assert ... severity error`: prints the face and **continues, exit 0** -- bare ghdl's hole. The drive pins `--assert-level=error`, which stops the run (exit 1), and the judge reads the face either way |
| `(assertion warning)`, `(assertion note)` | `ghdl -r tb_warn` | the gentler severities on a continuing run -- continuations, **never judged** |
| `(report failure)`, `./<top>:error: report failed` | `report ... severity failure;` statement, `ghdl -r tb_rep_fail` | a report statement carrying `severity failure` stops the run, exit 1 -- caught by `:error:` |
| `(report error)` | `report ... severity error;` statement: bare `ghdl -r tb_rep_err` | bare ghdl prints and continues, exit 0, **no judged face** (bare-ghdl-only, see limits). Through the drive, `--assert-level=error` stops the run with the `./<top>:error: report failed` face, which judges |
| `ghdl:error: unknown command option '--assert-level=error'` | `ghdl -r --assert-level=error tb` | the assert level is a SIMULATION option: after the toplevel unit only |
| `ghdl:error: too many unit names for command '-e'` | `ghdl -e gate top tb` | `-e` takes ONE unit name. Two names are misread as primary + secondary (`error: cannot find entity or configuration gate`) |
| `ghdl:error: unknown command option '--std=08'` | `ghdl --std=08 -a f.vhd` (probed under the pinned-08 era) | ghdl's argv is command-first: every option rides AFTER the command -- the drive's argv does, and there is no std option left to misplace (the std pin dropped, 2026-11-08) |
| `error: architecture "rtl" of "top" is obsoleted by entity "gate"` | the drive's second run in one output directory, consumer-first blueprint | the stale work library clock: re-analysis obsoletes dependents, `ghdl -e` refuses the unchanged design -- the wipe exists for this |
| `simulation stopped @0ms` | `ghdl -r tb_stop` on a testbench calling `std.env.stop` | a legitimate stop exits 0 -- clean, not a crash-door finding |
| `cannot find resource library "grlib"` (+ `unit "grlib_pkg" not found in library "grlib"`, `entity "tb" was not analysed`, `ghdl:error: compilation error`) | `ghdl -a --work=work tb.vhd` with no `grlib-obj93.cf` in the cwd | the absent-sibling-library face, exit 1 -- four markers on ONE failed spawn; the retry resolves once the provider's row writes `grlib-obj93.cf` (the cross-library deferral face: the drive's multi-library blueprint runs `3 file(s) analyzed in 3 pass(es)`, probed through the drive) |
| exit 0 end to end, `(report note): SIM PASSED` relayed, two libraries | per-file `ghdl -a --work=work tb.vhd` / `--work=work gate.vhd` / `--work=grlib pkg.vhd` (consumers FIRST); then `ghdl -e --work=work tb`; then `ghdl -r --work=work tb` (same through the drive; the current argv -- no std flag, the tool default rides) | **the multi-library face**: one `<library>-obj<STD>.cf` per `--work` library in the cwd (`-obj93` under the default the drive rides); the consumers' `library grlib;` resolves against the sibling `grlib-obj93.cf` AUTOMATICALLY -- no `-P`/`-L` flag exists or is needed; elab/run pin the top's library and resolve the other library through the sibling file |
| `error: cannot find entity or configuration tb` (+ `ghdl:error: compilation error`) | `ghdl -e tb` with tb analyzed into `--work=counter` -- bare, and pinned to the wrong library (`--work=work`) alike | **the bare-top WORK-only search**, exit 1: even with the library's sibling file in the cwd, a bare (or wrongly pinned) top is searched in the named library alone -- why the drive pins `--work=<the top's library>` on every elaborate and run |
| exit 0, `(report note): SIM PASSED` | `ghdl -e --work=counter tb; ghdl -r --work=counter tb` (tb analyzed into `--work=counter`) | the pinned-library elab/run: the top resolves in its OWN library, what it references through the sibling file |
| `--work=MyLib` writes `mylib-obj93.cf` | `ghdl -a --work=MyLib pkg.vhd` | ghdl LOWERCASES the library name in the state file's name, and `library MyLib;` clauses still resolve against it -- why the drive's wipe keys on the whole `<library>-obj<STD>.cf` family (every std tag; the drive pins no std) |

Case-exactness is part of the contract: ghdl spells its failures in
lowercase, so `:ERROR:`, `(ASSERTION FAILURE)`, `(ASSERTION ERROR)`, and
`FATAL:` are not its faces and never judge a log -- an uppercase drift of a
real failure log judges clean, and so does a clean log that merely mentions
"ERROR" mid-text (a filename, an echoed banner). That is deliberate: the
retired gate this replaces counted `error):`/`ERROR` substrings, matched
nothing ghdl writes, and false-failed clean logs.

### nvc 1.23.0

| face / fact | argv that wrote it | what nvc did |
|------|-----|------|
| `** Failure: 0ms+0: the design is wrong` (then `   Process :tb_fail:_p0 at tb_fail.vhd:5`) | `nvc --std=2008 -a tb_fail.vhd; nvc --std=2008 -e tb_fail; nvc --std=2008 -r tb_fail` (same through the drive) | a violated `assert ... severity failure` stops the run, exit 1 -- honest |
| `** Error: 0ms+0: check failed` | `nvc --std=2008 -r tb_err` on a `severity error` assert | nvc prints the face, **continues** (`** Note: 0ms+0: simulation continued` follows), and **still exits 1** -- unlike bare ghdl's exit-0 hole, nvc's exit convicts on its own; the face is the second witness |
| `** Warning: 0ms+0: ...`, `** Note: 0ms+0: ...` | `nvc --std=2008 -r tb_warn` | the gentler severities on a continuing run that **exits 0** -- continuations, **never judged** |
| `** Failure: 0ms+0: stop now` / `** Error: 0ms+0: flagged` | `report ... severity failure;` / `report ... severity error;` statements, `nvc --std=2008 -r ...` | report statements ride the same faces: failure stops (exit 1), error continues and **exits 1** -- the bare-ghdl report hole does not exist on nvc |
| `** Error: unexpected end while parsing port clause, ...` above a `> broken.vhd:3` caret block | `nvc --std=2008 -a broken.vhd` | the analysis-error face (same `** Error:` marker as runtime), exit 1; the analyze step gates on the exit and is never judged |
| `** Error: design unit GATE not found in library WORK` | `nvc --std=2008 -a top.vhd` with `gate.vhd` unanalyzed | the order-dependence face: the consumer cannot analyze before its provider, exit 1 -- and the failed analysis does not poison the library: provider then consumer both analyze after (the deferral loop's shape, probed live) |
| `** Fatal: cannot find unit WORK.TB_NOSUCH in library WORK` | `nvc --std=2008 -e tb_nosuch` | the elaboration refusal when the top names no analyzed unit, exit 1 |
| `** Fatal: unrecognised analysis option --std=2008` | `nvc -a --std=2008 f.vhd` | nvc's argv is option-FIRST: the global `--std` leads and the command follows -- the opposite of ghdl's command-first shape |
| exit 0 end to end, `** Note: 0ms+0: SIM PASSED` relayed, two libraries | per-file `nvc --std=2008 -L . --work=grlib -a pkg.vhd` / `--work=work -a gate.vhd` / `--work=work -a tb.vhd` (consumers FIRST, in any order); then `nvc --std=2008 -L . --work=work -e tb`; then `nvc --std=2008 -L . --work=work -r tb` | **the multi-library face**: one directory per `--work` library in the cwd; the consumers' `library grlib;` resolves at analyze against `grlib/` because `-L .` puts the cwd on the search path; elab/run pin the top's library and resolve the other library's units through the search path |
| `** Error: library GRLIB not found` (+ caret block, `= Note: library search path contains: ., ...`) then `** Fatal: required library GRLIB not found` (+ the `add additional directories ... -L` note) | `nvc --std=2008 --work=work -a gate.vhd` with no `-L` on a `library grlib;` consumer; `nvc --std=2008 -e tb` with `grlib/` in the cwd but unsearched | **the missing-search-path face**, two markers on ONE failed spawn, exit 1: the reader's `** Error:` and the process-level `** Fatal:`; the note lists the search path it tried and names the `-L` fix -- why every drive spawn carries `-L .` |
| `** Error: library GRLIB not found` on the consumer-first analyze, then provider-then-consumer all exit 0 | blueprint order tb, gate (both `library grlib;`), pkg (into `--work=grlib`): pass 1 fails both consumers on the ABSENT `grlib/` directory while pkg creates it; passes 2-3 retire gate then tb | **the cross-library deferral face**: a library directory that does not exist yet reads as the library-not-found face, and the retry resolves -- the deferral loop needs no change to defer across libraries (the drive's multi-library blueprint runs `3 file(s) analyzed in 3 pass(es)`, probed through the drive) |
| `** Fatal: cannot find unit WORK.TB in library WORK` | `nvc --std=2008 -L . -e tb` with tb analyzed into `--work=grlib` | **the bare-top WORK-only search**, exit 1: even with the library's directory on the search path, a bare top is searched in WORK alone -- why the drive pins `--work=<the top's library>` on every elaborate and run |
| exit 0, `** Note: 0ms+0: SIM PASSED` | `nvc --std=2008 -L . --work=grlib -e tb; nvc --std=2008 -L . --work=grlib -r tb` (tb analyzed into grlib, nothing in work) | the pinned-library elab/run: the top resolves in its OWN library, what it references through the search path |
| `<lib>/_NVC_LIB` written even on a failed analyze | `nvc --std=2008 --work=freshlib -a broken.vhd` (syntax error, exit 1); `nvc --std=2008 --work=optfail -a --bogus broken.vhd` (option error, exit 1) | every directory nvc makes a work library carries the `_NVC_LIB` marker file (nvc's own recognition rule refuses a directory without it) -- the drive's wipe keys on it |
| re-analysis, elaboration and run all exit 0 | a second drive in one output directory: provider then consumer re-analyzed, `-e`, `-r` | nvc tolerates stale-library re-analysis without ghdl's obsoleted-design refusal -- against nvc itself the wipe is symmetry, not load-bearing; on a multi-library blueprint it IS load-bearing (a library this blueprint no longer analyzes must not stay resolvable) |

nvc's markers are case-exact capitals: a lowercase drift (`** error:`,
`** failure:`) is not one of its faces and never judges a log. And as with
ghdl, a testbench's own report note that embeds an nvc face shape
(`** Error: retrying`) convicts the run -- keep report notes free of
nvc-face shapes.

### iverilog 12.0 (stable) / vvp 12.0

| face / fact | argv that wrote it | what iverilog/vvp did |
|------|-----|------|
| `FATAL: tb_fatal.v:4: the design is wrong` (+ `       Time: 1  Scope: tb_fatal` beneath, on stdout) | `iverilog -g2012 -o f.vvp tb_fatal.v && vvp f.vvp` (same through the drive) | a `$fatal(1, "...")` call stops the run, exit 1 -- honest |
| `ERROR: tb_error.v:4: check failed` (+ Time/Scope, on stdout) | `iverilog -g2012 -o e.vvp tb_error.v && vvp e.vvp` | a `$error("...")` call: vvp prints the face and **continues** -- the testbench's own `$display` output and `tb_error.v:6: $finish called at 1 (1s)` follow -- and **exits 0**: the bare-ghdl hole again, the judge load-bearing |
| `WARNING: tb_warn.v:4: check flagged` (+ Time/Scope) | `vvp w.vvp` on a `$warning` call | the gentler severity on a continuing run that **exits 0** -- never judged |
| `tb_stop.v:5: $stop called at 1 (1s)`, `** VVP Stop(0) **`, `> ** Continue **` | `vvp s.vvp` on a `$stop` call | a `$stop` is an interactive stop, exit 0 -- a legitimate stop, clean, not a crash-door finding (a verilator model's `$stop` aborts instead: below) |
| `broken.v:2: syntax error` then `I give up.` (stderr) | `iverilog -o snap_broken.vvp broken.v` | the parse-error face, **exit 2**; the compile gates on the exit and is never judged, the face is recorded |
| `error: Unable to find the root module "nosuch_top" in the Verilog source.` then `1 error(s) during elaboration.` | `iverilog -g2012 -s nosuch_top -o x.vvp tb.v` | the unknown-`-s`-root refusal, **exit 1**: top names are validated at compile time, so the build face carries the check for free |
| consumer-first compile exits 0 | `iverilog -g2012 -o order.vvp top.v gate.v tb.v` (consumer before provider) | **order-free compile** -- one invocation, no deferral loop; repeated `-s` flags are legal (exit 0) and multiple roots share the one snapshot and the one `vvp` run |
| `$fatal` accepted at compile, exit 0 | `iverilog -o nog2012.vvp tb_fatal.v` (no `-g`) | iverilog 12.0's default generation also accepts `$fatal` -- the `-g2012` pin keeps the judged faces pinned, not the acceptance |

vvp's runtime failure faces are case-exact capitals on stdout: a
lowercase drift (`fatal:`, `error:`) is not one of its faces and never
judges a log. As with the VHDL tools, a testbench's own `$display` that
embeds a face shape (`ERROR: retrying`) convicts the run -- keep
`$display` text free of face shapes.

### verilator 5.032

| face / fact | argv that wrote it | what verilator / the model did |
|------|-----|------|
| `%Error: broken.v:2:3: syntax error, unexpected input, expecting ';'` (+ caret block, then `%Error: Exiting due to 1 error(s)`) | `verilator --lint-only broken.v` (same through the drive, which pins `--timing`) | the lint-error face, exit 1 -- the lint gates on the exit and is never judged, the face is recorded |
| `%Error-NEEDTIMINGOPT: tb.sv:7:5: Use --timing or --no-timing to specify how delays should be handled` | `verilator --lint-only tb.sv` (no `--timing`) on a delay-carrying file | the coded lint face, exit 1 -- why the drive pins `--timing` on every argv; the bare `%Error` vocabulary face subsumes the coded form |
| `%Error: Specified --top-module 'nosuch' was not found in design.` | `verilator --lint-only --timing --top-module nosuch tb.sv ...` | the refusal of an unknown top, exit 1 -- on the lint it fires only when the unknown name is the LAST `--top-module` (the honored one; a non-last one lints clean, exit 0), and every top is validated by its own per-top build on both faces |
| `%Warning-WIDTHTRUNC: ...` still exit 1 | `verilator --lint-only --timing -Wall widthy.sv` (and even under `-Wwarn-WIDTHTRUNC`) | an *enabled warning* fails the lint -- there is no exit-0 warning face in lint land; a warning face never needs judging |
| `%Fatal: tb_fatal.sv:8: Assertion failed in tb_fatal: the design is wrong`, then `%Error: tb_fatal.sv:8: Verilog $stop`, then `Aborting...` (stdout) | `verilator --binary --build --timing --top-module tb_fatal ... && ./obj_dir/Vtb_fatal` (same through the drive) | a model `$fatal(1, "...")`: the faces land, the model dies by **SIGABRT** -- no exit code at all (shell: `Aborted`; the drive records -1) -- the exit convicts, the face is the second witness |
| `%Error: tb_err.sv:8: Assertion failed in tb_err: check failed` (+ `%Error: ... Verilog $stop`, `Aborting...`) | `./obj_dir/Vtb_err` on a `$error("...")` call | a model `$error`: the same faces, the same SIGABRT -- unlike vvp's continue-and-exit-0 `$error`, the built model has no interactive continue |
| `%Error: tb_stop.sv:9: Verilog $stop` (+ `Aborting...`) | `./obj_dir/Vtb_stop` on a plain `$stop` | even a `$stop` aborts the model (SIGABRT) -- a finding on both witnesses; contrast vvp's interactive `$stop` (exit 0, clean) |
| `[1000] %Warning: tb_vwarn.sv:8: tb_vwarn: check flagged` | `./obj_dir/Vtb_vwarn` on a `$warning` call | the gentler severity in the model, continuing run, **exit 0** -- never judged |
| `- S i m u l a t i o n   R e p o r t: Verilator 5.032 ...`, `- Verilator: $finish at 1ns ...` | `./obj_dir/Vtb` on a clean run | the clean model's stdout banner -- continuations, never judged |
| consumer-first lint and build exit 0; repeated and differing `--top-module` flags exit 0; `obj_dir/Vtb` and `obj_dir/Vtb_err` coexist | `verilator --lint-only --timing --top-module tb --top-module tb_err tb.sv tb_err.sv top.sv gate.sv`; `verilator --binary --build --timing --top-module tb top.sv gate.sv tb.sv` | **order-free lint and build** -- one lint, per-top builds into the shared `obj_dir`, no deferral loop; multiple roots share one lint |

verilator's markers are case-exact capitals: a lowercase drift
(`%error:`, `%fatal:`) is not one of its faces and never judges a log.
And as with the other tools, a testbench's own `$display` that embeds a
verilator face shape (`%Error: retrying`) convicts the run -- keep
`$display` text free of face shapes.

### yosys 0.52

| face / fact | argv that wrote it | what yosys did |
|------|-----|------|
| `broken.v:3: ERROR: syntax error, unexpected TOK_ENDMODULE` (stderr) | `yosys -p "read_verilog broken.v; synth"` (same through the drive) | the frontend's syntax face, exit 1 -- the `ERROR:` marker rides mid-line behind the source location |
| `ERROR: Module \`nosuch' not found!` (stderr) | `yosys -p "read_verilog top.v gate.v; synth -top nosuch; write_verilog netlists/nosuch.v"` | the script's refusal of an unknown top, exit 1 -- the `-p` script aborts and no netlist is written |
| `ERROR: No such command: frobnicate (type 'help' for a command overview)` (stderr) | `yosys -p "read_verilog gate.v; frobnicate"` | the unknown-`-p`-command face, exit 1 |
| `ERROR: Can't open output file \`netlists-missing/x.v' for writing: No such file or directory` (stderr) | `yosys -p "...; write_verilog netlists-missing/x.v"` | the failed-write face, exit 1 -- a late failure still convicts |
| `ERROR: Module \`\gate' referenced in module `\top' in cell `\u_gate' is not part of the design.` (stderr) | `yosys -p "read_verilog top.v; synth -top top; ..."` with `gate.v` never read | the missing-provider face, exit 1 -- `synth`'s hierarchy check catches an instantiation whose provider was never read; no silent blackbox degradation |
| log on stdout, exit 0, `netlists/top.v` written | `yosys -p "read_verilog gate.v; synth; write_verilog netlists/top.v"` | the clean synth: the whole synthesis log rides stdout, the netlist lands |
| consumer-first synth exits 0 | `yosys -p "read_verilog top.v gate.v; synth -top top; write_verilog ..."` (consumer before provider) | **order-free** -- yosys reads all files into one design and elaborates at `synth` time; no deferral loop |

yosys's marker is case-exact uppercase (`ERROR:`), line-start on the
script faces and mid-line behind the source location on the frontend's
syntax face; a lowercase drift (`error:`) is not one of its faces and
never judges a log. The log rides stdout and the failure faces stderr;
the drive joins the streams, so the face reads either way.

### vivado 2025.2.1

| face / fact | argv that wrote it | what the tool did |
|------|-----|------|
| `Failure: the design is wrong` (+ the `Time: ... File: ...` line, on stdout) | `xvhdl --work=work tb_fatal.vhd && xelab -L work tb_fatal -s sim_fail && xsim sim_fail -R` (same through the drive) | a violated `assert ... severity failure` stops the check -- and **xsim EXITS 0**: the run exit is a lie, the line-start face is the only witness |
| `Error: check failed` (+ the continuation `Note:` line, on stdout) | `xsim sim_err -R` on a `severity error` assert | the face prints, the simulation **continues** (`Note: continued past the error` follows), and **xsim still exits 0** -- the same lie; the face convicts |
| `Note: sim done`, the kernel `INFO:` lines | `xsim sim_clean -R` on a passing testbench | the clean run exits 0 -- continuations, **never judged** |
| `ERROR: [VRFC 10-4982] syntax error near 'end' [...:9]` (+ `ERROR: [VRFC 10-2989]`, then the `10-8704` ignored line, on **stdout**) | `xvhdl --work=work broken.vhd` (same through the drive, which joins the streams) | the compile-error face, **exit 1**; stderr is EMPTY on every stage-binary drive, error or clean; the compile gates on the exit and is never judged -- the uppercase face is deliberately outside the mixed-case vocabulary |
| `ERROR: [VRFC 10-2987] 'top' is not compiled in library 'work'` | `xvhdl --work=work tb_clean.vhd top.vhd gate.vhd` (consumer before provider) | **order-sensitive within the work library**, exit 1 -- the deferral loop's reason to exist here; even the tb-gate-top order fails (`gate` before `top` is required) |
| `INFO: [VRFC 10-163] Analyzing VHDL file ".../pkg.vhd" into library grlib`, exit 0, and `xsim.dir/grlib/` appears in the cwd | `xvhdl --work=grlib pkg.vhd`, beside a `-work work` compile: `xsim.dir/` then holds `grlib/` and `work/` as siblings | **the per-library compile face** (the 2026-11-07 argv, superseded on the sim compile by the 2026-11-08 single-work ruling): `--work=<lib>` is accepted with the equals spelling and lands the named library INSIDE the one `xsim.dir/` of the working directory -- the blueprint's libraries live on in the build face's `read_vhdl -library` reads and the sim face's `xsim.ini` mappings |
| `INFO: [VRFC 10-2263] Analyzing Verilog file ...` , exit 0 | `xvlog vtop.v vgate.v` (consumer before provider) | **order-free** -- xvlog compiles in one invocation, no deferral loop; `-work work` pins the library (Verilog speaks no library clauses: the blueprint's library column has no xvlog face) |
| `Compiling package grlib.grlib_pkg` ... `Built simulation snapshot tb`, exit 0; then `Note: SIM PASSED`, exit 0 | `xelab -L grlib -L work work.tb -s tb && xsim tb -R` (the package compiled into `--work=grlib`, the design and tb into `work`) | the per-library era's elaboration face (superseded by the single-work ruling): the top rode its own library's prefix (`work.tb`); the cross-library `library grlib;` clause resolved through the sibling `xsim.dir/grlib/` -- and the pure-VHDL faces needed no `-L` at all (probed: `xelab work.tb -s tb` with NO `-L` anywhere builds and passes; the `-L` set is the Verilog and mixed-language face, per xelab's own help) |
| exit 0, `Built simulation snapshot tb2` | `xelab -L grlib -L work grlib.tb -s tb` with EVERYTHING compiled into `--work=grlib` (`xsim.dir/` holds no `work/` at all), and `xelab -L grlib -L nosuchlib grlib.tb -s tb` | the single-library collapse: an explicit non-`work` prefix works, and a `-L` naming an absent library is accepted **silently** -- over-naming costs nothing |
| `ERROR: [XSIM 43-3225] Cannot find design unit work.tb in library work located at xsim.dir/work.` -- the refusal names the IMPLICIT `work.` prefix | `xelab -L work -L grlib tb -s tb` (bare) with EVERYTHING compiled into `--work=grlib`; same under `-L grlib -L work` | **the bare top is work-only**: a bare design unit is implicitly `work.`-qualified and resolves in the work library alone -- why the per-library era pinned the prefix, and why the single-work ruling makes the bare top the natural face (everything IS in work) |
| `Note: SIM PASSED`, exit 0 | `xelab -L work -L grlib work.tb -s tb` AND the bare `xelab -L work -L grlib tb -s tb`, on a tb in `work` whose component declaration (in the grlib package) and entity BOTH live in `--work=grlib` | **the binding rule is the component's own library**: a default component binding (no binding indication) searches the component declaration's library -- it binds under the bare and the qualified top ALIKE; the top's form is binding-neutral (the multi-library drive live-pins this through its own component fixture) |
| `WARNING: [VRFC 10-4940] 'gate' remains a black box since it has no binding entity [...:11]`, snapshot built, the run fails downstream | the same tb shape with the component declared in the `work` tb and the entity compiled into `--work=grlib` -- under the qualified top, the bare top, with `-L`, with `-relax`, in either `-L` order, AND through an `xsim.ini`-mapped alias | **the multi-library wall**: a default binding CANNOT cross libraries, no xelab flag extends the search, and the mapped-alias merge does not lift it -- designs relying on it (the GRLIB style; GRLIB's own sim flows keep one physical library) merge into `work` by the single-work ruling |
| `ERROR: [XSIM 43-3225] Cannot find design unit work.tb_nosuch in library work located at xsim.dir/work.` | `xelab -L work tb_nosuch -s sim_nosuch` | the elaboration refusal of an unknown top, **exit 1** -- honest, the elaboration gates on the exit |
| `ERROR: Please check the snapshot name ... does not exist` | `xsim nosuch_snap -R` | the missing-snapshot face, **exit 1** -- the crash door; uppercase, outside the vocabulary, the exit convicts |
| `SYNTH FAILED: ERROR: [Common 17-69] Command failed: Vivado Synthesis failed`, **exit 1** | `vivado -mode batch -nojournal -nolog -source synth.tcl` with a syntax-broken `read_vhdl` inside the catch-guard | the fail-fast batch face: the return-code catch-guard stops the script and exits 1 (unguarded batch **continues past a failed synth_design**, probed) |
| **exit 1 with all three netlists written** | the same script whose guard tested `$synth_err` emptiness instead of catch's return code | **the probed Tcl trap**: `catch {script} var` writes the script's NORMAL RETURN VALUE into `var`, and `write_verilog`/`write_checkpoint` return the written filename -- an emptiness guard false-fails a CLEAN synth |
| exit 0, `netlists/top.dcp` + `top.v` + `top.edf` written | `vivado -mode batch -nojournal -nolog -source synth.tcl`, guard on catch's return code, `synth_design -top top -part xc7z010clg400-1 -mode out_of_context` | the clean synth face; the echo of the sourced commands rides stdout, the synth errors ride **stderr**, exit honest |
| exit 0, the netlists written, a TWO-library closure (`read_vhdl -library grlib ./pkg.vhd` + `read_vhdl -library work ./gate.vhd` + `read_vhdl -library work ./tb.vhd` in the sourced script) | the generated script driven through `vivado-build` (`cargo test -- --ignored`, the gated harness) | **the library column's script face**: each VHDL read names the file's own library -- the compile's mirror, the GRLIB flow's own shape (probed live on the GRLIB flow); `synth_design -top` finds the top across the named libraries unchanged |
| the tb carrying `library grlib; use grlib.grlib_pkg.all;` compiles clean into `work`; the SAME source without the mapping dies on the dangling clause | cwd `xsim.ini` carrying `grlib=xsim.dir/work`, then `xvhdl --work=work tb.vhd` (the drive writes it per drive; `ieee`/`std` still resolve beside the partial ini) | **the mapped-library face** (the GRLIB `vmap` convention -- many logical names, one physical library): the single-work compile's clause resolution, the drive's own two-library live fixture riding it |
| `Note: SIM PASSED`, exit 0, a TWO-library blueprint | `xvhdl --work=work tb.vhd` / `--work=work gate.vhd` / `--work=work pkg.vhd` (one invocation per file, blueprint order, `xsim.ini` mapping `grlib`) then `xelab -L work tb -s tb && xsim tb -R` -- the drive's own gated live fixture | **the single-work sim face** (the 2026-11-08 ruling): everything merges into one physical library, the direct `entity work.gate` reference resolves inside it, the bare top is its natural reading, the deferral loop unchanged |
| `-version` exits 0 on every family binary | `vivado -version`, `xvhdl -version`, `xvlog -version`, `xelab -version`, `xsim -version` | the identity face (single dash), the drive's spawnability probe |

vivado's run vocabulary is line-start mixed-case and case-exact: an
uppercase drift (`FATAL:`, `ERROR:`) is not one of xsim's run faces and
never judges a log -- and the uppercase `ERROR:` faces of the compile and
elaboration stages are deliberately outside the vocabulary, because those
stages gate on their honest exits. A testbench's own report note that
LEADS its line with a face shape (`Failure: retrying`) convicts the run;
mid-line prose never does. The recipe env (see the Vivado drive page) rides every spawn, set
by the drive; `.Xil/` and the stage logs (`xvhdl.log`, `xvlog.log`,
`xsim.log`, `xsim.jou`) land in the working directory and are the tools'
own scratch.

### cocotb 2.1.0 (+ ghdl 6.0.0, the backing simulator)

| face / fact | argv that wrote it | what cocotb/the runner did |
|------|-----|------|
| `** test_counter.reads_the_first_count   PASS ...`, then `** TESTS=1 PASS=1 FAIL=0 SKIP=0` | `get_runner("ghdl")`; `build(sources=[counter.vhd], hdl_toplevel="counter", always=True)`; `test(test_module="test_counter", hdl_toplevel="counter")` | the clean pass: the regression table's PASS row and the clean summary, **exit 0** through the generated script |
| `** test_counter_fail.expects_the_wrong_count   FAIL ...`, then `** TESTS=1 PASS=0 FAIL=1 SKIP=0` (+ the `AssertionError: the design is wrong` traceback above) | the same runner call, `test_module="test_counter_fail"` | a failed assert: the row column reads FAIL, the summary counts it -- and **`ghdl -r` EXITS 0**: the finding lands only in results.xml, the generated script's `get_results` verdict is the load-bearing exit (exit 1) |
| `ERROR: Simulation terminated abnormally. Results file ... not found.` | `test_module` naming a module that fails to import (or carries no tests: `RuntimeError: No tests were discovered in any module: ...`) | the sim dies before results.xml is written; `cocotb_tools.check_results.get_results` raises, the generated script relays the message, **exit 1** -- the crash door |
| `Simulation failed: <rc>` (the runner's log), then the script exits with the sim's code | the runner's own `_execute` on a nonzero simulator exit | a SIMULATOR-level failure exits the runner nonzero -- honest; only the failed-TEST exit lies |
| `python -m cocotb.runner` does not exist; `TOPLEVEL`/`SIM` env vars do not exist | the 2.x runner face | the runner is `cocotb_tools.runner.get_runner(...)`, the env vars renamed (`COCOTB_TOPLEVEL`, `COCOTB_TEST_MODULES`, `COCOTB_RESULTS_FILE`), all set by the runner itself |
| `ValueError: Ghdl only supports VHDL. '<file>' cannot be compiled.` | the runner's own `_build_command` on a non-VHDL source | the ghdl backend compiles VHDL only -- the drive's fileset door, probed shut |
| the runner unlinks `results.xml` before each test; `build_dir` defaults to `sim_build` | the runner's source, probed behavior | the stale-results door is closed by the runner; the drive's own `sim_build/` wipe is symmetry |

cocotb's row column is uppercase `FAIL`, space-padded on both sides --
case-exact, and the padding is load-bearing: the clean summary's own
`FAIL=0` counter (and bare `FAIL` prose) shares no shape with ` FAIL `,
so it never judges a clean run. The faces are cocotb's OWN aggregates:
the backing simulator's lowercase ghdl faces belong to the ghdl
vocabulary, never this one.

