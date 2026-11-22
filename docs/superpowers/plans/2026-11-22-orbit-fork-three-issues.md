# Orbit Fork Three Issues Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix chaseruskin/orbit issues #23/#24/#25 on the `nabilmerk/orbit` fork, then make orbit-drives work with the forked orbit.

**Architecture:** Three independent fix branches on the fork (relative-branch library-qualified duplicate check; manifest `[toolchain]` face with capability validation; `[project] std` carried per-file in the blueprint + `ORBIT_STD` + per-target `mixed-std`), each TDD'd, merged to `trunk`. Then one orbit-drives branch: capability declarations in config.toml, drives consuming the std face, blueprint 4-column parsing, docs, end-to-end probe, merge to `main`.

**Tech Stack:** Rust (orbit: edition 2024, serde+toml; orbit-drives: std-only, edition 2021), bash system tests (just), gh CLI.

**Spec:** `docs/superpowers/specs/2026-11-22-orbit-fork-three-issues-design.md` — the plan argues from the spec; executors read both.

## Global Constraints

- Every cargo command runs under `systemd-run --user --scope -p MemoryMax=4000M nice -n 19` (box rule).
- Commit messages carry no attribution lines (owner rule, overrides harness defaults). Fork commit subjects may cite upstream issue numbers (`(closes #23)` style). Lowercase conventional subjects in the family style: `type: the <change> -- the <detail>`.
- orbit repo: `/home/nabil/claude-dev/electron-ic/orbit` — remotes: `origin`=upstream, `fork`=nabilmerk/orbit. Fix branches off `trunk` (896a1a5). orbit-drives repo: `/home/nabil/claude-dev/electron-ic/orbit-drives` — branch off `main`.
- orbit tests: inline `#[cfg(test)]` modules; system tests `tests/sN/<name>.sh` + `comp.py` wired via `justfile` (`run-sys-tests` recipe = the CI set). The plan adds `tests/s10/` (next free number).
- TDD: every fix lands with its failing test first, seen failing, then green.
- Branch order on the fork: `issue-23-dst-library-namespace` → `issue-25-toolchain-face` → `issue-24-std-face`, each merged to `trunk` when green (later branches rebase on updated trunk).
- orbit-drives gates: `cargo test` always-run suite; live tool tests are `#[ignore]`d with inner PATH guards, run here with `-- --include-ignored` (ghdl/nvc/iverilog/verilator/yosys present; cocotb/vivado/quartus absent — their suites honest-skip).
- English everywhere; the spec's amendment table records every verdict.

## Review Focus

