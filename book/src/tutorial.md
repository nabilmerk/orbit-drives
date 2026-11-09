# Tutorial: the first honest drive

This walkthrough runs a VHDL testbench through orbit and nvc, end to end.

## 1. Install

The drives ship as one binary:

```sh
cargo install --path .
```

You need [orbit](https://github.com/chaseruskin/orbit) on PATH, and nvc for this walkthrough.

## 2. Wire the drives into orbit

Include the shipped target fragment into your project's orbit configuration:

```sh
orbit config --push include="<orbit-drives>/config.toml"
```

This adds the drive targets (ghdl, nvc, iverilog, verilator, yosys, vivado,
quartus, cocotb) to every project.

## 3. The first run

From the project root:

```sh
orbit test --target nvc
```

orbit plans the blueprint; orbit-drives analyzes it in blueprint order and
runs the testbench.

## 4. Reading the verdict

The drive prints the tool's log, then one verdict line:

- `testbench: pass` — the log carries no probed failure face AND the tool exited 0.
- `testbench: fail (…)` — the log carries a failure face OR the exit was nonzero.

Either witness convicts: the judge exists because the exits lie — bare ghdl exits
0 on a violated `severity error`; a killed simulation can leave no face at all.
