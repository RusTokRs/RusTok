# SEO Crate API Specification

## Platform Capability Bindings
- **RusToKModule Implementation**: `SeoModule` in `src/module.rs`
- **Module Slug**: `seo`
- **Module Name**: `SEO`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `["content"]`
- **Permissions**:
  - `SEO_READ`
  - `SEO_UPDATE`
  - `SEO_PUBLISH`
  - `SEO_GENERATE`
  - `SEO_MANAGE`
- **Event Listeners**:
  - `SeoCanonicalUrlRedirectHandler`: Automatically handles URL redirects upon content slug changes.
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (10 migrations covering redirects, metadata, structured data, and sitemaps).

## Domain Collaborations
- **Content**: Subscribes to content lifecycle events to create canonical redirects and update sitemap entries.
- **Media**: Validates and attaches OpenGraph and Twitter card image assets.
- **Tenant**: Multi-tenant host domain resolution for sitemaps, robots.txt, and canonical origins.
- **Translation Targets**: Integrates with translation target providers for localized meta title and description variations.
- **Outbox**: Publishes SEO update and sitemap generation events into transactional outbox.
- **GraphQL & HTTP**: Exposes `SeoQuery`, `SeoMutation`, and axum HTTP controllers for `robots.txt` and `sitemap.xml`.

## Minimum Contract Set
- `SeoMetadataService`: Page and entity metadata resolution, fallback rules, structured data validation.
- `SeoRedirectService`: Redirect pattern evaluation, loop prevention, and generation management.
- `SeoSitemapService`: Sitemap generation, chunking, submission to external search engines, and robots.txt rendering.
- `SeoRoutingService`: Canonical URL resolution, trailing slash enforcement, and hreflang clustering.
- `SeoBulkService`: Bulk SEO metadata import/export, patch operations, and repair jobs.

### Input DTOs/Commands
- `SeoMetaInput`: Title, description, keywords, canonical URL, OpenGraph, Twitter card, robots directives, structured data blocks.
- `SeoRedirectInput`: Source path, destination URL, match type (Exact, Prefix, Regex), HTTP status code (301, 302, 307, 308), active flag.
- `SeoBulkMetaPatchInput`: Bulk field patch operations for multi-document SEO updates.
- `SeoPageContext`: Request context for resolving effective SEO document for a path or entity.

### Domain Invariants
- **Redirect Loop & Self-Redirect Prohibition**: Redirect target cannot resolve to the source pattern.
- **Valid Destination URLs**: Destination URLs must be valid HTTP/HTTPS URLs or valid relative paths without user credentials.
- **JSON-LD Schema Validity**: Structured data blocks must be valid JSON-LD objects with declared `@type`.
- **Public Origin Isolation**: Sitemap and robots origins must resolve to trusted tenant domains and reject private IP ranges.
- **Deterministic Sitemap Keys**: Sitemap event and artifact keys are deterministic and outcome-sensitive.

### Events / Outbox Side Effects
- `DomainEvent::SeoMetaUpdated`: Emitted when SEO metadata for a target entity changes.
- `DomainEvent::SeoRedirectCreated` / `SeoRedirectUpdated`: Emitted upon redirect rule modifications.
- `DomainEvent::SeoSitemapGenerated`: Emitted upon background completion of sitemap index files.

### Errors / Failure Codes
- `SeoError::Validation`: Input validation or schema format error.
- `SeoError::RedirectLoop`: Circular redirect detected.
- `SeoError::EntityNotFound`: Target metadata or redirect record not found.
- `SeoError::Database`: Database storage layer failure.
- `SeoError::Core`: Core platform integration failure.
