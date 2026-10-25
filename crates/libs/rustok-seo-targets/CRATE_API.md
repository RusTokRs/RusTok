# rustok-seo-targets / CRATE_API

## Public surface

### Primary Public Types and Signatures

- `pub struct SeoTargetSlug`: Validated newtype representing a registered SEO target identifier (e.g. `page`, `product`, `blog_post`, `discussion_topic`).
- `pub mod builtin_slug`: Canonical constants for built-in SEO target slugs (`PAGE`, `BLOG_POST`, `DISCUSSION_TOPIC`, `PRODUCT`).
- `pub enum SeoTargetCapabilityKind`: Capabilities supported by a target (`Routing`, `Metadata`, `Sitemaps`, `Bulk`).
- `pub struct SeoTargetCapabilities`: Bitset/flag representation of target capabilities.
- `pub struct SeoTargetRecord`: Target descriptor carrying `slug`, `display_name`, `owner_module_slug`, and `capabilities`.
- `pub struct SeoTargetRegistry`: Shared capability registry storing registered SEO target providers and their metadata.
- `pub trait SeoTargetProvider`: Asynchronous trait implemented by owner modules to provide target state, route resolution, sitemap entries, and structured JSON-LD data.
- `pub struct SeoTargetImageRecord`: Self-contained image metadata DTO decoupled from internal media persistence.
- `pub mod schema`: Helper functions for structured JSON-LD data construction (`web_page`, `web_page_with_image`, `collection_page`, `collection_page_with_image`, `product`, `product_with_image`, `blog_posting`, `blog_posting_with_image`, `discussion_forum_posting`, `discussion_forum_posting_with_image`).
- `pub fn populate_image_template_fields(...)`: Template field population for SEO image tags.
- `pub fn register_seo_target_provider(...)`: Extension method for registering providers into `rustok_core::ModuleRuntimeExtensions`.
- `pub fn seo_target_registry_from_extensions(...)`: Helper for retrieving the registered SEO target registry from runtime extensions.
- `pub enum SeoTargetError`: Strongly-typed error enum covering slug validation, registration collisions, missing providers, and resolution failures.
- `pub struct GeneratedSeoMetadata`: AI-assisted SEO metadata payload.
- `pub fn seo_ai_verticals() -> Vec<SeoAiVertical>`: Discovery metadata for AI-assisted SEO generation.
- `pub fn validate_seo_metadata_payload(...) -> Result<(), SeoTargetError>`: Verification for generated SEO payloads.

## Platform Capability Bindings

- **SEO Target Capability Registry**: Acts as the shared registry and abstraction layer across domain modules and `rustok-seo`.
- **Runtime Extension Injection**: Injected into `rustok-core::ModuleRuntimeExtensions` during host initialization.
- **Structured Data Generation**: Provides JSON-LD Schema.org builders without pulling heavy external scrapers.

## Domain Collaborations

- **`rustok-core`**: Integration with `ModuleRuntimeExtensions` for provider discovery.
- **`rustok-outbox`**: Transactional event bus integration when server feature is active.
- **`rustok-seo`**: Consumes `SeoTargetRegistry` to drive routing, bulk operations, sitemaps, and metadata resolution.
- **Owner Modules (`rustok-pages`, `rustok-product`, `rustok-blog`, `rustok-forum`)**: Implement `SeoTargetProvider` and register into the shared registry.

## Minimum Contract Set

### Input DTOs/Commands
- `SeoTargetSlug`: Must be non-empty, alphanumeric with dashes/underscores, lowercase normalized.
- `SeoTargetImageRecord`: Carries optional `media_asset_id` and normalized URL, alt text, dimensions, and MIME type.
- `GeneratedSeoMetadata`: Input payload for AI-generated meta titles, descriptions, and OpenGraph attributes.

### Domain Invariants
- **Registry Uniqueness**: Each SEO target slug must be unique within the runtime registry; duplicate registration must return an error.
- **Fail Fast on Missing Registry**: Runtime consumers must fail fast if the shared target registry is missing rather than falling back to uncoordinated built-ins.
- **Media Decoupling**: Owner modules map media assets at provider boundaries via `SeoTargetImageRecord` without leaking private persistence models.
- **Tenant Context Awareness**: Target provider methods accept tenant and database context where required by provider implementations.

### Events / Outbox Side Effects
- Does not directly publish outbox events; provides the contracts for owner modules and `rustok-seo` to publish SEO lifecycle updates.

### Errors / Failure Codes
- `SeoTargetError::InvalidSlug`: Given slug string fails validation rules.
- `SeoTargetError::DuplicateSlug`: Target slug has already been registered in the registry.
- `SeoTargetError::TargetNotFound`: Requested target slug does not exist in registry.
- `SeoTargetError::CapabilityNotSupported`: Target does not support the requested capability kind (e.g. sitemaps or routing).
- `SeoTargetError::ValidationFailed`: Generated SEO metadata payload failed validation invariants.
