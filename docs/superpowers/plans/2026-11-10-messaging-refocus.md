# Messaging Refocus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mechanics-first openings of orbit-drives' four front-door surfaces with the approved pure-benefit framing: "never write or think about the scripts around your EDA tools — describe HDL, orbit-drives runs it."

**Architecture:** Docs + metadata only. Three local surfaces (README intro, book landing intro, Cargo.toml `description`) get exact verbatim text from the spec, each as its own committed task; the outward-facing GitHub About is the final task and only runs after all local commits land.

**Tech Stack:** Markdown, TOML, `gh` CLI for the repo description.

**Spec:** `docs/superpowers/specs/2026-11-10-messaging-refocus-design.md`

## Global Constraints

- **No `Co-Authored-By` (or any Claude attribution) in any commit** — user rule, overrides all defaults.
- Openings carry pure-benefit wording only: the strings "blueprint", "probed", "honest exit" must NOT appear in the four surfaces' opening text (spec: Decision).
- Only openings change; nothing below the README intro paragraph, below the book's intro paragraph, or other `Cargo.toml` keys.
- README and book lines wrapped at ≤ 80 chars (matches both files' current style).
- GitHub About (`gh repo edit`) is the LAST task — outward-facing, spec requires local commits to land first.
- Commit style follows repo history: lowercase conventional type, form `type: the <change> -- the <detail>`.

---

### Task 1: README intro — benefit-lead

**Files:**
- Modify: `README.md:3-5`

**Interfaces:**
- Consumes: approved text from spec §Decision (verbatim below).
- Produces: nothing downstream depends on this.

- [ ] **Step 1: Replace the intro paragraph**

Replace exactly:

```markdown
orbit-drives removes the scripting from your orbit targets: orbit plans
your dependencies into a blueprint — the drives execute it, tool by tool,
each carrying its probed failure vocabulary and an honest exit.
```

with exactly:

```markdown
orbit-drives exists so you never write — or think about — the scripts
around your EDA tools again. No Makefiles, no Tcl, no run glue: describe
your HDL, and orbit-drives runs it everywhere.
```

The `# orbit-drives` title above and the "The full guide:" line below stay untouched.

- [ ] **Step 2: Verify**

Run: `head -8 README.md && ! grep -q "probed failure" README.md && echo CLEAN`
Expected: new paragraph visible, `CLEAN` printed.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: the readme leads with the objective -- no mechanics in the opening"
```

---

### Task 2: Book landing intro — same benefit text

**Files:**
- Modify: `book/src/index.md:3-5`

**Interfaces:**
- Consumes: same approved text as Task 1 (verbatim below).
- Produces: nothing downstream.

- [ ] **Step 1: Replace the intro paragraph**

Replace exactly:

```markdown
orbit-drives removes the scripting from your [orbit](https://github.com/chaseruskin/orbit)
targets: orbit plans your dependencies into a blueprint — the drives execute it, tool by
tool, each carrying its probed failure vocabulary and an honest exit.
```

with exactly:

```markdown
orbit-drives exists so you never write — or think about — the scripts
around your EDA tools again. No Makefiles, no Tcl, no run glue: describe
your HDL, and orbit-drives runs it everywhere.
```

The `# The orbit-drives Guide` title and the "Start with the Tutorial…" line stay untouched.

- [ ] **Step 2: Verify**

Run: `head -8 book/src/index.md && ! grep -q "probed failure" book/src/index.md && echo CLEAN`
Expected: new paragraph visible, `CLEAN` printed.

- [ ] **Step 3: Commit**

```bash
git add book/src/index.md
git commit -m "docs: the book landing leads with the objective -- same benefit text"
```

---

### Task 3: Cargo.toml description

**Files:**
- Modify: `Cargo.toml:7`

**Interfaces:**
- Consumes: approved sentence from spec §Decision.
- Produces: Task 4 uses the identical sentence for the GitHub About.

- [ ] **Step 1: Replace the description value**

Replace exactly:

```toml
description = "Probed failure vocabularies and honest-exit targets for the orbit HDL package manager"
```

with exactly:

```toml
description = "Stop maintaining scripts around your EDA tools. Write HDL, let orbit-drives run it."
```

- [ ] **Step 2: Verify the manifest still parses**

Run: `cargo build 2>&1 | tail -3`
Expected: build finishes (incremental; no compile errors).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: the cargo description sells the objective -- one sentence"
```

---

### Task 4: GitHub About (outward-facing — final task only)

**Files:**
- None (remote repo metadata, via `gh`).

**Interfaces:**
- Consumes: the Task 3 sentence, verbatim; all three local commits already pushed/committed on `main`.

- [ ] **Step 1: Update the repo description**

```bash
gh repo edit --description "Stop maintaining scripts around your EDA tools. Write HDL, let orbit-drives run it."
```

- [ ] **Step 2: Verify**

Run: `gh repo view --json description --jq .description`
Expected: `Stop maintaining scripts around your EDA tools. Write HDL, let orbit-drives run it.`

- [ ] **Step 3: Report**

No commit — nothing local changed. Include the verified description in the final report.
