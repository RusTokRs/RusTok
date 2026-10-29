//! # RusToK Events Module
//!
//! Runtime registration adapter for the `rustok-events` capability.
//!
//! Event contracts and transport-facing types remain owned by
//! `rustok_events`. This small adapter exists because `rustok-core` already
//! consumes those contracts; keeping the runtime implementation in a separate
//! crate preserves the dependency direction and avoids a cycle.

pub mod module;

pub use module::EventsModule;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contract_tests;