- A catalog dependency colliding cross-library with the root must STILL transform (upstream's `dst.sh` promise) — pinned by the existing `tests/s3/dst.sh` staying green and by the new qualified-key unit test in Task 1 asserting the catalog scan is name-only.
- A relative dep whose units collide only in NAME (different libraries) must plan clean and appear under both libraries in the blueprint — pinned by `tests/s10/rel-dupes.sh`.
- `orbit test --target <build-only-target>` must die at the CLI before any drive spawns — pinned by the select_target unit test in Task 5 (no target process runs; error names the verb).
- A std-carrying blueprint must not break 3-column consumers: orbit-drives parses 3 fields as before and a 4th only when present — pinned by Task 14's parse tests (old 3-column text unchanged behavior).
- An unset `[toolchain]`/`std`/`mixed-std` must keep every existing workspace byte-compatible: no new defaults, no new errors — pinned by `cargo test` (159 inline tests) staying green on each fork branch and `tests/config_shape.rs` updated in lockstep in Task 15.

---

### Task 1: #23 — library-qualified duplicate check on the relative branch

**Files:**
- Modify: `src/core/algo.rs:114-119` (unit_map type + root seed), `:146-150` (relative collision check), `:185-197` (relative insertion), `:252-284` (catalog scan stays name-only over qualified keys), `:286-299` (catalog insertion)
- Modify: `src/core/lang/vhdl/primaryunit.rs:373-385` (DuplicateAcrossLocal — now constructed; verilog/sv twins `src/core/lang/verilog/primaryunit.rs:211+`, `src/core/lang/sv/primaryunit.rs:279+`)
- Test: inline `#[cfg(test)]` in `src/core/lang/vhdl/primaryunit.rs` (Display test), `tests/s10/` (Task 2 carries the end-to-end proof)

**Interfaces:**
- Produces: `unit_map: HashMap<(LangIdentifier, LangIdentifier), (LangUnit, ProjectIdSpec)>` — key = `(owning library, unit identifier)`; catalog branch collision predicate `unit_map.keys().any(|(_, u)| u == key)` (name-only); relative branch predicate `unit_map.contains_key(&(lib.clone(), key.clone()))`.
- Produces: `HdlNamingError::DuplicateAcrossLocal(iden, ProjectIdSpec, PathBuf, Position)` — constructed in the relative dep-vs-dep same-library arm.

- [ ] **Step 1: Failing Display test for DuplicateAcrossLocal** — in `primaryunit.rs` test module, assert the Display output contains `conflicts with local dependency:` and the unit name (pattern follows the existing error tests if present; else a plain format! assertion).
- [ ] **Step 2: Run** `systemd-run --user --scope -p MemoryMax=4000M nice -n 19 cargo test -p orbit --lib` — expected: PASS (variant exists; this pins the message before it gains a producer).
- [ ] **Step 3: Qualified keys in graph_project** — change `unit_map` to the `(LangIdentifier, LangIdentifier)` key type; seed root units under `root.get_hdl_library()`; relative branch: collision lookup by `(relative_prj.get_hdl_library(), key)` — on hit, `is_root` arm keeps `DuplicateAcrossDirect`, else constructs `DuplicateAcrossLocal(name, dupe.1.clone(), local path, local pos)`; relative insertion uses qualified keys. Catalog branch: collision scan `units.iter().find(|(key, _)| unit_map.keys().any(|(_, u)| u == key))` (name-only semantics preserved); insertion uses qualified keys so relative cross-library coexistence holds both units.
- [ ] **Step 4: Run** full `cargo test` + `just run-sys-tests` (s3's `dst.sh` MUST stay green — the catalog promise) — expected: PASS.
- [ ] **Step 5: Commit** on branch `issue-23-dst-library-namespace`: `fix: the relative branch qualifies duplicate units by library -- cross-library path deps plan, same-library errors honestly (closes #23)`

### Task 2: #23 — system test for the relative-branch semantics

**Files:**
- Create: `tests/s10/README.md`, `tests/s10/rel-dupes.sh`, fixture projects `tests/s10/root/`, `tests/s10/dep/` (each: `Orbit.toml` with distinct `library` + one `package config` + root has `entity root_unit`), `tests/s10/same-lib/` variants (dep declares the SAME library as root)
- Modify: `justfile` — add `test-rel-dupes` recipe + wire into `run-sys-tests`

**Interfaces:**
- Consumes: Task 1's behavior (cross-library relative dupes plan; same-library relative dep-vs-dep errors "conflicts with local dependency").

- [ ] **Step 1: Write `rel-dupes.sh`** — pattern of `tests/s3/dst.sh` (`set -e`, run orbit, compare exact expected stdout, exit 101 on mismatch): case A cross-library → `orbit build --plan tsv` exits 0 and the blueprint (stdout of the run or the written file) carries both libraries' rows; case B same-library → `orbit build --plan tsv` exits with the local-dependency error (assert the message substring via `! orbit ... 2>&1 | grep -q "conflicts with local dependency"` inverted per the s3 grep style).
- [ ] **Step 2: Wire into justfile** `run-sys-tests` (CI set), add the recipe.
- [ ] **Step 3: Run** `just test-rel-dupes` — expected: PASS (this is the issue's reproducer, now green).
- [ ] **Step 4: Commit**: `test: the relative-duplicate system fixture -- cross-library plans, same-library errors`

### Task 3: #23 — docs + changelog, merge

**Files:**
- Modify: `docs/src/topic/dst.md:24-28,107` — limitations rewritten: catalog indirect clashes transform (unchanged); relative duplicates qualify by library (cross-library legal, same-library error with its message); the transform's `new_rel` plumbing mention if accurate.
- Modify: `CHANGELOG.md` — new `## 0.29.4` `- unreleased` block at top (per CONTRIBUTING header rules; `### Fixes` entry citing `(closes #23)`).

- [ ] **Step 1: Docs edit** (state the implemented semantics; no promises beyond it).
- [ ] **Step 2: Changelog block** (create once here; Tasks 6/12 append their entries).
- [ ] **Step 3: Commit + merge to trunk**: `docs: dst topic matches the relative-branch library semantics -- changelog opens 0.29.4`

### Task 4: #25 — manifest `[toolchain]` schema

**Files:**
- Modify: `src/core/manifest.rs` — new `Toolchain` struct (after `Dependencies` area, ~:277-310), `Manifest` gains `#[serde(skip_serializing_if = "Option::is_none", default)] toolchain: Option<Toolchain>`, `Manifest::new` (:429-452) gains the field, getters `get_toolchain() -> Option<&Toolchain>`; `Toolchain` getters `get_default()`, `get_build()`, `get_test()` (all `Option<&String>`)
- Test: inline `manifest.rs` test module (:801-922) — new fixture const `EX8`

**Interfaces:**
- Produces:
```rust
#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Toolchain {
    default: Option<String>,
    build: Option<String>,
    test: Option<String>,
}
```
- Produces: `Manifest::get_toolchain(&self) -> Option<&Toolchain>`; consumed by Task 5's `select_target`.

- [ ] **Step 1: Failing parse test** — `EX8` TOML with `[toolchain] default = "nvc" build = "nvc-build" test = "nvc"`; assert `from_str::<Manifest>(EX8)` yields the three values; assert `EX1` (minimal) yields `get_toolchain() == None`; assert serialization round-trip keeps `[toolchain]` and omits it when unset (pattern of `ut_serialize`).
- [ ] **Step 2: Run** `cargo test -p orbit --lib manifest` — expected: FAIL (no `toolchain` field).
- [ ] **Step 3: Implement** the struct + field + getters + `Manifest::new` init (`toolchain: None`).
- [ ] **Step 4: Run** — expected: PASS; full `cargo test` green.
- [ ] **Step 5: Commit** on branch `issue-25-toolchain-face`: `feat: the manifest gains the toolchain face -- default and per-verb target bindings (closes #25)`

### Task 5: #25 — resolution tiers, inference, capability face

**Files:**
- Modify: `src/core/context.rs:412-456` (`select_target`), new private helper `read_manifest_toolchain(&self) -> Option<Toolchain>` (light TOML extract from `self.get_project_path()`'s `Orbit.toml` — only the `toolchain` table, ignoring the rest of the document; any failure = `None`)
- Modify: `src/error.rs:57-81` — new variants `TargetWrongVerb(String, bool)` (message: `target "{0}" is not a {build|test} target (it declares {test|build} only)` + the matching `Hint::TargetsList{Build|Test}`); hints if needed
- Modify: `src/core/manifest.rs` `Manifest::from_str` error path — when the toml error string contains ``unknown field `target` ``, append the hint `\n\nhint: targets are defined in orbit config files, not the manifest; bind one per verb in a [toolchain] section instead`
- Test: inline context.rs test module (pattern of the `find_target_path` test at :477-506 using `tests/t5`); new fixture dir `tests/t8/` (config.toml with two targets + a project with `[toolchain]`)

**Interfaces:**
- Consumes: Task 4's `Manifest::get_toolchain`/`Toolchain` getters.
- Produces (resolution order inside `select_target`, unchanged signature): CLI `--target` > `ORBIT_TARGET` env > manifest `[toolchain]` per-verb key > manifest `default` > config `[build]`/`[test].target` > single-candidate inference (`get_targets(is_build).len() == 1`).
- Produces: wrong-verb detection — a selected name that exists in the UNFILTERED target set but is excluded by `can_build()`/`can_test()` → `TargetWrongVerb` (never reaches a target process). Names absent from the unfiltered set keep the existing `TargetNotFound*` errors.

- [ ] **Step 1: Failing tests in context.rs test module** — (a) manifest tier: fixture project with `[toolchain] test = "t2"` + config defining `t1`/`t2` → `select_target(&None, true, false)` resolves `t2`; (b) inference: config with exactly ONE build-able target, no faces → resolves it; (c) wrong verb: target declaring `build = false` selected for build → `Err(TargetWrongVerb)`; (d) name absent everywhere → still `TargetNotFoundBuild`.
- [ ] **Step 2: Run** — expected: FAIL (no tier/mismatch logic).
- [ ] **Step 3: Implement** the tiers + inference + wrong-verb face + the manifest-parse hint.
- [ ] **Step 4: Run** full `cargo test` + `just run-sys-tests` — expected: PASS (back-compat: unset faces change nothing).
- [ ] **Step 5: Commit**: `feat: targets resolve through the manifest toolchain face -- single candidates infer, wrong-verb picks die at the cli (closes #25)`

### Task 6: #25 — docs + changelog, merge

**Files:**
- Modify: `docs/src/reference/manifest.md` — `[toolchain]` section (keys, precedence, inference).
- Modify: `docs/src/reference/configuration.md` — `[build].target`/`[test].target` now the LAST tier; precedence chain documented.
- Modify: `docs/commands.toml` + regenerate manuals IF the `--target` prose names resolution (`tools/mansync.py` convention per CONTRIBUTING).
- Modify: `CHANGELOG.md` — `### Features` entry in the 0.29.4 block.

- [ ] **Step 1: Docs edits** (exact precedence chain from Task 5's Interfaces).
- [ ] **Step 2: Changelog entry.**
- [ ] **Step 3: Commit + merge to trunk**: `docs: the toolchain face is documented -- precedence chain and target hints`

### Task 7: #24 — manifest `[project] std` + `ORBIT_STD`

**Files:**
- Modify: `src/core/manifest.rs` `Package` (:608-644) — `std: Option<String>` + `get_std()`; load-time union-vocabulary check in `Manifest::from_file` after `is_deps_valid`: token must be one of `87 93 2008 2019 95 2001 2005 2012 2017` (else `Error::StdVocabulary(token, vocabulary)`)
- Modify: `src/util/environment.rs` — `pub const ORBIT_STD: &str = "ORBIT_STD";` (constants block ~:486); `from_project` (:323-369) gains the env entry when the root project declares std (swap variable `{{ orbit.std }}` derives automatically via `to_variable`)
- Modify: `src/error.rs` — `StdVocabulary(String, String)` (`unknown std "{0}" (expected one of: {1})`)
- Test: manifest.rs test module (`std` parse + vocabulary rejection), environment.rs tests (from_project exposes ORBIT_STD)

**Interfaces:**
- Produces: `Package::get_std(&self) -> Option<&String>`; `ORBIT_STD` env var + `{{ orbit.std }}` swap variable; the union vocabulary const shared with Task 9's per-language check.

- [ ] **Step 1: Failing tests** — EX8-style fixture with `std = "2008"` parses and `get_std() == Some("2008")`; `std = "banana"` → `StdVocabulary`; unset → `None`; `from_project` sets `ORBIT_STD` iff declared.
- [ ] **Step 2: Run** — FAIL; **Step 3: implement**; **Step 4: Run** — PASS + full suite green.
- [ ] **Step 5: Commit** on branch `issue-24-std-face`: `feat: projects declare their language standard -- carried to targets as orbit.std (closes #24)`

### Task 8: #24 — blueprint std column + per-file language check

**Files:**
- Modify: `src/core/blueprint.rs` — `JsonEntry` gains `std: String`; `Entry::to_string` TSV arm emits a 4th tab-separated field (owning project's std or empty)
- Modify: `src/commands/plan.rs` — the TSV/JSON emission site (`create_outputs` region :1833-1923) supplies `node.project.get_man().get_project().get_std()`; per-file vocabulary check: a `.vhd/.vhdl` row's std must be in `87 93 2008 2019`, a `.v/.vl/.vlg` row's in `95 2001 2005 2012`, an `.sv` row's in `2005 2012 2017` (else `Error::StdVocabulary(token, language)`)
- Test: blueprint.rs + plan.rs inline tests (row rendering with/without std; language mismatch rejection)

**Interfaces:**
- Consumes: Task 7's `get_std` + vocabulary const.
- Produces: TSV blueprint rows `fileset\tlibrary\tfilepath\tstd` (std column ALWAYS present; empty when undeclared) and JSON `std` key. This is the format orbit-drives Task 14 consumes.

- [ ] **Step 1: Failing tests** — Hdl node with `std = "93"` renders `VHDL\tlib\tpath\t93`; undeclared renders trailing `\t`; `.vhd` row with `std = "2012"` → `StdVocabulary`.
- [ ] **Step 2: Run** — FAIL; **Step 3: implement**; **Step 4: Run** — PASS + full suite + `just run-sys-tests` green (s1–s9 blueprint consumers may pin row shapes — update ONLY if their fixtures declare std; they do not, so their rows gain an empty trailing column: fix any exact-line assertions accordingly).
- [ ] **Step 5: Commit**: `feat: blueprint rows carry their project's std -- per-file vocabulary checked at emission (closes #24)`

### Task 9: #24 — per-target `mixed-std` + graph validation

**Files:**
- Modify: `src/core/target.rs:39-51` — `mixed_std: Option<bool>` (serde rename `mixed-std`) + `get_mixed_std() -> Option<bool>` + `forbids_mixed_std() -> bool` (`matches!(self.mixed_std, Some(false))` is FORBID; unset = tolerant)
- Modify: `src/commands/plan.rs` `Plan::run` — after `compute_final_project_graph` (:101): collect distinct declared stds across graph projects (root + nodes); if >1 AND `target.forbids_mixed_std()` → `Error::MixedStdForbidden(target name, project=std list)`
- Modify: `src/error.rs` — `MixedStdForbidden(String, String)`
- Modify: `docs/src/reference/limitations.md` — tokenizer honesty note (orbit's reference walk parses a union grammar, 2008-era citations)
- Test: target.rs deser test (`mixed-std = false` parses; unset → tolerant), plan.rs validation unit test

**Interfaces:**
- Produces: `Target::forbids_mixed_std(&self) -> bool`; the graph std scan helper (name: `collect_graph_stds(graph) -> Vec<(ProjectIdSpec, String)>`).

- [ ] **Step 1: Failing tests** — deser + validation (two-project graph, stds `93`/`2008`, strict target → `MixedStdForbidden`; tolerant target → Ok).
- [ ] **Step 2: Run** — FAIL; **Step 3: implement**; **Step 4: Run** — full suite + sys tests green.
- [ ] **Step 5: Commit**: `feat: targets declare the mixed-std discipline -- mixing graphs die loudly on strict targets (closes #24)`

### Task 10: #24 — docs + changelog; push fork

**Files:**
- Modify: `docs/src/reference/manifest.md` (`std` key), `docs/src/reference/blueprint.md` (std column), `docs/src/reference/environment_variables.md` (`ORBIT_STD`, `{{ orbit.std }}`), `CHANGELOG.md` (`### Features`)
- 

- [ ] **Step 1: Docs edits.** **Step 2: Changelog entry.**
- [ ] **Step 3: Verify changelog/Cargo version pairing** (`tools/evalver.py` + `.github/workflows/check-release.yml`): if the release check demands Cargo.toml == top changelog version, bump `Cargo.toml` to `0.29.4`; else leave 0.29.3.
- [ ] **Step 4: Full gate** — `cargo test`, `just run-sys-tests`, `cargo build --release`; **Step 5: merge to trunk, push**: `git push fork issue-24-std-face trunk` (all three branches + trunk up).

### Task 11: drives — install forked orbit + end-to-end probe (orbit side)

**Files:** none (environment + probe log into the spec's verdict table)

- [ ] **Step 1: Install**: `cargo install --path /home/nabil/claude-dev/electron-ic/orbit --force` (forked orbit on PATH).
- [ ] **Step 2: End-to-end probe** in a fresh `/tmp/e2e-std` project: default-target inference (`orbit build` with one target), `[toolchain]` binding (test=nvc, build=nvc-build), `[project] std = "93"` with a VHDL-93-shaped project → `orbit test --target nvc` passes with `ORBIT_STD` visible to the drive (assert via a shim command target first: `command = ["sh", "-c", "echo std=$ORBIT_STD"]`), `orbit test --target nvc-build` dies at the CLI (`TargetWrongVerb`), same-library relative dupes error, cross-library plans. Record every verdict in the spec's amendment table.

### Task 12: orbit-drives — blueprint std column + std consumption in drives

**Files:**
- Modify: `src/blueprint.rs` — `Entry` gains `std: Option<String>`; `parse_blueprint` accepts 3 or 4 fields (4th may be empty → `None`; >4 → `MalformedLine`); doc comment updated
- Create: `src/stdmap.rs` — token→flag tables per tool + `pub fn vhdl_flag(tool: Tool, token: &str) -> Option<String>` / `pub fn verilog_flag(tool: Tool, token: &str) -> Option<String>`; vocabularies mirror orbit's per-language sets
- Modify: `src/nvc.rs` — std resolution precedence: row std > `ORBIT_STD` > `ORBIT_DRIVES_NVC_STD` > pin (`--std=2008`); rows partitioned by (library, std) for analysis groups; update the :62-77 rationale comment (upstream face exists now)
- Modify: `src/ghdl.rs` — conditional `--std=<mapped>` per analysis group when a std is declared (row std > `ORBIT_STD`; unset = today's default-riding behavior, the 2026-11-08 un-pinning stands)
- Modify: `src/iverilog.rs` — `GEN` becomes resolved: row stds must agree (else exit-2: single-analysis-set tool) then mapped `-g<token>`; unset → `-g2012`
- Modify: `src/verilator.rs` — same single-set rule; `--language 1364-2005|1800-2017` mapping when declared; unset → today's flags
- Modify: `src/quartus.rs` — `STD` becomes resolved per row (vcom `-<token>` mapping); unset → `-2008` pin (documented-only rules respected)
- Modify: `src/yosys.rs` — honest comment: no std face (frontend default), row stds must agree or exit-2
- Modify: `src/cocotb.rs` — probe `build_args` acceptance in the runner script; if accepted, map ORBIT_STD→ghdl `--std` there; else loud honest exit-2 + book note
- Modify: `src/lib.rs` — `pub mod stdmap;`
- Test: unit tests per drive (mapping tables, precedence, single-set rule); `tests/nvc_shape.rs` + `tests/ghdl_shape.rs`-equivalent (orbit_shape.rs) gain a std-propagation case (gated live)

**Interfaces:**
- Consumes: orbit Task 8's 4-column TSV + `ORBIT_STD`.
- Produces: `stdmap::vhdl_flag(tool, token) -> Option<String>` / `stdmap::verilog_flag(tool, token) -> Option<String>` exact signatures pinned in-code; `Entry.std: Option<String>`.

- [ ] **Step 1: Failing parse tests** (blueprint 4-column + empty-4th + 5-column rejection) — **Step 2: run, FAIL** — **Step 3: implement parse** — **Step 4: green.**
- [ ] **Step 5: Failing stdmap/precedence unit tests per drive** — **Step 6: implement** the shared module + per-drive wiring (the per-drive wiring steps follow the order nvc → ghdl → iverilog → verilator → quartus → yosys → cocotb, each with its unit test green before the next).
- [ ] **Step 7: Live gated tests** `cargo test -- --include-ignored` (available tools run for real; absent honest-skip) — green.
- [ ] **Step 8: Commit(s)** per drive cluster: `feat: drives carry the std face -- row std and orbit.std resolve per tool, single-set tools refuse mixes`

### Task 13: orbit-drives — capability declarations + docs

**Files:**
- Modify: `config.toml` — every `[[target]]` gains its capability line (`test = true` / `build = false` on the six sim/test targets incl. cocotb; `build = true` / `test = false` on the seven build targets); header comment gains the `[toolchain]` example for project manifests
- Modify: `tests/config_shape.rs` — pin the capability lines (count 14 unchanged, ALLHDL line unchanged, capability key present in every block)
- Modify: `book/src/quickstart.md` + `README.md` — project `[toolchain]` snippet + std declaration + the new default/inference behavior
- Modify: `book/src/reference/env-faces.md` (ORBIT_STD row), `book/src/reference/honest-limits.md` (standards bullet rewrite: the orbit face exists; unset = tool defaults), `book/src/reference/probed-facts.md` (new argv rows for std faces probed in Task 14)
- Modify: `TODO.md` — park the cocotb/quartus ALLHDL raw-filter bug (`fileset != "VHDL"` bypass) + any cocotb std-face outcome

**Interfaces:**
- Consumes: orbit Task 5's wrong-verb face (config capabilities make it fire for the 14 shipped targets).

- [ ] **Step 1: config.toml + config_shape.rs in lockstep** (run `cargo test config_shape` green).
- [ ] **Step 2: book + README edits.** **Step 3: TODO parking.**
- [ ] **Step 4: Commit**: `feat: the 14 targets declare their capability -- docs teach the toolchain and std faces`

### Task 14: end-to-end + integration verdicts; merge + push both repos

- [ ] **Step 1: Full orbit-drives gate**: `cargo build --all-targets`, `cargo test`, `cargo test -- --include-ignored`.
- [ ] **Step 2: Live end-to-end with the forked orbit** (Task 11's probe extended): a project with `[toolchain] test = "nvc"` + `std = "93"` runs `orbit test` (no --target) through the real nvc drive with the std flag present in the spawned argv (assert via the drive's printed command face or a shim target).
- [ ] **Step 3: Spec verdict table** — append every amendment from Tasks 11–14.
- [ ] **Step 4: Merge orbit-drives branch to main; push origin; push fork trunk (if not already).** Final report states what shipped where.
