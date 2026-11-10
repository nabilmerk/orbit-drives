# Tools CI: per-OS environments with every drive tool installed

- Date: 2026-11-10
- Status: approved (provisioning strategy chosen by user: ghcr image + setup scripts)
- Type: CI infrastructure + docs

## Problem

CI (`ci.yml`) builds the Rust crate on three OSes but never installs or calls
nvc, ghdl, verilator, iverilog, yosys, or cocotb. The gated `*_shape.rs`
tests self-skip when a tool is absent, so the drive recipes — the core
deliverable — are only ever exercised on machines that happen to have the
tools installed. Probed facts can rot silently.

## Goal

One environment per OS where every tool that OS claims is installed, so
`cargo test` in CI runs the gated tool tests instead of skipping them, and
a missing promised tool fails loudly.

## Decision: ghcr image (Linux) + setup scripts (Windows/macOS)

GitHub-hosted runners support custom container images only on Linux; the
equivalent for Windows/macOS is an idempotent setup script run as a CI step.

### The verified per-OS tool sets (every claim checked against its registry on 2026-11-10)

| Tool | Linux (Docker image) | Windows (`setup-windows.ps1`) | macOS (`setup-macos.sh`) |
|---|---|---|---|
| ghdl | official `6.0.0` ubuntu24.04 tarball from ghdl/ghdl releases (the version config.toml probes) | `ghdl-mcode-*-ucrt64.zip` from the same releases | `ghdl-llvm-*-macos15-aarch64.tar.gz` (Apple-silicon runners) |
| nvc | `apt install nvc` (present in noble ✓) | not installed (README matrix: WSL) | not installed (README matrix: no) |
| iverilog | `apt install iverilog` ✓ | `choco install iverilog` ✓ | `brew install icarus-verilog` ✓ |
| verilator | `apt install verilator` ✓ | not installed (WSL) | `brew install verilator` ✓ |
| yosys | `apt install yosys` ✓ | OSS CAD Suite `oss-cad-suite-windows-x64-*.tgz` from YosysHQ releases ✓ | `brew install yosys` ✓ |
| cocotb | `pip install cocotb==2.1.0` (the probed version, needs `python3-pip`) | not installed (POSIX-only) | `pip3 install cocotb==2.1.0` ✓ |

Vendor tools (Vivado, Quartus) stay out — licensed, tens of GB; they remain
local-probed per the README table.

### File layout

```
.github/
├── workflows/
│   ├── ci.yml            (unchanged)
│   └── tools.yml         (new)
├── docker/
│   └── linux.Dockerfile  (published as ghcr.io/<owner>/orbit-drives-ci-linux)
└── scripts/
    ├── setup-windows.ps1
    └── setup-macos.sh
```

### tools.yml behavior

- Triggers: push/PR to `main` on `src/**`, `tests/**`, `Cargo.toml`,
  `.github/workflows/tools.yml`, `.github/docker/**`, `.github/scripts/**`;
  weekly cron (Monday 06:00 UTC) as the rot detector; `workflow_dispatch`.
- Job `image` (ubuntu-latest): builds `linux.Dockerfile`, pushes to ghcr
  with `:latest` + `:${{ github.sha }}` tags, gha layer cache. On PRs it
  builds but does not push (Dockerfile validity proven; the rebuilt image
  is exercised after merge).
- Job `linux` (needs image): `container:` the ghcr image; asserts all six
  binaries; `cargo test`.
- Job `windows`: runs `setup-windows.ps1`; asserts ghdl/iverilog/yosys on
  PATH; `cargo test` (nvc/verilator/cocotb tests self-skip — honest).
- Job `macos`: runs `setup-macos.sh`; asserts ghdl/iverilog/verilator/
  yosys/cocotb; `cargo test`.
- Assertions are explicit steps (`ghdl --version`, `nvc --version`,
  `iverilog -V 2>&1 | head -1`, `verilator --version`, `yosys -V`,
  `python3 -c "import cocotb; print(cocotb.__version__)"` or PowerShell
  `Get-Command` equivalents) so a vanished package is a red CI step, not a
  quiet skip.

