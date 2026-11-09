# orbit-drives

orbit-drives removes the scripting from your orbit targets: orbit plans
your dependencies into a blueprint — the drives execute it, tool by tool,
each carrying its probed failure vocabulary and an honest exit.

The full guide: **[The orbit-drives Guide](https://nabilmerk.github.io/orbit-drives/)**.

## Install

```sh
cargo install --path .
```

No dependencies beyond the Rust standard library.

## Quick start

```sh
orbit config --push include="<orbit-drives>/config.toml"
orbit test --target nvc
```

The [Tutorial](https://nabilmerk.github.io/orbit-drives/tutorial.html) walks
the first run end to end.

## The tools

| drive | tool | Linux | Windows | macOS |
|---|---|---|---|---|
| ghdl | GHDL | yes | yes | yes |
| nvc | nvc | yes | no (WSL) | no |
| iverilog | Icarus Verilog | yes | yes | yes |
| verilator | Verilator | yes | WSL | yes* |
| yosys | yosys | yes | yes | yes |
| vivado | Vivado xsim/synth | yes (probed) | untested | untested |
| quartus | Quartus Questa | untested | untested | untested |
| cocotb | cocotb + any simulator | yes | no (POSIX-only) | yes* |

`*` = the tool builds exist; the drive's recipe is probed on Linux only.
The probed facts and the honest limits live in the
[Reference](https://nabilmerk.github.io/orbit-drives/reference/probed-facts.html).

## Development

Build and test (the gated live tests need the vendor tools; the unit and
shape tests run anywhere):

```sh
cargo build --all-targets
cargo test
```

The per-OS matrix runs on CI for `src/**`, `tests/**`, `Cargo.toml` changes.

## Status

- [TODO.md](TODO.md) — the standing deferred items.
- MIT OR Apache-2.0, same as Rust.
