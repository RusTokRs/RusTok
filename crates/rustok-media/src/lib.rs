pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod image;
pub mod lifecycle;
pub mod migrations;
pub mod ports;
pub mod service;

use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use dto::{
    ALLOWED_MIME_PREFIXES, CreateRenditionInput, DEFAULT_MAX_SIZE, MediaImageDeliveryProfile,
    MediaImageDescriptor, MediaItem, MediaRenditionItem, MediaTranslationItem,
    PrepareUploadSessionInput, PreparedUploadSession, UploadInput, UpsertTranslationInput,
};
pub use entities::*;
pub use error::{MediaError, Result};
pub use graphql::{MediaMutation, MediaQuery};
pub use image::{
    CropRect, ImageBackground, ImageOutput, ImageOutputFormat, ImageProcessingError,
    ImageProcessingLimits, ImageRecipe, ImageResize, ImageWorker, QuarterTurn,
};
pub use lifecycle::{AssetState, BlobState, RenditionState, UploadState};
pub use ports::*;
pub use service::{
    MediaReconciliationDecision, MediaReconciliationReport, MediaService, MediaUsageSnapshot,
    load_media_usage_snapshot,
};

pub struct MediaModule;

#[async_trait]
impl RusToKModule for MediaModule {
    fn slug(&self) -> &'static str {
        "media"
    }

    fn name(&self) -> &'static str {
        "Media"
    }

    fn description(&self) -> &'static str {
        "Media library, uploads and localized asset metadata"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Media, Action::Create),
            Permission::new(Resource::Media, Action::Read),
            Permission::new(Resource::Media, Action::Update),
            Permission::new(Resource::Media, Action::Delete),
            Permission::new(Resource::Media, Action::List),
            Permission::new(Resource::Media, Action::Manage),
        ]
    }
}

impl MigrationSource for MediaModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
