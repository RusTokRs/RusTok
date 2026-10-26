mod components;
mod i18n;
mod model;
mod panel;
mod transport;

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

#[cfg(test)]
mod contract_tests;
