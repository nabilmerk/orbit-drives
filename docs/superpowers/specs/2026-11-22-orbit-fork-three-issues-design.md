# Fork orbit and resolve #23 / #24 / #25; orbit-drives rides the fork

- Date: 2026-11-22
- Status: executing (approaches pre-picked by the issues' own "the ask"
  sections; remaining calls recorded here with rationale, per the
  standing no-review-gates rule)
- Type: upstream fork campaign + integration

## Problem

Three open issues by the owner against `chaseruskin/orbit` 0.29.3
(all verified live against a trunk build, 2026-11-22):

- **#23** — duplicate design units across *different libraries* hard-error
  when the projects connect through a **path dependency**. The duplicate
  check is name-only (`unit_map: HashMap<LangIdentifier, _>`,
  `src/core/algo.rs:115-119`); the relative branch never applies the DST
  transform that the catalog branch applies, though its plumbing
  (`CacheSlot::new_rel`, `install_dst`) is half-built and unreachable.
- **#24** — no language-standard face anywhere in the stack: no manifest
  field, no target key, no CLI flag. Tools' std defaults disagree
  (ghdl 6.0 defaults 93; nvc 2008-era), so the same workspace compiles
  differently per target through no user choice. The only std selector
  in the entire stack lives inside drives (`nvc.rs` `--std=2008` pin,
  iverilog `-g2012`, quartus `-2008`).
- **#25** — no tool-selection face: no default target (even a single
  candidate is not inferred), capability encoded only in target *names*
  (`nvc` vs `nvc-build`), no manifest toolchain section. (The issue also
  claims a `[[target]]` array in `Orbit.toml` "parses clean and
  registers on no verb" — re-probed 2026-11-22: it is in fact a loud
  parse error via `deny_unknown_fields`; the issue's transcript never
  loads the manifest because target selection precedes it. The residual
  trap is that the parse error does not say where targets belong.)

orbit-drives is the target package these gaps force workarounds into.
The campaign: fork orbit to `nabilmerk/orbit`, fix all three there, then
make orbit-drives work with the fork (capability declarations, std
consumption, docs, tests) and push both.

## Repos and finish state

- Fork: `nabilmerk/orbit` (created 2026-11-22, default branch `trunk`,
  local clone `../orbit` at upstream HEAD 896a1a5, remote `fork` added).
  Fixes land on three branches (`issue-23-dst-library-namespace`,
  `issue-25-toolchain-face`, `issue-24-std-face`), each merged to
  `trunk` after its tests are green; trunk pushed to the fork. No
  upstream PRs (not asked; the fork carries the fixes).
- orbit-drives: one branch merged to `main`, pushed to origin, per the
  standing finish rule.
- CHANGELOG (fork): `## 0.29.4 - unreleased` block per CONTRIBUTING;
  `Cargo.toml` version bumped to 0.29.4 only if the release-check tooling
  demands the pairing (verify `tools/evalver.py` first, else leave 0.29.3).

## Decision #23 — qualify the RELATIVE branch's duplicate check by library; same-library collisions get the honest error

The issue offers: (1) qualify the check by library, or (2) apply the DST
transform on the relative branch. **Taking (1), scoped to the relative
branch only, with (2) deliberately not taken.**

Amendment (2026-11-22, evidence from `tests/s3`): the spec first said
"uniformly on both branches". The s3 fixture proves the catalog branch
MUST keep its name-only semantics — there, `ip-c` (library `ip-c`) and
catalog dep `ip-b` (library `dupe`) collide **cross-library**, and the
DST transform is upstream's tested promise for exactly that shape
(`dupe_ca99a73147e03b1d` asserted in `dst.sh`; dst.md promises indirect
clashes get transformed). Library-qualifying the catalog branch would
silently stop that transform and break `dst.sh`. So: catalog branch
byte-identical; the relative branch — the branch issue #23 is about —
gets the library-qualified check.

