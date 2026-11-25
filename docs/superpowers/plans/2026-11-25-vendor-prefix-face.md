# Vendor-Prefix Face Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The ghdl drive detects external vendor-library references from the blueprint alone and either wires `ORBIT_DRIVES_GHDL_VENDOR_PREFIX` as `GHDL_PREFIX` on every spawn or exits 2 with a provisioning hint.

**Architecture:** A new `src/vendor.rs` scanner derives the external-library set from the blueprint entries (comment-stripped use/library clauses vs the blueprint's own libraries plus the predefined set); `src/ghdl.rs` gains the env face (resolver + spawn wiring on all three spawn sites) and a pre-analysis gate whose hint exits through the usage-class error path.

**Tech Stack:** std-only Rust edition 2021; the crate's `Entry`/`DriveError`/print-block conventions.

**Spec:** `docs/superpowers/specs/2026-11-25-vendor-prefix-face-design.md`

## Global Constraints

- Every cargo command runs under `systemd-run --user --scope -p MemoryMax=4000M nice -n 19` (box rule).
- Commit messages carry no attribution lines (owner rule). Lowercase conventional subjects in the family style: `type: the <change> -- the <detail>`.
- The scanner gates wiring and hints only; it never rejects a file, reorders rows, or validates the prefix directory.
- The hint's exit class is usage (2), per `src/contracts.rs`.
- Temp-dir fixtures key by test name (the parked pid-keyed collision).

## Review Focus

- A commented `-- use unisim.foo;` line must not count: comment stripping runs before tokenizing -- pinned by the scanner test with a comment-only fixture.
- Case variance (`use UNISIM.VCOMPONENTS.all;`, `library UNISIM;`) must resolve to `unisim` and diff case-insensitively against the blueprint's libraries -- pinned by the scanner test with mixed-case fixtures.
- A referenced library that IS a blueprint library (`gaisler`) must not surface as external -- pinned by the scanner test diffing against entry libraries.
- The gate must not fire when the environment already carries a non-empty `GHDL_PREFIX` (out-of-band provisioning) -- pinned by the gate test's third arm.
- A garbage prefix path must not be validated by the drive: the face wires the env verbatim and ghdl's own error convicts the run -- pinned by the resolver test asserting verbatim passage (no canonicalization).

---

### Task 1: The scanner (`src/vendor.rs`)

**Files:**
- Create: `src/vendor.rs`
- Modify: `src/lib.rs` (add `pub mod vendor;` after `pub mod stdmap;`)
- Test: inline `#[cfg(test)]` in `src/vendor.rs`

**Interfaces:**
- Produces: `pub fn external_libraries(entries: &[Entry]) -> Vec<String>` -- sorted, deduped, lowercase library names referenced by the entries' files but absent from the entries' `library` fields and `PREDEFINED_LIBS`.
- Produces: `pub const PREDEFINED_LIBS: [&str; 3]` = `["ieee", "std", "work"]`.

- [ ] **Step 1: Write the failing tests** in `src/vendor.rs`'s test module. Fixtures live in `std::env::temp_dir().join(concat!("orbit_vendor_", env!("CARGO_PKG_NAME"), "_external_libraries"))` (test-name-keyed per the parked collision). Three fixture files + one blueprint each:
  - `external_case.vhd`: `use UNISIM.VCOMPONENTS.all;` + `library UNISIM;` + an `entity e is end;` -- with entries whose `library` is `"techmap"`; expects `["unisim"]`.
  - `comment_only.vhd`: `-- use unisim.something;` as the only reference -- expects `[]`.
  - `local_ref.vhd`: `use gaisler.arith.all;` with an entry whose `library` is `"gaisler"` -- expects `[]`.
  Also assert the returned Vec is sorted and the predefined set constant is `["ieee", "std", "work"]`.
- [ ] **Step 2: Run** `systemd-run --user --scope -p MemoryMax=4000M nice -n 19 cargo test -p orbit-drives --lib vendor` -- expected: FAIL (module absent; create the module skeleton with `todo!()` bodies so the test compiles and fails at runtime, or expect the compile failure as the RED state).
- [ ] **Step 3: Implement** `external_libraries`: for each entry, `fs::read` + `from_utf8_lossy`; per line, truncate at the first `--`; `split_whitespace`; on a token equal (ASCII-case-insensitive) to `use` take the next token and cut at the first `.`; on `library` take the next token trimmed of `;` and whitespace; lowercase, drop empties; subtract `PREDEFINED_LIBS` and the entries' lowercased `library` fields; dedupe and sort.
- [ ] **Step 4: Run** the vendor tests -- expected: PASS.
- [ ] **Step 5: Commit**: `feat: the blueprint scan names the external libraries -- use and library clauses vs the carried set`

### Task 2: The ghdl face -- resolver and spawn wiring

**Files:**
- Modify: `src/ghdl.rs` (face consts near `ASSERT_LEVEL_ENV` ~line 124; resolvers near `assert_level_from` ~line 127; spawn sites `analyze_once` ~line 575, `run_top` ~line 554, the elaborate spawn ~line 472)
- Test: inline `#[cfg(test)]` in `src/ghdl.rs`

**Interfaces:**
- Produces: `pub const VENDOR_PREFIX_ENV: &str = "ORBIT_DRIVES_GHDL_VENDOR_PREFIX";`
- Produces: `fn vendor_prefix() -> Option<String>` (trimmed env value; empty = None).
- Produces: `fn apply_vendor_prefix(command: &mut Command)` -- sets `GHDL_PREFIX` to the resolved value when `Some`.

- [ ] **Step 1: Write the failing tests**: `vendor_prefix_from`-style resolver behavior through `apply_vendor_prefix` -- build a `Command::new("true")`, call `apply_vendor_prefix` with the env var set (via `std::env::set_var` in the test, restoring after) to a known dir, assert `command.get_env("GHDL_PREFIX") == Some(OsStr::new(dir))`; unset env asserts `get_env` stays `None`; a whitespace-only env value asserts `None` (verbatim passage, no canonicalization -- the Review Focus garbage-path pin).
- [ ] **Step 2: Run** `cargo test -p orbit-drives --lib ghdl` -- expected: FAIL (face absent).
- [ ] **Step 3: Implement** the const, resolver, and `apply_vendor_prefix`; call `apply_vendor_prefix` in the three spawn sites (`analyze_once`, the elaborate spawn, `run_top`) before `capture`.
- [ ] **Step 4: Run** -- expected: PASS; full `cargo test -p orbit-drives --lib` green.
- [ ] **Step 5: Commit**: `feat: the ghdl face carries the vendor prefix -- the provisioned dir rides ghdl_prefix on every spawn`

### Task 3: The gate and its wiring into `drive()`

**Files:**
- Modify: `src/ghdl.rs` (`DriveError` enum ~line 288 area: new variant; `drive()` ~line 373: the gate after `carried` at ~line 409, before `analyze_with_deferral` at ~line 430)
- Modify: `src/contracts.rs` (pin the hint's exit class if the contract table enumerates error variants -- read it first and follow its pattern)
- Test: inline `#[cfg(test)]` in `src/ghdl.rs`

**Interfaces:**
- Produces: `fn vendor_gate(externals: &[String], face_set: bool, env_prefix_set: bool) -> Result<(), String>` -- `Ok` unless `!externals.is_empty() && !face_set && !env_prefix_set`; the `Err` payload names the libraries and the recipe.
- Produces: `DriveError::Vendor(String)` whose Display carries the hint; exit class usage (2).

- [ ] **Step 1: Write the failing tests**: `vendor_gate` arms -- externals + unset face + unset env prefix = `Err` naming `unisim`; externals + face set = `Ok`; externals + env prefix set = `Ok`; empty externals = `Ok` regardless.
- [ ] **Step 2: Run** -- expected: FAIL.
- [ ] **Step 3: Implement** `vendor_gate`, `DriveError::Vendor`, and the `drive()` wiring: after `carried` computes, `let externals = vendor::external_libraries(&carried);` then match on `vendor_prefix()`/`GHDL_PREFIX` presence: provisioned paths print one `orbit-drives: ghdl: {label}: vendor prefix: <dir> (external libraries: <set>)` line (or the `note:` line when only the env prefix exists) and call `apply_vendor_prefix`'s env read per spawn; unprovisioned path prints the hint via `eprintln!("orbit-drives: ghdl: error: {message}")` and returns `Err(DriveError::Vendor(message))`. Check `contracts.rs` first: if it pins DriveError variants to exit classes, register the new variant there.
- [ ] **Step 4: Run** -- expected: PASS; full `cargo test -p orbit-drives --lib` green; `cargo test -p orbit-drives --test contracts` green (exit-class contract).
- [ ] **Step 5: Commit**: `feat: the ghdl gate refuses unprovisioned vendor references -- the hint names the libraries and the recipe`

### Task 4: Docs and record

**Files:**
- Modify: `book/src/reference/env-faces.md` (row after `ORBIT_DRIVES_GHDL_ASSERT_LEVEL`; a short recipe paragraph: ghdl's `scripts/vendors/compile-xilinx-vivado.sh --unisim --vhdl93` over a vendor source tree, consumed via `GHDL_PREFIX` layout `<prefix>/<lib>/v93/`)
- Modify: `book/src/reference/probed-facts.md` (the loader search pattern row, probed 2026-11-25)
- Modify: `TODO.md` + the campaign spec's three-face round section: the face shipped (the manual env now a face)

- [ ] **Step 1: env-faces row + recipe paragraph.** **Step 2: probed-facts row.** **Step 3: TODO/spec touches.**
- [ ] **Step 4: Commit**: `docs: the vendor prefix face is documented -- the recipe and the loader pattern`

### Task 5: Gates, the zedboard probe, merge and push

- [ ] **Step 1:** `systemd-run --user --scope -p MemoryMax=4000M nice -n 19 cargo build --all-targets` clean; `cargo test`; `cargo test -- --include-ignored` (ghdl/nvc live shapes unaffected -- no vendor refs in their fixtures).
- [ ] **Step 2: The zedboard probe, both arms**, in the demo work dir against `ghdl-clean`:
  - provisioned: `ORBIT_DRIVES_GHDL_VENDOR_PREFIX=$PWD/../ghdl-prefix` (no manual `GHDL_PREFIX`) -- expected `orbit-drives: ghdl: sim: testbench: pass`, the vendor-prefix note line in the output.
  - unprovisioned: the face and `GHDL_PREFIX` unset -- expected exit 2 with the hint naming `unisim` before any analysis line.
- [ ] **Step 3:** merge the branch to `main`, push `origin main`.
- [ ] **Step 4: Commit** (if the merge needs a message): `merge: the vendor prefix face -- the drives-side external-library detection`
