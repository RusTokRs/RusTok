//! SeaORM entities for pages module.

pub mod menu;
pub mod menu_item;
pub mod menu_item_translation;
pub mod menu_translation;
pub mod page;
pub mod page_body;
pub mod page_builder_scenario_baseline;
pub mod page_channel_visibility;
pub mod page_published_landing_artifact;
pub mod page_static_landing_artifact;
pub mod page_translation;

pub use menu::Entity as Menu;
pub use menu_item::Entity as MenuItem;
pub use page::Entity as Page;
pub use page_builder_scenario_baseline::Entity as PageBuilderScenarioBaseline;
pub use page_channel_visibility::Entity as PageChannelVisibility;
pub use page_published_landing_artifact::Entity as PagePublishedLandingArtifact;
pub use page_static_landing_artifact::Entity as PageStaticLandingArtifact;