- A library is a namespace (LRM): `demo4.config` and `dep4.config` are
  distinct units. In the relative branch, the collision check requires
  the same unit name AND the same owning project library; cross-library
  duplicates coexist. This is the GRLIB shape (`grlib.config` next to a
  design's own `config`) through path dependencies, and it becomes
  plannable.
- Same library + same unit name across relative-linked projects = a true
  homograph collision: root-vs-dep keeps the existing
  `DuplicateAcrossDirect` error; dep-vs-dep gets the resurrected
  `DuplicateAcrossLocal` variant ("conflicts with local dependency" —
  dead code today; the current message "conflicts with direct
  dependency" is wrong for dep-vs-dep relative collisions).
- Implementation shape: `unit_map` keys become
  `(LangIdentifier /*library*/, LangIdentifier /*unit*/)`. The catalog
  branch's collision check scans all entries by unit name (any library —
  preserving today's name-only behavior); the relative branch's check is
  a qualified-key lookup; insertions use qualified keys so relative
  cross-library coexistence can hold both units.
- Why not (2): DST-on-relative is exactly what option (1) makes
  unnecessary for the reported case, `generate_dst_lut` explicitly
  refuses non-physical mappings (`project.rs:388-390`), and mangling a
  user's checked-in path-dependency sources is a heavier promise than
  the issue requires. Recorded as future work, not built.
- Docs (`docs/src/topic/dst.md` lines 24-28, 107) rewritten to match the
  implementation: catalog indirect clashes transform (unchanged promise);
  relative duplicates qualify by library — cross-library legal,
  same-library an error (hints unchanged in spirit: rename or
  project.ignore).
- Tests: new system test (fixture projects, path dep, cross-library
  duplicate → `orbit build --plan tsv` exits 0, blueprint carries both
  libraries) + same-library relative collision → the local-dependency
  error; unit tests for the qualified key; existing `tests/s3/dst.sh`
  (catalog transform) must stay green unchanged.

## Decision #25 — a manifest `[toolchain]` face; capability validation at the CLI; the noop becomes loud

Schema (manifest; `deny_unknown_fields`, serde defaults, getters on
`Manifest`, `Manifest::new` updated):

```toml
[toolchain]
default = "nvc"          # fallback for any verb
build   = "nvc-build"    # per-verb binding, one target owns the verb
test    = "nvc"
```

- Resolution precedence in `Context::select_target` (the manifest face
  rides in as a new input): CLI `--target` > `ORBIT_TARGET` env >
  manifest per-verb key (`build`/`test`) > manifest `default` > config
  `[build]`/`[test].target`. The manifest tier sits above config
  defaults because it is in-repo: "a project cannot declare 'test with
  nvc, synth with vivado'" is the issue's complaint.
- Single-candidate inference: when no tier resolves a name and exactly
  one target exists for the verb, infer it (the issue's "error even when
  exactly one target is defined"). Zero or many → today's errors,
  unchanged. `--list` marks the effective default, inference included.
- Capability validation: orbit already carries the face — `Target`
  `build`/`test` bools defaulting to `true` (`target.rs:49-96`) — but
  nothing meaningful declares them. Two halves:
  1. orbit's mismatch face gets precise: selecting a target that
     *exists* but declared the other capability is its own loud exit-2
     ("target `x` is not a test target (it declares build only)" +
     the `--list` hint) instead of the misleading "no test target
     named".
  2. orbit-drives' `config.toml` declares capability on all 14 targets
     (`test = true, build = false` on sim drives, inverse on build
     drives). Unspecified stays permissive for third-party configs.
- The manifest `[[target]]` trap: already a loud parse error
  (`deny_unknown_fields` — re-probed; the issue's "silent noop" was an
  inference from a transcript that never loads the manifest). The fix is
  the teaching face: when the manifest parse error is the unknown-field
  `target` case, append a hint (targets live in orbit config; bind one
  per verb in `[toolchain]`). Cargo-style manifest-local target
  *definitions* are a non-goal — the config-include pattern already
  defines targets, and filesets/plans semantics make manifest-local
  definitions a much larger surface. Why not support them: the ask is
  the tool-selection face, not target definition; the trap face now
  teaches the pattern either way.
- "synth" from the ask maps to orbit's build verb (the drive
  `vivado-build` IS synthesis through `orbit build`); a `synth` key no
  orbit verb reads would be a new silent noop — the exact class #25
  condemns. Keys are the verbs that take targets: `build`, `test`.
- Docs: `reference/manifest.md` gains `[toolchain]`;
  `reference/configuration.md` documents the precedence chain; the
  build/test command pages' `--target` prose updated via
  `docs/commands.toml` if it names resolution.

## Decision #24 — `[project] std`, carried per-file in the blueprint and by `ORBIT_STD`, gated by a per-target `mixed-std` face

- Manifest: `[project] std = "93"` (bare revision token; per project =
  per library, the ask's model). Documented vocabulary per language —
  vhdl: `87 | 93 | 2008 | 2019`; verilog: `95 | 2001 | 2005 | 2012`;
  sv: `2005 | 2012 | 2017` — validated at load: unknown token = loud
  exit-2 naming the vocabulary (no silent degradation).
- Carrier: the blueprint gains a trailing `std` field — 4th TSV column,
  `std` key in `JsonEntry` — holding the row's owning project's std,
  empty when undeclared. orbit-drives' `parse_blueprint` (strict 3-field
  today) becomes strict-3 + optional trailing std. Old drives against a
  std-carrying blueprint fail loud (the pairing requirement is
  documented); drives without std rows are unaffected.
- Environment: `ORBIT_STD` exports the root project's std (the uniform
  case) through the existing env-file face; swap variable
  `{{ orbit.std }}` added alongside.
- Per-tool mixed-revision face: `mixed-std = true/false` on `[[target]]`
  (default unset = tolerant — no new failure modes for existing
  workspaces; `false` = the discipline choice, erroring loudly when the
  graph mixes declared stds). Validation runs at verb execution after
  planning: >1 distinct declared std + `mixed-std = false` → exit-2
  naming the projects and their stds. Tools that compile one set with
  one selector (iverilog, verilator) are the natural `mixed-std = false`
  declarers; per-file-analyzing tools (ghdl, nvc, xsim/vhdl, vcom) can
  stay tolerant.
- Drive consumption (orbit-drives): a shared std-token→flag mapping per
  tool, `ORBIT_STD` as the top precedence tier over the interim env
  pins — nvc (`--std=`), ghdl (`--std=`; conditional re-insert, the
  2026-11-08 un-pinning stays for the unset case), iverilog (`-g`),
  quartus (`-`+token for vcom), verilator (`--language`), yosys none
  (honest), cocotb via runner `build_args` (probed; if cocotb's runner
  refuses, the face stays honest exit-2 and is documented).
  `ORBIT_DRIVES_NVC_STD` keeps working below `ORBIT_STD`.
- Planner honesty: docs note (limitations) that orbit's reference-walk
  tokenizer parses under a union grammar (2008-era LRM citations) — the
  tokenizer itself does not become std-aware (that would be language
  logic creep; the drives own the std semantics, orbit carries them).
- Tests: manifest std parse/validation units; blueprint column units
  both sides; mixed-std validation unit + system test (two path-dep
  projects, different stds: tolerant target plans clean with per-file
  stds in the blueprint, strict target exits loud); shape tests per
  drive for the new argv face.

## Non-goals (this campaign)

- No upstream PRs; no DST on the relative branch; no manifest-local
  target definitions; no std-aware orbit tokenizer; cocotb's std face
  only if its runner accepts it cleanly (else documented exit-2).
- The pre-existing ALLHDL routing bug in cocotb/quartus sim faces (raw
  `fileset != "VHDL"` filters bypass the by-extension routing) is parked
  in TODO.md, not fixed here — separate defect, separate pass.

## Verification

1. Fork: `cargo test` (159 inline tests) + `just run-sys-tests` green
   per fix branch, new tests added per issue (TDD: failing test first);
   `cargo build --release` under the box's 4 GB cap.
2. Integration (this box has ghdl/nvc/iverilog/verilator/yosys): forked
   orbit installed; end-to-end probe — fresh project with `[toolchain]`
   + `std`, `orbit build` infers/binds correctly, `orbit test --target
   vivado-build` dies loudly at the CLI, drives receive the std face;
   then orbit-drives' full suites (`cargo test`,
   `cargo test -- --include-ignored` for the tools present here).
