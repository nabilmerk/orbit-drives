# Examples

The worked invocations, one section per face family. Wire the target
fragment once; every drive then runs as an orbit target. Flag placement is per target: the cocotb drive takes its flags before
`--`; the build targets (vivado-build, quartus-build) take `--part` and
`--top` after `--` — each example below shows its target's shipped form.

## Wiring the fragment in

Include the shipped target fragment into an orbit project configuration:

```sh
orbit config --push include=/path/to/orbit-drives/config.toml
```

The raw face a target spawns, typed by hand against a plan file — the
shape every target entry carries:

```sh
orbit-drives nvc --blueprint plan.tsv --top tb_counter
```

## The nvc / ghdl test invocations

The judged VHDL sim faces. No `--top` is needed through `orbit test`:
the drive runs the testbench orbit exports as `$ORBIT_TB_NAME`.

```sh
orbit test --target nvc
orbit test --target ghdl -- -gW=4 --vcd=waves.vcd
```

The same judged drive against a chosen top, with a generic extra riding
the last `ghdl -r` argv:

```sh
orbit-drives ghdl --blueprint plan.tsv --top tb_counter --vcd=waves.vcd
```

## The vivado-build synthesis invocation

`--part` is REQUIRED on `vivado-build`: the gate fires before any drive,
exit 2 naming the flag — the probed alternative is a ~4s
synth-then-fail inside the tool. The target form:

```sh
orbit build --target vivado-build -- --part xc7z010clg400-1 --top top
```

The raw face — same flags, the blueprint in place of orbit's export:

```sh
orbit-drives vivado-build --blueprint plan.tsv --part xc7z010clg400-1 --top top
```

## The cocotb `--test-module` face

`--test-module` is REQUIRED on `cocotb` — a path to a `.py` file
carrying `@cocotb.test()` coroutines; absent, the gate fires before any
drive, exit 2 naming the flag. Its file stem names the module the runner
imports; its directory rides the spawned python's `PYTHONPATH`.

```sh
orbit test --target cocotb --test-module tests/tb_counter.py
```

The raw face, the test module named against a hand-run blueprint:

```sh
orbit-drives cocotb --blueprint plan.tsv --top tb_counter --test-module tests/tb_counter.py
```

Every face's exits mean the same thing: `0` clean, `1` findings, `2`
usage. See [the probed facts](reference/probed-facts.md) for each tool's
failure vocabulary and [honest limits](reference/honest-limits.md) for
what is still unprobed.
