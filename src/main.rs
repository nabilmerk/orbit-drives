//! The `orbit-contracts` binary: argv in, honest exit out.
//!
//! ```text
//! orbit-contracts ghdl --blueprint <path> --top <name> [--] [extra ghdl args]
//! ```
//!
//! Orbit spawns the target with the working directory set to the target's
//! output directory and `ORBIT_BLUEPRINT` pointing at the plan file, so
//! the default blueprint is the environment's; `--blueprint` overrides it
//! for direct drives. Arguments after `--` are orbit-appended ghdl run
//! options and reach the `ghdl -r` argv verbatim.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use orbit_contracts::contracts::Tool;
use orbit_contracts::ghdl;

const USAGE: &str = "\
orbit-contracts: the probed failure vocabularies and the honest-exit targets for orbit

USAGE:
    orbit-contracts <TOOL> [TOOL OPTIONS] [--] [EXTRA TOOL ARGS]

TOOLS:
    ghdl    analyze the blueprint's VHDL in blueprint order, elaborate and
            run each --top, and gate every run log on ghdl's probed
            failure vocabulary

GHDL OPTIONS:
    --blueprint <path>  the TSV blueprint (default: $ORBIT_BLUEPRINT)
    --top <name>        a top unit to elaborate and run (repeatable, required)
    --                  everything after is appended to each `ghdl -r` argv
    -h, --help          print this help

EXITS:
    0    clean (every run log judged clean)
    1    findings (a log carried a failure face, or ghdl reported a broken design)
    2    usage (bad argv, unreadable blueprint, a fileset the drive does not carry)
";

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
    /// The tool to drive.
    tool: Tool,
    /// The `--blueprint` path, when given.
    blueprint: Option<PathBuf>,
    /// The `--top` units, in the order given.
    tops: Vec<String>,
    /// The argv after `--`, appended to each `ghdl -r`.
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
    let tool = Tool::from_str(tool_arg).map_err(|e| CliError(e.to_string()))?;

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
            "--" => {
                extra = rest.cloned().collect();
                break;
            }
            other => {
                return Err(CliError(format!(
                    "unknown argument {other:?} for tool {}",
                    tool.name()
                )));
            }
        }
    }
    Ok(Cli::Run(Request {
        tool,
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
    if request.tops.is_empty() {
        eprintln!("orbit-contracts: error: no --top given: name at least one top unit to elaborate and run");
        return ExitCode::from(2);
    }
    match ghdl::drive(&ghdl::Drive {
        blueprint,
        tops: request.tops,
        extra: request.extra,
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => ExitCode::from(e.exit_code()),
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
        assert_eq!(request.tool, Tool::Ghdl);
        assert_eq!(request.blueprint, Some(PathBuf::from("b.tsv")));
        assert_eq!(request.tops, vec!["tb_a".to_string(), "tb_b".to_string()]);
        assert_eq!(
            request.extra,
            vec!["-gW=4".to_string(), "--vcd=w.vcd".to_string()]
        );
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
    }

    #[test]
    fn an_unknown_flag_is_a_loud_usage_error() {
        assert!(parse(&argv(&["ghdl", "--frobnicate"])).is_err());
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
}
