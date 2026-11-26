//! # rustok-blog-cli
//!
//! External operational command adapters for `rustok-blog`.
//!
//! This crate maps platform CLI requests to owner-owned Blog services. It
//! deliberately contains no domain policy, terminal output, or exit handling.

mod commands;
mod provider;
mod runtime;

#[cfg(test)]
mod tests;

pub use commands::cleanup_tokens::{CleanupTokensOptions, parse_cleanup_tokens_options};
pub use commands::publish_scheduled::{PublishScheduledOptions, parse_publish_scheduled_options};
pub use provider::{BlogCommandProvider, command_provider};