3. Push both; record verdicts in this spec's table.

## Commits

Plain messages, no attribution lines (owner rule). Fork commits
reference the upstream issue numbers (`(closes #23)` style, matching
upstream CHANGELOG convention). Lowercase conventional subjects in the
family style.

## Verdicts and amendments (execution, 2026-11-22)

| # | Verdict (evidence) | Amendment |
|---|---|---|
| 1 | Issue #25's "[[target]] parses clean" is wrong in mechanism: `deny_unknown_fields` makes it a loud parse error; the issue's transcript never loads the manifest (target selection precedes it) — re-probed | The fix became the teaching hint appended to that parse error, not a new loud face |
| 2 | s3's catalog collision is CROSS-library (ip-c lib `ip-c` vs ip-b lib `dupe`) and upstream's DST exists for exactly that shape | Decision #23 rescoped: the relative branch qualifies by library; the catalog branch stays byte-identical (`dst.sh` green unchanged) |
| 3 | In-process `Context` unit tests would touch the real `$HOME` (context construction reads the user config) | The resolution tiers extracted as the pure `resolve_target_name`; the CLI faces proven by the s11/s12 fixtures instead |
| 4 | An always-on 4th blueprint column breaks upstream's own s7/s9 `comp.py` (`split('\t')[-1]` reads the path) | The std column is conditional: any declared std → all rows carry it; none → byte-identical 3-column |
| 5 | The shipped orbit-drives targets ride the ALLHDL custom fileset, whose rows are `Auxiliary` entries — the first e2e probe showed std EMPTY through them | `Entry::Auxiliary` gained the std field; both custom-fileset add paths pass the owning project's std; s13 case A2 pins it |
| 6 | ghdl 6.0 REJECTS `--std=87` (probed live); nvc accepts only 1993/2008/2019; the iverilog/verilator spellings probed live | The stdmap tables are probed facts with dates, not guesses; 87-through-ghdl and 2019-through-vcom are the loud exit-2 |
| 7 | cocotb 2.1.0 installable on this box (`pip3 install --break-system-packages`); the ghdl runner's `build(build_args=[...])` signature-probed | The cocotb std face shipped live (single-set + ghdl spellings), not documented-only |
| 8 | The installed orbit binary predated the Auxiliary amendment — the first re-probe showed empty stds through ALLHDL rows | Environment lesson: reinstall after every orbit-side amendment; the re-probe showed `93` riding the rows |
| 9 | s13's case C2 grep pinned the std ORDER in the mixed-std face message — the graph walk order is not deterministic | The assertion is order-agnostic (both tokens, either order) |
| 10 | The s12 inference face cannot run live on this box (the developer global config's 19 targets pollute the candidate set; ORBIT_HOME redirection breaks on the include walk) | s12 honest-skips loudly when polluted; the unit tests pin the tier and a fresh CI runner runs it live |

## Final review (fresh Opus reviewer, 2026-11-22) and fix pass

Verdict: no Critical; all five review-focus guarantees hold. Two
Important findings fixed RED→GREEN, one Minor regraded Important by
effect and fixed, the rest deferred:

| Finding | Grade | Action |
|---|---|---|
| quartus-build's single-set check refused legal cross-language std graphs (vhdl 93 + verilog 2001) | Important | fixed: per-language grouping — only a same-language disagreement refuses; RED→GREEN |
| stdmap's env-mutating test raced the drives' default-path tests on a false safety comment | Important | fixed: pure `resolve_std(entry_std, env_std)` split, no env mutation in tests; RED→GREEN |
| `.v 2005` + `.sv 2005` compiled under whichever selector came first (order-dependent results) | Minor → regraded Important by effect | fixed: iverilog/verilator compare RESOLVED selectors, not tokens; RED→GREEN |
| sibling same-library relative deps report "direct" where "local" is promised (message misattribution, refusal correct) | Minor | deferred |
| catalog-branch dupe witness pick nondeterministic when a name coexists in 2+ libraries (message-only) | Minor | deferred |
| malformed `[toolchain]` silently drops the tier in list mode (build/test fail loudly) | Minor | deferred |
| elaborate/run std rides the root's std, not the top's owning project's | Minor | deferred |
| custom-fileset rows bypass the planning-time vocabulary check (loud later at the drive) | Minor | deferred |
| ghdl prints std faces in Debug form | Minor | deferred |
| stale exported `ORBIT_STD` leaks into std-less projects | Minor | deferred |
| unused `verilog_only` import + orphaned `EX11` fixture | Minor | the import and one `mut` (both this campaign's warnings, the pristine-output gate) fixed; `EX11` deferred |
