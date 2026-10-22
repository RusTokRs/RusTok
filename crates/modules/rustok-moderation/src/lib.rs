pub mod model;
pub mod provider;

pub use model::*;
pub use provider::*;

pub mod api {
    pub use super::model::*;
    pub use super::provider::*;
}

#[cfg(feature = "server")]
pub mod application;
#[cfg(feature = "server")]
pub mod application_dispatch;
#[cfg(feature = "server")]
pub mod application_recovery;
#[cfg(feature = "server")]
mod application_scheduler;
#[cfg(feature = "server")]
mod commands;
#[cfg(feature = "server")]
pub mod domain;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub mod ports;
#[cfg(feature = "server")]
mod receipts;
#[cfg(feature = "server")]
pub mod service;

#[cfg(test)]
mod contract_tests;

#[cfg(feature = "server")]
pub use application::{
    DEFAULT_APPLICATION_LEASE_SECONDS, MAX_APPLICATION_LEASE_SECONDS,
    MAX_APPLICATION_RETRY_SECONDS, MAX_DUE_APPLICATION_OPERATIONS,
};
#[cfg(feature = "server")]
pub use application_dispatch::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, APPLICATION_RETRY_BASE_SECONDS,
    APPLICATION_RETRY_MAX_SECONDS, application_retry_delay_seconds,
};
#[cfg(feature = "server")]
pub use application_recovery::MAX_APPLICATION_RECOVERY_REASON_BYTES;
#[cfg(feature = "server")]
pub use domain::*;
#[cfg(feature = "server")]
pub use error::{ModerationError, ModerationResult};
#[cfg(feature = "server")]
pub use module::ModerationModule;
#[cfg(feature = "server")]
pub use ports::*;
#[cfg(feature = "server")]
pub use service::ModerationService;

