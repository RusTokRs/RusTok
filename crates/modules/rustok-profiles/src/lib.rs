//! # rustok-profiles
//!
//! Universal public profile domain module for RusToK platform users.
//!
//! Provides public profile data, presentation mappings, privacy evaluation,
//! and batched summary loading across downstream domain surfaces.

pub mod api;

pub use api::{
    GqlProfileSummary, GqlProfileVisibility, ProfileSummaryAudience, ProfileSummaryReadError,
    ProfileSummaryReadResult, ProfileSummaryReader, ProfileSummaryVisibility,
};
pub use api::ProfileSummary as PublicProfileSummary;

#[cfg(feature = "server")]
mod account_redaction;
#[cfg(feature = "server")]
mod content_write;
#[cfg(feature = "server")]
pub mod dto;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod graphql;
#[cfg(feature = "server")]
mod handle_write;
#[cfg(feature = "server")]
pub mod loader;
#[cfg(feature = "server")]
mod locale_write;
#[cfg(feature = "server")]
pub mod media;
#[cfg(feature = "server")]
mod media_write;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod mutations;
#[cfg(feature = "server")]
pub mod observability;
#[cfg(feature = "server")]
pub mod presentation;
#[cfg(feature = "server")]
pub mod privacy;
#[cfg(feature = "server")]
mod profile_updated_event;
#[cfg(feature = "server")]
pub mod reader;
#[cfg(feature = "server")]
pub mod services;
#[cfg(feature = "server")]
mod upsert_write;
#[cfg(feature = "server")]
mod visibility_write;

#[cfg(feature = "server")]
pub use account_redaction::redact_profile_for_account_deactivation_in_tx;
#[cfg(feature = "server")]
pub use dto::{ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};
#[cfg(feature = "server")]
pub use entities::ProfileRecord;
#[cfg(feature = "server")]
pub use error::{ProfileError, ProfileResult};
#[cfg(feature = "server")]
pub use loader::{ProfileSummaryLoader, ProfileSummaryLoaderKey};
#[cfg(feature = "server")]
pub use media::{
    ProfileImagePresentation, ProfileMediaPublicImageProvider, ProfileMediaSlot,
    profile_image_presentation, validate_profile_media_asset,
};
#[cfg(feature = "server")]
pub use mutations::{ProfileBackfillRequest, ProfileMutationContext, ProfileMutationService};
#[cfg(feature = "server")]
pub use observability::{
    PROFILE_BACKFILL_OPERATION, PROFILE_OPERATION_TARGET, ProfileBackfillTimer, ProfileOperation,
    ProfileOperationTimer,
};
#[cfg(feature = "server")]
pub use presentation::ProfilePresentationService;
#[cfg(feature = "server")]
pub use privacy::{
    ProfileAccessAudience, ProfilePrivacyDecision, ProfilePrivacyReadPort,
    ProfilePrivacyReadRequest, ProfilePrivacyRuntime, ProfilePrivacyService,
    evaluate_profile_access,
};
#[cfg(feature = "server")]
pub use reader::ProfilesReader;
#[cfg(feature = "server")]
pub use services::{MAX_PROFILE_HANDLE_BATCH, ProfileBackfillResult, ProfileService};

#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub use module::ProfilesModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;
