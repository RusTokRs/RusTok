//! # rustok-messenger-admin
//!
//! Leptos admin UI package for the `rustok-messenger` module.
//!
//! Exposes messenger administration, channel management, real-time fabric
//! observability, and cryptographic device audits.

pub mod core;
pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{ChannelFormDraft, MessengerAdminTab};
pub use model::{
    ChannelListItemPayload, CreateChannelPayload, E2eeDeviceAuditPayload, MessengerOverviewPayload,
};
pub use transport::{fetch_channels, fetch_e2ee_devices, fetch_overview};
pub use ui::MessengerAdmin;

#[cfg(test)]
mod contract_tests;
