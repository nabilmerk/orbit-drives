//! The `orbit-contracts` binary: argv in, honest exit out.
//!
//! ```text
//! orbit-contracts <SUBCOMMAND> --blueprint <path> --top <name> [extra tool args...]
//! ```
//!
//! Orbit spawns the target with the working directory set to the target's
//! output directory and `ORBIT_BLUEPRINT` pointing at the plan file, so
//! the default blueprint is the environment's; `--blueprint` overrides it
//! for direct drives. Orbit consumes its own `--` separator and appends
//! the user's extras straight onto the target's argv
//! (`src/core/target.rs` of orbit: `[&arguments, extra_args].concat()`),
//! so there is no separator to find here: the drive's own flags are
//! `--blueprint`, `--top`, `-h`/`--help`, and EVERY other token rides the
//! drive's last tool argv verbatim. With no `--top`, the drive runs the
//! testbench orbit exports as `$ORBIT_TB_NAME` (empty in all-mode, which
//! counts as absent); it is a loud exit-2 when neither names a top.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use orbit_contracts::{blueprint, ghdl, iverilog, nvc, verilator};

const USAGE: &str = "\
orbit-contracts: the probed failure vocabularies and the honest-exit targets for orbit

USAGE:
    orbit-contracts <SUBCOMMAND> [SUBCOMMAND OPTIONS] [EXTRA TOOL ARGS...]

SUBCOMMANDS:
    ghdl         analyze the blueprint's VHDL in blueprint order, elaborate
                 and run each top, and gate every run log on ghdl's probed
                 failure vocabulary AND ghdl's exit (the ghdl sim face)
    ghdl-build   analyze the blueprint's VHDL in blueprint order and
                 elaborate each top, exit honest; no simulation and
                 nothing judged -- the elaboration exits are honest, so
                 the exit is the only witness a build needs (the ghdl
                 build face)
    nvc          the same judged sim drive, on nvc's probed failure
                 vocabulary (the nvc sim face)
    nvc-build    the same honest build face, on nvc (the nvc build face)
    iverilog     compile the blueprint's Verilog into one snapshot, run it
                 once through vvp, and gate the run log on iverilog's
                 probed failure vocabulary AND vvp's exit (the iverilog
                 sim face; vvp exits 0 on a $error call, so the judge is
                 the load-bearing gate there)
    iverilog-build
                 the one order-free compile only (every --top a -s root),
                 exit honest; no simulation and nothing judged -- the
                 compile exits are honest, and iverilog validates every
                 -s root name at compile time (the iverilog build face)
    verilator    lint the blueprint's Verilog once, build one simulation
                 model per top (--binary --build --timing into obj_dir),
                 run each model, and gate every run log on verilator's
                 probed failure vocabulary AND the model's exit (the
                 verilator sim face; every probed model failure dies by
                 SIGABRT, so the exit convicts and the judge is the
                 second witness)
    verilator-build
                 the lint and the per-top model builds only, exit
                 honest; no simulation and nothing judged -- the lint
                 exits are honest (exit 1 on a lint error or an enabled
                 warning, probed live) and every top is validated by
                 its own per-top build: verilator honors only the LAST
                 --top-module on the lint, so an unknown top fails its
                 build with exit 1 (the verilator build face)

SHARED OPTIONS:
    --blueprint <path>  the TSV blueprint (default: $ORBIT_BLUEPRINT)
    --top <name>        a top unit to elaborate (and run: the sim
                        faces); repeatable
    -h, --help          print this help

TOPS:
    with no --top, the drive runs the testbench orbit exports as
    $ORBIT_TB_NAME (empty in all-mode counts as absent); exit 2 when
    neither names a top

EXTRAS:
    every other argument rides the drive's LAST tool argv verbatim: the
    `ghdl -r` / `nvc -r` / `vvp` / model argv on the sim faces, the
    `ghdl -e` / `nvc -e` / `iverilog` compile / verilator build argv on
    the build faces (there is no run to carry them, and a dropped
    argument would be a silent degradation). Orbit strips its own `--`
    separator and appends the extras straight onto this argv, so `orbit
    test --target ghdl -- -gW=4 --vcd=w.vcd` passes both through to ghdl

EXITS:
    0    clean (every run judged clean, or every top elaborated clean)
    1    findings (a log carried a failure face, a run exited nonzero, or
         the tool reported a broken design)
    2    usage (bad argv, no top to run, unreadable blueprint, a fileset
         the drive does not carry)
";

