//! # rustok-messenger-storefront
//!
//! Leptos storefront UI package for the `rustok-messenger` module.
//!
//! Exposes conversational workspaces, message feeds, read receipts, and contextual
//! floating chat widgets.

pub mod core;
pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{
    ChatMessageViewModel, ContextDrawerTab, ConversationFilter, ConversationItemViewModel,
    DeliveryStatus, MessageDateGroup, MessengerLabels, MessengerWorkspaceState,
};
pub use model::{
    ConversationItemPayload, MessageItemPayload, SendMessagePayload, UpdateCursorPayload,
};
pub use transport::{fetch_conversations, fetch_messages, send_message, update_cursor};
pub use ui::{MessengerFloatingWidget, MessengerView, MessengerWorkstation};

#[cfg(test)]
mod contract_tests;
