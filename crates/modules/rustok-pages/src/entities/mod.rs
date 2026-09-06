//! SeaORM entities for pages module.

pub mod page;
pub mod page_artifact_binding_replacement_operation;
pub mod page_artifact_rebuild_operation;
pub mod page_body;
pub mod page_builder_scenario_baseline;
pub mod page_channel_visibility;
pub mod page_publish_operation;
pub mod page_publish_operation_artifact;
pub mod page_publish_rebuild_source;
pub mod page_published_landing_artifact;
pub mod page_rollback_operation;
pub mod page_route_alias;
pub mod page_route_history_import;
pub mod page_route_publication;
pub mod page_static_landing_artifact;
pub mod page_translation;
pub mod translation_change;

pub use page::Entity as Page;
pub use page_artifact_binding_replacement_operation::Entity as PageArtifactBindingReplacementOperation;
pub use page_artifact_rebuild_operation::Entity as PageArtifactRebuildOperation;
pub use page_builder_scenario_baseline::Entity as PageBuilderScenarioBaseline;
pub use page_channel_visibility::Entity as PageChannelVisibility;
pub use page_publish_operation::Entity as PagePublishOperation;
pub use page_publish_operation_artifact::Entity as PagePublishOperationArtifact;
pub use page_publish_rebuild_source::Entity as PagePublishRebuildSource;
pub use page_published_landing_artifact::Entity as PagePublishedLandingArtifact;
pub use page_rollback_operation::Entity as PageRollbackOperation;
pub use page_route_alias::Entity as PageRouteAlias;
pub use page_route_history_import::Entity as PageRouteHistoryImport;
pub use page_route_publication::Entity as PageRoutePublication;
pub use page_static_landing_artifact::Entity as PageStaticLandingArtifact;
