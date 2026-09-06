#![recursion_limit = "256"]

mod core;
mod i18n;
mod locale_switch;
mod model;
mod page_builder;
mod topic_fork_model;
mod topic_merge_model;
mod topic_reply_range_model;
mod topic_slug_rename_model;
mod topic_split_model;
mod transport;
mod ui;
mod widget_preview_transport;
mod widget_property_transport;

pub use page_builder::{
    ForumContributionAdapter, ForumWidgetOwnerSchemaRef, ForumWidgetPropertyEditorModel,
    ForumWidgetRenderModel, build_forum_admin_contribution_registry, forum_contribution_manifest,
    forum_fly_registry_set, forum_full_admin_contribution_policy, forum_widget_contribution,
    forum_widget_preview_contribution, register_forum_fly_widgets,
};
pub use ui::root::ForumAdmin;
pub use widget_preview_transport::{
    ForumPageBuilderTransportAttestationResponse, ForumWidgetPreviewTransportRequest,
    attest_forum_page_builder_transport, preview_forum_page_builder_widget,
};
pub use widget_property_transport::{
    ForumWidgetPropertySchemaTransportRequest, ForumWidgetPropertySchemaTransportResponse,
    ForumWidgetPropertyValidationIssueTransport, ForumWidgetPropertyValidationTransportRequest,
    ForumWidgetPropertyValidationTransportResponse, load_forum_page_builder_widget_property_schema,
    validate_forum_page_builder_widget_properties,
};
