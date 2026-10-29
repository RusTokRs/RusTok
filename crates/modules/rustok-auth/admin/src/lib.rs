//! # RusToK Auth Admin
//!
//! Leptos administration and user management UI package for `rustok-auth`.
//!
//! Provides operator sign-in, account registration, password resets, user directory,
//! role/permission inspect panels, and OAuth application configuration.

#![recursion_limit = "256"]

pub mod core;
pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;

pub use ui::{
    AcceptInvite, AuthAdmin, Login, OAuthAppsPage, Profile, Register, ResetPassword, Security,
    UserDetails, Users,
};