/// The subcommands the binary carries: one per orbit target entry, each a
/// face of a probed tool's drive. A name outside this registry is a loud
/// usage error, never a guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Subcommand {
    /// `ghdl-build`: ghdl's build face -- analyze + elaborate per top,
    /// exit honest, no simulation, nothing judged.
    GhdlBuild,
    /// `ghdl`: ghdl's sim face -- the judged drive (analyze + elaborate +
    /// run, every run log gated).
    GhdlSim,
    /// `nvc-build`: nvc's build face -- the same honest shape, nvc's
    /// probed process.
    NvcBuild,
    /// `nvc`: nvc's sim face -- the judged drive on nvc's probed faces.
    NvcSim,
    /// `iverilog-build`: iverilog's build face -- the one order-free
    /// compile, exit honest, no simulation, nothing judged.
    IverilogBuild,
    /// `iverilog`: iverilog's sim face -- the one compile, then the
    /// judged `vvp` run.
    IverilogSim,
    /// `verilator-build`: verilator's build face -- the lint and the
    /// per-top model builds, exit honest, no simulation, nothing
    /// judged.
    VerilatorBuild,
    /// `verilator`: verilator's sim face -- lint, per-top builds, then
    /// the judged model runs.
    VerilatorSim,
}

impl Subcommand {
    /// The drive mode the subcommand picks: how far the tool's shared
    /// drive goes.
    fn mode(self) -> blueprint::Mode {
        match self {
            Subcommand::GhdlBuild
            | Subcommand::NvcBuild
            | Subcommand::IverilogBuild
            | Subcommand::VerilatorBuild => blueprint::Mode::Build,
            Subcommand::GhdlSim
            | Subcommand::NvcSim
            | Subcommand::IverilogSim
            | Subcommand::VerilatorSim => blueprint::Mode::Sim,
        }
    }
}

/// The error for a subcommand name that resolves to nothing: loud,
/// usage-class, exit 2.
fn unknown_subcommand(name: &str) -> String {
    format!(
        "no target named {name:?}: carried subcommands: ghdl, ghdl-build, nvc, nvc-build, \
         iverilog, iverilog-build, verilator, verilator-build"
    )
}

/// The argv parse outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Cli {
    /// Print the help and exit 0.
    Help,
    /// Drive a tool.
    Run(Request),
}

/// The argv-level request, before the environment fills the gaps.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    /// The subcommand to drive.
    subcommand: Subcommand,
    /// The `--blueprint` path, when given.
    blueprint: Option<PathBuf>,
    /// The `--top` units, in the order given.
    tops: Vec<String>,
    /// The argv after `--`, appended to the drive's last tool argv.
    extra: Vec<String>,
}

/// An argv problem: loud, usage-class, exit 2.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CliError(String);

/// Parse the argv (program name already skipped).
fn parse(argv: &[String]) -> Result<Cli, CliError> {
    let Some(tool_arg) = argv.first() else {
        return Err(CliError("no tool given".to_string()));
    };
    if tool_arg == "--help" || tool_arg == "-h" {
        return Ok(Cli::Help);
    }
    let subcommand = match tool_arg.as_str() {
        "ghdl-build" => Subcommand::GhdlBuild,
        "ghdl" => Subcommand::GhdlSim,
        "nvc-build" => Subcommand::NvcBuild,
        "nvc" => Subcommand::NvcSim,
        "iverilog-build" => Subcommand::IverilogBuild,
        "iverilog" => Subcommand::IverilogSim,
        "verilator-build" => Subcommand::VerilatorBuild,
        "verilator" => Subcommand::VerilatorSim,
        other => return Err(CliError(unknown_subcommand(other))),
    };

    let mut blueprint: Option<PathBuf> = None;
    let mut tops = Vec::new();
    let mut extra = Vec::new();
    let mut rest = argv[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Cli::Help),
            "--blueprint" => {
                let value = rest
                    .next()
                    .ok_or_else(|| CliError("--blueprint needs a path".to_string()))?;
                blueprint = Some(PathBuf::from(value));
            }
            "--top" => {
                let value = rest
                    .next()
                    .ok_or_else(|| CliError("--top needs a unit name".to_string()))?;
                tops.push(value.clone());
            }
            // Orbit strips its own separator before this argv lands here,
            // so this only fires on direct drives: everything after it
            // rides the run verbatim, without a second look.
            "--" => {
                extra.extend(rest.cloned());
                break;
            }
            // Everything the drive does not own is a tool extra: the
            // orbit spawn shape appends the user's extras with no
            // separator, and rejecting one would reject the whole target.
            other => extra.push(other.to_string()),
        }
    }
    Ok(Cli::Run(Request {
        subcommand,
        blueprint,
        tops,
        extra,
    }))
}

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    match parse(&argv) {
        Ok(Cli::Help) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Cli::Run(request)) => drive_request(request),
        Err(e) => {
            eprintln!("orbit-contracts: error: {}", e.0);
            eprintln!("try `orbit-contracts --help`");
            ExitCode::from(2)
        }
    }
}

