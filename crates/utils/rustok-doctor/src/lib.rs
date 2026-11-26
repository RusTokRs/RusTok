//! Repository diagnostics shared by the RusToK command-line tools.

mod cli;
mod discovery;
mod hotspots;
mod hygiene;
mod report;
mod source;

pub use cli::run;
pub use hygiene::{
    Confidence, Finding, Severity, parse_dev_only_crates, scan_crate_source, scan_source,
};
