FROM ubuntu:24.04

# git: actions/checkout needs it inside containers. curl: toolchain bootstrap.
# iverilog/verilator/yosys are in noble universe. nvc is NOT (noble has no
# such package -- checked 2026-11-10); it comes from nickg's official .deb.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl git \
      python3 python3-pip \
      iverilog verilator yosys \
 && rm -rf /var/lib/apt/lists/*

# nvc — the official ubuntu-24.04 .deb from the nickg/nvc release (apt pulls
# its declared deps, e.g. llvm, from the normal repos).
ARG NVC_VERSION=1.23.1
RUN curl -fsSL -o /tmp/nvc.deb \
      "https://github.com/nickg/nvc/releases/download/r${NVC_VERSION}/nvc_${NVC_VERSION}-1_amd64_ubuntu-24.04.deb" \
 && apt-get install -y /tmp/nvc.deb \
 && rm /tmp/nvc.deb \
 && nvc --version

# ghdl 6.0.0 — the official ubuntu24.04 build (the version the probed facts pin).
# Tarball layout is bin/ lib/ include/ at the root; /usr/local merges it onto PATH.
ARG GHDL_VERSION=6.0.0
RUN curl -fsSL -o /tmp/ghdl.tgz \
      "https://github.com/ghdl/ghdl/releases/download/v${GHDL_VERSION}/ghdl-mcode-${GHDL_VERSION}-ubuntu24.04-x86_64.tar.gz" \
 && tar -xzf /tmp/ghdl.tgz -C /usr/local \
 && rm /tmp/ghdl.tgz \
 && ghdl --version | head -1

# cocotb 2.1.0 — the probed runner version (cocotb_tools distribution).
RUN python3 -m pip install --break-system-packages cocotb==2.1.0 \
 && python3 -c "import cocotb; print('cocotb', cocotb.__version__)"
