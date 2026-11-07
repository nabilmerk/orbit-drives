//! `orbit-contracts`: the probed failure vocabularies and the honest-exit
//! target drives for the [orbit](https://github.com/chaseruskin/orbit) HDL
//! package manager.
//!
//! Orbit's only contract with a target is the exit code: nonzero fails the
//! build. This crate makes that exit mean something. Each target drive
//! captures the tool's log and gates it on the tool's probed failure
//! vocabulary -- faces read off live runs, never invented -- before
//! reporting its own honest exit to orbit. The judge exists because the
//! tools' exits do not always: ghdl exits 0 on a violated `severity error`
//! check, and the face in the log is the only witness; nvc's exits have
//! probed honest so far, and there the judge is the second witness. Either
//! witness convicts, everywhere.
//!
//! - [`contracts`]: the per-tool vocabularies and the judge.
//! - [`ghdl`]: the ghdl target drives -- blueprint in, judged verdict out.
//! - [`nvc`]: the nvc target drives -- the same shapes, nvc's probed faces.
//! - [`iverilog`]: the iverilog target drives -- one order-free compile,
//!   one judged `vvp` run.
//! - [`verilator`]: the verilator target drives -- one lint, per-top
//!   model builds, judged model runs.

pub mod contracts;
pub mod ghdl;
pub mod iverilog;
pub mod nvc;
pub mod verilator;
