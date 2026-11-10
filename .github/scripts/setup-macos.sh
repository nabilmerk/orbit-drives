#!/usr/bin/env bash
# macOS tool env for the tools CI matrix (promised set: ghdl iverilog verilator yosys cocotb).
set -euo pipefail

brew install llvm icarus-verilog verilator yosys

# ghdl is NOT in Homebrew (checked 2026-11-10: no core formula, no tap).
# Use the official llvm-backend build for Apple silicon; needs brew llvm.
GHDL_URL=$(curl -fsSL https://api.github.com/repos/ghdl/ghdl/releases/latest \
  | python3 -c "import json,sys
assets = json.load(sys.stdin)['assets']
print(next(a['browser_download_url'] for a in assets
           if 'llvm' in a['name'] and a['name'].endswith('macos15-aarch64.tar.gz')))")
curl -fsSL -o /tmp/ghdl.tgz "$GHDL_URL"
sudo tar -xzf /tmp/ghdl.tgz -C /usr/local
rm /tmp/ghdl.tgz

# cocotb pinned to the probed version (--break-system-packages is a no-op
# when no EXTERNALLY-MANAGED marker exists).
python3 -m pip install --break-system-packages cocotb==2.1.0

# The promised set must answer, or this script fails the job.
ghdl --version | head -1
iverilog -V 2>&1 | head -1
verilator --version
yosys -V
python3 -c "import cocotb; print('cocotb', cocotb.__version__)"
