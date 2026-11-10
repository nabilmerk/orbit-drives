# Windows tool env for the tools CI matrix (promised set: ghdl iverilog yosys).
# nvc/verilator/cocotb stay WSL/POSIX-only per the README matrix -- their
# gated tests self-skip here, honestly.
$ErrorActionPreference = 'Stop'

# iverilog — the only one of our tools Chocolatey carries (checked 2026-11-10).
choco install iverilog -y --no-progress

# ghdl — official mcode build for Windows (ucrt64), not on Chocolatey.
$ghdl = (Invoke-RestMethod 'https://api.github.com/repos/ghdl/ghdl/releases/latest').assets |
  Where-Object { $_.name -match '^ghdl-mcode-.*-ucrt64\.zip$' } |
  Select-Object -First 1
Invoke-WebRequest $ghdl.browser_download_url -OutFile "$env:RUNNER_TEMP\ghdl.zip"
Expand-Archive "$env:RUNNER_TEMP\ghdl.zip" -DestinationPath "$env:RUNNER_TEMP\ghdl" -Force
"$(Get-ChildItem -Directory "$env:RUNNER_TEMP\ghdl" | Select-Object -First 1 -ExpandProperty FullName)\bin" |
  Add-Content $env:GITHUB_PATH

# yosys — OSS CAD Suite is the official Windows distribution (no choco package).
$oss = (Invoke-RestMethod 'https://api.github.com/repos/YosysHQ/oss-cad-suite-build/releases/latest').assets |
  Where-Object { $_.name -match '^oss-cad-suite-windows-x64-.*\.tgz$' } |
  Select-Object -First 1
Invoke-WebRequest $oss.browser_download_url -OutFile "$env:RUNNER_TEMP\oss-cad-suite.tgz"
tar -xzf "$env:RUNNER_TEMP\oss-cad-suite.tgz" -C "$env:RUNNER_TEMP"
"$env:RUNNER_TEMP\oss-cad-suite\bin" | Add-Content $env:GITHUB_PATH

# The promised set must answer on THIS step's PATH, or the script fails
# (the same directories just written to GITHUB_PATH, applied locally).
$env:PATH = "$env:RUNNER_TEMP\ghdl\$(Get-ChildItem -Directory "$env:RUNNER_TEMP\ghdl" | Select-Object -First 1 -ExpandProperty Name)\bin;$env:RUNNER_TEMP\oss-cad-suite\bin;$env:PATH"
ghdl --version | Select-Object -First 1
iverilog -V 2>&1 | Select-Object -First 1
yosys -V
