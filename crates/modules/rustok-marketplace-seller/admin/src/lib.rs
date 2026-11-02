//! # RusToK Marketplace Seller Admin
//!
//! Leptos admin UI package for the `rustok-marketplace-seller` module.
//! Provides seller profile management, onboarding workflows, seller-scoped member administration, and audit event history panels.

mod core;
#[path = "ui/event_history_panel.rs"]
mod event_history_panel;
#[path = "ui/event_timeline.rs"]
mod event_timeline;
mod i18n;
mod model;
mod transport;
mod ui;

pub use core::{
    MarketplaceSellerAdminTransportProfile, build_marketplace_seller_admin_shell,
    selected_transport_profile,
};
pub use event_history_panel::MarketplaceSellerEventHistoryPanel;
pub use event_timeline::MarketplaceSellerEventTimeline;
pub use model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminCommandResult,
    MarketplaceSellerAdminDetail, MarketplaceSellerAdminDirectory,
    MarketplaceSellerAdminEvent, MarketplaceSellerAdminEventHistory,
    MarketplaceSellerAdminFilters, MarketplaceSellerAdminListItem,
    MarketplaceSellerAdminMember, MarketplaceSellerAdminRecord,
    MarketplaceSellerAdminShell, MarketplaceSellerCreateDraft,
    MarketplaceSellerMemberCreateDraft, MarketplaceSellerMemberUpdateDraft,
    MarketplaceSellerProfileDraft,
};
pub use transport::{
    MARKETPLACE_SELLER_TRANSPORT_FALLBACK_POLICY, MarketplaceSellerAdminTransportContext,
    execute_marketplace_seller_command, load_marketplace_seller_detail,
    load_marketplace_seller_directory, load_marketplace_seller_event_history,
};
pub use ui::leptos::MarketplaceSellerAdmin;

#[cfg(test)]
mod contract_tests;
