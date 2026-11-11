//! Repository diagnostics shared by the RusToK command-line tools.

mod cli;
mod discovery;
mod hotspots;
mod hygiene;
mod report;
mod source;

pub use cli::run;
pub use hygiene::{Confidence, Finding, Severity, scan_source};
