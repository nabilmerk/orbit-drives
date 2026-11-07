//! `orbit-drives`: the probed failure vocabularies and the honest-exit
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
//! - [`blueprint`]: the shared machinery -- the TSV blueprint parse, the
//!   argv-level request view, and the fileset filters.
//! - [`proc`]: the shared process plumbing -- spawn-and-capture, the log
//!   join, the verbatim block relays.
//! - [`contracts`]: the per-tool vocabularies and the judge.
//! - [`ghdl`]: the ghdl target drives -- blueprint in, judged verdict out.
//! - [`nvc`]: the nvc target drives -- the same shapes, nvc's probed faces.
//! - [`iverilog`]: the iverilog target drives -- one order-free compile,
//!   one judged `vvp` run.
//! - [`verilator`]: the verilator target drives -- one lint, per-top
//!   model builds, judged model runs.
//! - [`yosys`]: the yosys synthesis drive -- one script-driven synth per
//!   top, the netlists as payload. Build only: yosys has no runtime.
//! - [`vivado`]: the vivado target drives -- both languages compiled,
//!   the generated synthesis with the catch-guard, and the judged xsim
//!   runs (the run exit is a lie; the line-start faces are the truth).
//! - [`quartus`]: the quartus target drives -- the DOCUMENTED-ONLY
//!   drive: the generated `.qsf` + the batch `quartus_map` on the build
//!   face, the bundled Questa (vlib/vmap/vcom/vsim) on the judged sim
//!   face. Every face is the documented hypothesis, UNPROBED -- no
//!   quartus on the probing box; every hypothesis carries its
//!   first-contact rule in the README's honest limits.
//! - [`cocotb`]: the cocotb test drive -- cocotb's own python runner
//!   (`cocotb_tools.runner`, probed live on 2.1.0) over ghdl, the
//!   results.xml verdict read by the generated runner script, the log
//!   judged by cocotb's probed faces. Test only: there is no build face
//!   and no compile step of this drive's own -- the runner IS the
//!   compile.

pub mod blueprint;
pub mod cocotb;
pub mod contracts;
pub mod ghdl;
pub mod iverilog;
pub mod nvc;
pub mod proc;
pub mod quartus;
pub mod verilator;
pub mod vivado;
pub mod yosys;

pub use blueprint::{
    parse_blueprint, quartus_files, verilog_only, vhdl_only, vivado_files, BlueprintError, Drive,
    Entry, MalformedLine, Mode, NotCarried,
};
