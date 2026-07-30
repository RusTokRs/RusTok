use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod loader;
pub mod media;
pub mod migrations;
pub mod observability;
pub mod presentation;
pub mod privacy;
pub mod reader;
pub mod services;
mod visibility_write;

pub use dto::{ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};
pub use entities::ProfileRecord;
pub use error::{ProfileError, ProfileResult};
pub use loader::{ProfileSummaryLoader, ProfileSummaryLoaderKey};
pub use media::{
    ProfileImagePresentation, ProfileMediaPublicImageProvider, ProfileMediaSlot,
    profile_image_presentation, validate_profile_media_asset,
};
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
