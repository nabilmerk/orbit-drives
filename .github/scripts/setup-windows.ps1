# Windows tool env for the tools CI matrix (promised set: ghdl iverilog yosys).
# nvc/verilator/cocotb stay WSL/POSIX-only per the README matrix -- their
# gated tests self-skip here, honestly.
$ErrorActionPreference = 'Stop'
# native tools (choco, tar, gh) must fail the script too, not just set $LASTEXITCODE
$PSNativeCommandUseErrorActionPreference = $true

# iverilog — the only one of our tools Chocolatey carries (checked 2026-11-10).
choco install iverilog -y --no-progress

# ghdl — official mcode build for Windows (ucrt64), not on Chocolatey.
# Zip root layout (verified against the v6.0.0 asset): bin/ include/ lib/.
# gh release download uses the workflow's GH_TOKEN — anonymous api.github.com
# calls from shared runner IPs hit the rate limit (second run's verdict).
# gh release download has no -O flag (that's curl): it lands files in --dir.
# no tag argument on gh release download means: latest release.
gh release download --repo ghdl/ghdl --pattern 'ghdl-mcode-*-ucrt64.zip' -D "$env:RUNNER_TEMP\ghdl-dl"
$ghdlZip = (Get-ChildItem "$env:RUNNER_TEMP\ghdl-dl\ghdl-mcode-*-ucrt64.zip" | Select-Object -First 1).FullName
Expand-Archive $ghdlZip -DestinationPath "$env:RUNNER_TEMP\ghdl" -Force
if (-not (Test-Path "$env:RUNNER_TEMP\ghdl\bin\ghdl.exe")) {
  throw "ghdl.exe missing after extract; zip root was: $((Get-ChildItem "$env:RUNNER_TEMP\ghdl" | Select-Object -First 8 | ForEach-Object Name) -join ', ')"
}
Add-Content $env:GITHUB_PATH "$env:RUNNER_TEMP\ghdl\bin"

# yosys — OSS CAD Suite is the official Windows distribution (no choco package).
# Tgz root layout (verified): oss-cad-suite/ wrapping bin/ etc.
gh release download --repo YosysHQ/oss-cad-suite-build --pattern 'oss-cad-suite-windows-x64-*.tgz' -D "$env:RUNNER_TEMP\oss-dl"
$ossTgz = (Get-ChildItem "$env:RUNNER_TEMP\oss-dl\oss-cad-suite-windows-x64-*.tgz" | Select-Object -First 1).FullName
tar -xzf $ossTgz -C "$env:RUNNER_TEMP"
if (-not (Test-Path "$env:RUNNER_TEMP\oss-cad-suite\bin\yosys.exe")) {
  throw "yosys.exe missing after extract; tgz root was: $((Get-ChildItem "$env:RUNNER_TEMP" | Where-Object Name -match 'oss' | ForEach-Object Name) -join ', ')"
}
Add-Content $env:GITHUB_PATH "$env:RUNNER_TEMP\oss-cad-suite\bin"

# The promised set must answer on THIS step's PATH, or the script fails
# (the same directories just written to GITHUB_PATH, applied locally).
$env:PATH = "$env:RUNNER_TEMP\ghdl\bin;$env:RUNNER_TEMP\oss-cad-suite\bin;$env:PATH"
ghdl --version | Select-Object -First 1
iverilog -V 2>&1 | Select-Object -First 1
yosys -V
