use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod account_redaction;
mod content_write;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
mod handle_write;
pub mod loader;
mod locale_write;
pub mod media;
mod media_write;
pub mod migrations;
pub mod mutations;
pub mod observability;
pub mod presentation;
pub mod privacy;
mod profile_updated_event;
pub mod reader;
pub mod services;
mod upsert_write;
mod visibility_write;

pub use account_redaction::redact_profile_for_account_deactivation_in_tx;
pub use dto::{ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};
pub use entities::ProfileRecord;
pub use error::{ProfileError, ProfileResult};
pub use loader::{ProfileSummaryLoader, ProfileSummaryLoaderKey};
pub use media::{
    ProfileImagePresentation, ProfileMediaPublicImageProvider, ProfileMediaSlot,
    profile_image_presentation, validate_profile_media_asset,
};
pub use mutations::{ProfileBackfillRequest, ProfileMutationContext, ProfileMutationService};
pub use observability::{
    PROFILE_BACKFILL_OPERATION, PROFILE_OPERATION_TARGET, ProfileBackfillTimer, ProfileOperation,
    ProfileOperationTimer,
};
pub use presentation::ProfilePresentationService;
pub use privacy::{
    ProfileAccessAudience, ProfilePrivacyDecision, ProfilePrivacyReadPort,
    ProfilePrivacyReadRequest, ProfilePrivacyRuntime, ProfilePrivacyService,
    evaluate_profile_access,
};
pub use reader::ProfilesReader;
pub use services::{ProfileBackfillResult, ProfileService};

pub struct ProfilesModule;

#[async_trait]
impl RusToKModule for ProfilesModule {
    fn slug(&self) -> &'static str {
        "profiles"
    }

    fn name(&self) -> &'static str {
        "Profiles"
    }

    fn description(&self) -> &'static str {
        "Universal public profile domain for platform users"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["media", "social_graph", "taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PROFILES_CREATE,
            Permission::PROFILES_READ,
            Permission::PROFILES_UPDATE,
            Permission::PROFILES_DELETE,
            Permission::PROFILES_LIST,
            Permission::PROFILES_MANAGE,
        ]
    }
}

impl MigrationSource for ProfilesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Resource};

    #[test]
    fn profiles_permissions_cover_crud_and_list() {
        let permissions = ProfilesModule.permissions();
        for action in [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
            Action::Manage,
        ] {
            assert!(permissions.contains(&Permission::new(Resource::Profiles, action)));
        }
    }
}