### GHDL release-pinning policy

ghdl downloads use the **latest v\* release** of ghdl/ghdl at run time
(pinned today at 6.0.0, matching the probed facts) — weekly cron re-probes
new releases; if a future ghdl breaks a recipe, CI goes red and the facts
get re-probed. OSS CAD Suite likewise uses the latest windows asset.

## Verification

1. Local (what can run here): YAML parses (`python3 -c yaml.safe_load`),
   `bash -n setup-macos.sh`, Dockerfile lint by reading; PowerShell script
   reviewed line-by-line (no pwsh on this machine — honest limit, first
   real proof is CI).
2. Remote (the real proof): after push, `gh workflow run tools.yml`, then
   `gh run watch` — all three jobs green, each gated suite reports 0
   skipped-for-missing-tool within its promised set.

## Docs

README `## Development` gains a short paragraph: the tools matrix runs on
CI with the per-OS sets, image on ghcr, weekly re-probe.

## Commits

Plain messages, no Claude co-author. Finish = merge to main + push, then
dispatch and watch the first run; fix forward if a script trips (that is
the weekly cron's job going forward).

## Verdicts and amendments (CI rounds 1–3, 2026-11-10)

Each CI run produced facts that amended the design above. Recorded here
so the spec stays the truth, not the first draft.

| # | Verdict (evidence) | Amendment |
|---|---|---|
| 1 | gha cache export rejected: `Cache export is not supported for the docker driver` | `image` job gains `docker/setup-buildx-action@v3` (container driver) before build-push |
| 2 | apt: `Unable to locate package nvc` — nvc is NOT in noble; the earlier packages.ubuntu.com "200" was a soft-404 ("No such package" page, content never read) | nvc comes from nickg's official `nvc_1.23.1-1_amd64_ubuntu-24.04.deb` (pinned `ARG NVC_VERSION`); registry checks must read page content, not status codes |
| 3 | apt: deb's `libdw1t64` "not installable" inside the container although the deb's four named deps all exist in noble (content-verified) | deb's own Depends list installed explicitly by name (fresh `apt-get update` layer), then `dpkg -i` — a real gap now names itself |
| 4 | windows: anonymous `api.github.com` from shared runner IPs → `API rate limit exceeded` (also explains run 1's silent 27 s window) | all release fetches go through token-authenticated `gh release download --latest` (`GH_TOKEN: ${{ github.token }}` on the script steps); "latest" semantics kept per the pinning policy |
| 5 | windows/macos: `gh release download` has no `-O` flag (that's curl) | files land in `-D/--dir`, then resolved by glob |
| 6 | macos: `ghdl: command not found` — the ghdl macOS tarball is NESTED (`ghdl-llvm-*/bin/…`), so `-C /usr/local` buried the binary | extract with `--strip-components=1` (layout verified against the v6.0.0 asset) |
| 7 | macos: cocotb sdist aborted by cocotb's own python_requires check (runner python3 newer than 2.1.0 metadata); with the escape hatch it installed fine | `COCOTB_IGNORE_PYTHON_REQUIRES=1` kept — cocotb's official escape hatch |
| 8 | windows run 1: choco+assert log with a silent 27 s middle — native tools (`tar`, `choco`, `gh`) don't trigger `$ErrorActionPreference` | `$PSNativeCommandUseErrorActionPreference = $true` + literal paths (`bin\ghdl.exe`, `oss-cad-suite\bin\yosys.exe`) guarded by `Test-Path` throws that print directory contents |
| 9 | run 4: image **builds and pushes** (apt, ghdl tarball, nvc deb, cocotb all green); linux job's asserts all pass, then `linker 'cc' not found` — `--no-install-recommends` ships no C toolchain | `gcc` added to the image apt list |
| 10 | run 4: `gh release download` rejects `--latest` (Usage error) — the flag doesn't exist; a bare tag argument means latest | `--latest` dropped from both scripts; behavior unchanged |

Windows/macOS/Linux layout facts (zip root `bin/ include/ lib/`; tgz root
`oss-cad-suite/`; ghdl macos tarball nested) were verified by downloading
the actual release assets, not assumed.
