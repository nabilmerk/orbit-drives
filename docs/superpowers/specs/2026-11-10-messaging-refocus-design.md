# Messaging refocus: HDL-first, not script-mechanics

- Date: 2026-11-10
- Status: approved (wording direction chosen by user)
- Type: docs + repo metadata

## Problem

Every front-door surface of orbit-drives leads with internal mechanics —
"probed failure vocabularies", "honest exits", the blueprint — instead of
the project's objective: the user should never again write or think about
the scripts around their EDA tools (Makefiles, Tcl, run glue); they write
HDL, orbit-drives runs the tools.

## Decision (user-approved)

**Pure benefit framing, no mechanics in the openings.** The mechanics
vocabulary stays in the body of the docs (book chapters, reference); only
the openings change.

### Exact new texts

**README.md** — replace the intro paragraph (the one starting "orbit-drives
removes the scripting…") with:

> orbit-drives exists so you never write — or think about — the scripts
> around your EDA tools again. No Makefiles, no Tcl, no run glue: describe
> your HDL, and orbit-drives runs it everywhere.

**book/src/index.md** — same replacement; the "Start with the
Tutorial…" line is kept.

**Cargo.toml** line 7:

```toml
description = "Stop maintaining scripts around your EDA tools. Write HDL, let orbit-drives run it."
```

**GitHub About** — the same sentence as Cargo.toml, applied with:

```sh
gh repo edit --description "Stop maintaining scripts around your EDA tools. Write HDL, let orbit-drives run it."
```

Outward-facing: executed only as the final step, after local surfaces are
committed.

## Scope

Four surfaces: `README.md`, `book/src/index.md`, `Cargo.toml` (description
only), GitHub About. Nothing below the openings changes.

## Non-goals

- No rewording of the book's inner chapters (mechanics voice stays there by design).
- No CI/docs-site build changes.

## Verification

1. `cargo build` parses the manifest (metadata-only change).
2. `grep -r "probed failure vocabularies" README.md book/src/index.md Cargo.toml` → no matches.
3. `gh repo view --json description` reflects the new sentence.

## Commits

Plain messages, no Claude co-author trailer (user rule).
