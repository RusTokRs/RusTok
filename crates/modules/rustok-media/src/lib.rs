//! # RusToK Media Module
//!
//! Media asset management for RusTok — uploads, image transformations, metadata,
//! translations, and transport-neutral asset ports.
//!
//! Provides the media library service, pure-Rust image resizing and formatting,
//! storage reconciliation, GraphQL/REST controllers, and consumer reference retention.

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod image;
pub mod lifecycle;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod public_image;
pub mod service;
mod translation_evidence;
pub mod translation_target;

#[cfg(test)]
mod contract_tests;

pub use dto::{
    ALLOWED_MIME_PREFIXES, ApplyExactMediaTranslationInput, CreateRenditionInput, DEFAULT_MAX_SIZE,
    MediaAssetReference, MediaAssetReferenceAdmission, MediaAssetReferenceAdmissionState,
    MediaAssetReferenceInput, MediaImageDeliveryProfile, MediaImageDescriptor, MediaItem,
    MediaRenditionItem, MediaTranslationItem, NormalizedTranslationInput,
    PrepareUploadSessionInput, PreparedUploadSession, UploadInput, UpsertTranslationInput,
};
pub use entities::{
    asset, asset_reference, blob, media_translation, rendition, translation_change, upload_session,
};
pub use error::{MediaError, Result};
pub use graphql::{MediaMutation, MediaQuery};
pub use image::{
    CropRect, ImageBackground, ImageOutput, ImageOutputFormat, ImageProcessingError,
    ImageProcessingLimits, ImageRecipe, ImageResize, ImageWorker, QuarterTurn, inspect_image,
};
pub use lifecycle::{AssetState, BlobState, RenditionState, UploadState};
pub use module::MediaModule;
pub use ports::{
    MEDIA_OWNER_STREAMING_UPLOAD_PATH, MediaAssetPorts, MediaAssetReadPort,
    MediaAssetReferenceListPage, MediaAssetReferenceListRequest, MediaAssetReferenceLookupRequest,
    MediaAssetReferenceLookupResult, MediaAssetWritePort, MediaReconciliationRequest,
    MediaUploadRequest, MediaUploadTarget, MediaUploadTransport,
};
pub use public_image::{
    MEDIA_PUBLIC_IMAGE_PATH_PREFIX, MediaPublicImageAsset, MediaPublicImageBody,
    MediaPublicImageReadPort, MediaPublicImageService, public_image_path,
};
pub use service::{
    MediaReconciliationDecision, MediaReconciliationReport, MediaService, MediaUsageSnapshot,
    load_media_usage_snapshot,
};
pub use translation_target::MediaTranslationTargetProvider;
