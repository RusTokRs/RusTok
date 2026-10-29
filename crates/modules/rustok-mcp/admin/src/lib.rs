//! # rustok-mcp-admin
//!
//! Leptos admin UI package for the `rustok-mcp` control plane.
//!
//! Exposes MCP tool registry inspection, server configuration, resource views,
//! and prompt templates for platform operators.

pub mod core;
pub mod transport;
pub mod ui;

pub use ui::McpAdmin;

#[cfg(test)]
mod contract_tests;
