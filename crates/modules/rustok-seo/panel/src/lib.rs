//! # RusToK SEO Panel
//!
//! Reusable SEO authoring panel, widget helpers, and completeness analytics
//! for entity editors across RusToK administration surfaces.

mod components;
mod i18n;
mod model;
mod panel;
mod transport;

#[cfg(test)]
mod contract_tests;

pub use components::{
    SeoControlPlaneWidgetStateCard, SeoControlPlaneWidgets, SeoDeliveryStatusCards,
    SeoRecommendationsCard, SeoRemediationHintCard, SeoSchemaPreviewCard, SeoSnippetPreviewCard,
    SeoSummaryTile,
};
pub use model::{
    SeoCompletenessReport, SeoControlPlaneWidgetState, SeoControlPlaneWidgetStateKind,
    SeoEntityForm, SeoEventDeliverySummary, SeoMetaView, SeoRevisionView,
    derive_control_plane_widget_state, remediation_hint_for_issue_code, validate_target_id,
};
pub use panel::{SeoCapabilityNotice, SeoEntityPanel};
