#!/usr/bin/env bash
# macOS tool env for the tools CI matrix (promised set: ghdl iverilog verilator yosys cocotb).
set -euo pipefail

brew install llvm icarus-verilog verilator yosys

# ghdl is NOT in Homebrew (checked 2026-11-10: no core formula, no tap).
# Official llvm-backend build for Apple silicon; needs brew llvm. The tarball
# is NESTED (ghdl-llvm-*/bin ... -- verified against the v6.0.0 asset), so
# --strip-components=1 drops bin/ghdl straight onto /usr/local/bin. gh release
# download uses the workflow's GH_TOKEN -- anonymous api.github.com calls from
# shared runner IPs hit the rate limit (second run's verdict).
# gh release download has no -O flag: it lands files in --dir.
gh release download --repo ghdl/ghdl --pattern 'ghdl-llvm-*-macos15-aarch64.tar.gz' -D /tmp/ghdl-dl
sudo tar -xzf /tmp/ghdl-dl/ghdl-llvm-*-macos15-aarch64.tar.gz -C /usr/local --strip-components=1
rm -rf /tmp/ghdl-dl

# cocotb pinned to the probed version (--break-system-packages is a no-op
# when no EXTERNALLY-MANAGED marker exists). COCOTB_IGNORE_PYTHON_REQUIRES
# is cocotb's own escape hatch: runner python3 is newer than cocotb 2.1.0's
# metadata, so pip falls back to the sdist, whose check otherwise aborts.
COCOTB_IGNORE_PYTHON_REQUIRES=1 python3 -m pip install --break-system-packages cocotb==2.1.0

# The promised set must answer, or this script fails the job.
ghdl --version | head -1
iverilog -V 2>&1 | head -1
verilator --version
yosys -V
python3 -c "import cocotb; print('cocotb', cocotb.__version__)"