/// The tops to drive: the explicit `--top` units, else the testbench orbit
/// exports for the run.
///
/// `orbit test --target ghdl` never passes `--top`; it exports the
/// designated testbench as `$ORBIT_TB_NAME` instead. In all-mode orbit
/// exports the empty string, which counts as absent -- as does a missing
/// variable.
fn resolve_tops(explicit: Vec<String>, tb_name: Option<&str>) -> Vec<String> {
    if !explicit.is_empty() {
        return explicit;
    }
    match tb_name {
        Some(name) if !name.is_empty() => vec![name.to_string()],
        _ => Vec::new(),
    }
}

/// Resolve the environment's gaps and hand the request to the drive,
/// mapping the outcome to the honest exit.
fn drive_request(request: Request) -> ExitCode {
    let blueprint = request
        .blueprint
        .or_else(|| env::var_os("ORBIT_BLUEPRINT").map(PathBuf::from));
    let Some(blueprint) = blueprint else {
        eprintln!("orbit-contracts: error: no blueprint to read: pass --blueprint <path> or set ORBIT_BLUEPRINT");
        return ExitCode::from(2);
    };
    let defaulted = request.tops.is_empty();
    let tb_name = env::var("ORBIT_TB_NAME").ok();
    let tops = resolve_tops(request.tops, tb_name.as_deref());
    if tops.is_empty() {
        eprintln!(
            "orbit-contracts: error: no top to drive: pass --top <name>, or come through \
             orbit with $ORBIT_TB_NAME set (`orbit test --target ghdl` exports it)"
        );
        return ExitCode::from(2);
    }
    if defaulted {
        eprintln!(
            "orbit-contracts: note: no --top given; taking the testbench $ORBIT_TB_NAME: {}",
            tops[0]
        );
    }
    let outcome = match request.subcommand {
        Subcommand::GhdlBuild | Subcommand::GhdlSim => ghdl::drive(
            &blueprint::Drive {
                blueprint,
                tops,
                extra: request.extra,
            },
            request.subcommand.mode(),
        )
        .map_err(|e| e.exit_code()),
        Subcommand::NvcBuild | Subcommand::NvcSim => nvc::drive(
            &blueprint::Drive {
                blueprint,
                tops,
                extra: request.extra,
            },
            request.subcommand.mode(),
        )
        .map_err(|e| e.exit_code()),
        Subcommand::IverilogBuild | Subcommand::IverilogSim => iverilog::drive(
            &blueprint::Drive {
                blueprint,
                tops,
                extra: request.extra,
            },
            request.subcommand.mode(),
        )
        .map_err(|e| e.exit_code()),
        Subcommand::VerilatorBuild | Subcommand::VerilatorSim => verilator::drive(
            &blueprint::Drive {
                blueprint,
                tops,
                extra: request.extra,
            },
            request.subcommand.mode(),
        )
        .map_err(|e| e.exit_code()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    fn run_of(parsed: Cli) -> Request {
        let Cli::Run(request) = parsed else {
            panic!("expected a run, got {parsed:?}");
        };
        request
    }

    #[test]
    fn parse_collects_tops_and_extra_args() {
        let request = run_of(
            parse(&argv(&[
                "ghdl",
                "--blueprint",
                "b.tsv",
                "--top",
                "tb_a",
                "--top",
                "tb_b",
                "--",
                "-gW=4",
                "--vcd=w.vcd",
            ]))
            .unwrap(),
        );
        assert_eq!(request.subcommand, Subcommand::GhdlSim);
        assert_eq!(request.blueprint, Some(PathBuf::from("b.tsv")));
        assert_eq!(request.tops, vec!["tb_a".to_string(), "tb_b".to_string()]);
        assert_eq!(
            request.extra,
            vec!["-gW=4".to_string(), "--vcd=w.vcd".to_string()]
        );
    }

    #[test]
    fn ghdl_build_is_its_own_subcommand_driving_the_build_face() {
        // The split: `ghdl-build` is a separate target entry that shares
        // the drive with `ghdl` but stops after the elaborations.
        let request = run_of(parse(&argv(&["ghdl-build", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::GhdlBuild);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Build);
    }

    #[test]
    fn ghdl_drives_the_sim_face() {
        let request = run_of(parse(&argv(&["ghdl", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::GhdlSim);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Sim);
    }

    #[test]
    fn nvc_build_is_its_own_subcommand_driving_the_build_face() {
        let request = run_of(parse(&argv(&["nvc-build", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::NvcBuild);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Build);
    }

    #[test]
    fn nvc_drives_the_sim_face() {
        let request = run_of(parse(&argv(&["nvc", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::NvcSim);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Sim);
    }

    #[test]
    fn parse_leaves_the_blueprint_to_the_environment() {
        let request = run_of(parse(&argv(&["ghdl", "--top", "tb"])).unwrap());
        assert_eq!(request.blueprint, None);
    }

    #[test]
    fn parse_treats_nothing_after_the_separator_as_no_extra_args() {
        let request = run_of(parse(&argv(&["ghdl", "--top", "tb", "--"])).unwrap());
        assert!(request.extra.is_empty());
    }

    #[test]
    fn help_wins_wherever_it_sits() {
        assert_eq!(parse(&argv(&["--help"])), Ok(Cli::Help));
        assert_eq!(parse(&argv(&["ghdl", "-h"])), Ok(Cli::Help));
    }

    #[test]
    fn an_unknown_tool_is_a_loud_usage_error() {
        let err = parse(&argv(&["xsim", "--top", "tb"])).unwrap_err();
        assert!(
            err.0.contains("xsim"),
            "the error should name the tool: {}",
            err.0
        );
        // The error also names what IS carried, so the fix is readable
        // from the message alone.
        assert!(
            err.0.contains("ghdl-build")
                && err.0.contains("nvc-build")
                && err.0.contains("iverilog-build")
                && err.0.contains("verilator-build"),
            "the error should list the carried subcommands: {}",
            err.0
        );
    }

    #[test]
    fn iverilog_build_is_its_own_subcommand_driving_the_build_face() {
        // The split: `iverilog-build` is a separate target entry that
        // shares the drive with `iverilog` but stops after the compile.
        let request = run_of(parse(&argv(&["iverilog-build", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::IverilogBuild);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Build);
    }

    #[test]
    fn iverilog_drives_the_sim_face() {
        let request = run_of(parse(&argv(&["iverilog", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::IverilogSim);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Sim);
    }

    #[test]
    fn verilator_build_is_its_own_subcommand_driving_the_build_face() {
        // The split: `verilator-build` is a separate target entry that
        // shares the drive with `verilator` but stops after the model
        // builds.
        let request = run_of(parse(&argv(&["verilator-build", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::VerilatorBuild);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Build);
    }

    #[test]
    fn verilator_drives_the_sim_face() {
        let request = run_of(parse(&argv(&["verilator", "--top", "tb"])).unwrap());
        assert_eq!(request.subcommand, Subcommand::VerilatorSim);
        assert_eq!(request.subcommand.mode(), blueprint::Mode::Sim);
    }

    #[test]
    fn an_unknown_flag_rides_the_run_as_an_extra() {
        // The orbit spawn shape appends the user's extras with no
        // separator: `-gW=4 --vcd=w.vcd` reaches this argv bare, and
        // rejecting one would reject the whole target.
        let request = run_of(
            parse(&argv(&[
                "ghdl",
                "--top",
                "tb",
                "-gW=4",
                "--vcd=w.vcd",
                "--frobnicate",
            ]))
            .unwrap(),
        );
        assert_eq!(
            request.extra,
            vec![
                "-gW=4".to_string(),
                "--vcd=w.vcd".to_string(),
                "--frobnicate".to_string()
            ]
        );
    }

    #[test]
    fn a_dangling_flag_is_a_loud_usage_error() {
        assert!(parse(&argv(&["ghdl", "--top"])).is_err());
        assert!(parse(&argv(&["ghdl", "--blueprint"])).is_err());
    }

    #[test]
    fn empty_argv_is_a_loud_usage_error() {
        let err = parse(&[]).unwrap_err();
        assert_eq!(err.0, "no tool given");
    }

    #[test]
    fn explicit_tops_win_over_the_exported_testbench() {
        let tops = resolve_tops(vec!["tb_a".to_string()], Some("tb_exported"));
        assert_eq!(tops, vec!["tb_a".to_string()]);
    }

    #[test]
    fn the_exported_testbench_fills_an_empty_top_list() {
        let tops = resolve_tops(Vec::new(), Some("tb_exported"));
        assert_eq!(tops, vec!["tb_exported".to_string()]);
    }

    #[test]
    fn an_empty_export_counts_as_absent() {
        // orbit exports the empty string in all-mode.
        assert!(resolve_tops(Vec::new(), Some("")).is_empty());
        assert!(resolve_tops(Vec::new(), None).is_empty());
    }
}
