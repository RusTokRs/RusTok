# rustok-media / CRATE_API

## Public Modules
`controllers`, `dto`, `entities`, `error`, `graphql`, `image`, `lifecycle`, `migrations`, `module`, `ports`, `public_image`, `service`, `translation_target`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `MediaModule` implementing `RusToKModule` (slug `"media"`, dependencies `["outbox"]`) and `MigrationSource`.
- **Media Asset & Storage Service**:
  - `MediaService` providing uploads, blobs, renditions, lifecycle transitions, and usage snapshots.
  - Image processing pipeline (`ImageWorker`, `ImageRecipe`, `ImageResize`, `CropRect`, `ImageOutputFormat`).
  - Storage reconciliation (`reconcile_storage`, `MediaReconciliationReport`).
- **Media Ports & Transports**:
  - `MediaAssetReadPort` (`get_asset`, `get_asset_reference_admission`, `list_assets`, `get_image_descriptor`, `get_translations`).
  - `MediaAssetWritePort` (`prepare_upload`, `complete_upload`, `delete_asset`, `retain_asset_reference`, `release_asset_reference`, `upsert_translation`, `reconcile_storage`).
  - `MediaAssetPorts` providing paired read/write access.
  - `MediaPublicImageReadPort` and `MediaPublicImageService` for public content delivery.
- **Localization & Translation**:
  - `MediaTranslationTargetProvider` implementing platform translation contracts.
  - `UpsertTranslationInput` and `ApplyExactMediaTranslationInput`.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, call policies, and permissions.
- **Outbox & Storage**: Coordinates durable operation receipts with `rustok-outbox` and binary object persistence via `rustok-storage`.
- **Content & Commerce Consumers**: Provides reference admission (`retain_asset_reference`, `release_asset_reference`) to prevent premature deletion of active assets.

## Primary Public Types and Signatures
- `pub struct MediaModule`
- `pub struct MediaService`
- `pub trait MediaAssetReadPort`
- `pub trait MediaAssetWritePort`
- `pub struct MediaAssetPorts`
- `pub struct MediaItem`
- `pub struct MediaRenditionItem`
- `pub struct MediaTranslationItem`
- `pub struct MediaImageDescriptor`
- `pub struct MediaUploadRequest`
- `pub struct MediaUploadTarget`
- `pub enum MediaError`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-outbox`
- `rustok-storage`
- `rustok-telemetry`
- `rustok-translation-targets`

## Common AI Mistakes
- Using wildcard re-exports (`pub use entities::*;`, `pub use ports::*;`) instead of explicit named imports.
- Passing binary blob payloads directly through cross-module port DTOs rather than using `MEDIA_OWNER_STREAMING_UPLOAD_PATH` or presigned URLs.
- Deleting media assets that are still retained by active domain references without checking `MediaAssetReferenceAdmission`.
- Bypassing idempotency keys or deadlines on write port calls.

## Minimum Contract Set

### Input DTOs/Commands
- `MediaUploadRequest` (`original_name`, `content_type`, `content_length`, `owner_module`)
- `MediaAssetReferenceInput` (`reference_id`, `owner_module`, `entity_type`, `field_name`)
- `UpsertTranslationInput` (`locale`, `alt_text`, `title`, `description`)
- `MediaReconciliationRequest` (`limit`)

### Domain Invariants
- Reference integrity: Media assets retained by active domain entity references cannot be deleted; attempts return `MediaError::AssetReferenced` (`PortError::conflict`).
- Bounded limits: Reconciliation limit is strictly bounded by `MAX_MEDIA_RECONCILIATION_LIMIT` (1,000); list queries are bounded by `MAX_MEDIA_LIST_LIMIT` (100).
- Idempotency & deadlines: All write operations (`MediaAssetWritePort`) require valid write policies with non-empty idempotency keys and active deadlines.
- Tenant isolation: Assets and blobs are strictly partitioned by `tenant_id`; requests with nil or mismatched tenant IDs fail with `PortError::validation`.

### Events / Outbox Side Effects
- State changes (uploads, renditions, deletions, translations) schedule or publish domain events through `rustok-outbox`.
- Asset deletions trigger asynchronous blob deletion from the backing object store upon reference release.

### Errors / Failure Codes
- `MediaError` variants: `NotFound`, `Forbidden`, `UnsupportedMimeType`, `InvalidMediaContent`, `FileTooLarge`, `AssetReferenced`, `AssetReferenceConflict`, `UploadSessionExpired`.
- `PortError` mapped to `Validation`, `NotFound`, `Forbidden`, `Conflict`, `Timeout`, `Unavailable`, or `InvariantViolation`.
