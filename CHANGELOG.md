## 2026-06-07 — SEO Phase D (D5-D7) batch progress

- Hardened SEO index repair/replay idempotency: historical replay now deduplicates repeated operator runs and reuses transition keys across replay retries.
- Added replay failure-recovery coverage (`historical_replay_deduplicates_repeat_runs`, `historical_replay_retries_failed_delivery_without_duplicate_rows`) and transport parity coverage (`memory` + `streaming` reliability levels).
- Extended Next Admin SEO operator panel with failure drilldown (`failureSamples`) and updated GraphQL/REST contract mapping.
- Added renderer parity snapshot tests, including deterministic primary snapshot assertion and token-based normalization for non-deterministic payload fields.
- Added SEO replay/repair operational runbook with troubleshooting and verification checklist.

# Changelog

All notable changes to RusToK are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Newsletter Module (M-9)
- **New standalone module `rustok-newsletter`** for cross-domain email campaign management
- `rustok-newsletter-api` crate with contracts:
  - `NewsletterContentProvider` trait for source modules (blog, forum, commerce)
  - `SubscriberPort` and `CampaignPort` typed ports
  - Shared types: `ContentSourceSlug`, `SubscriberStatus`, `CampaignStatus`
- `rustok-newsletter` implementation crate (29 files):
  - Domain: subscriber/campaign lifecycle state machines with validated transitions
  - Services: `SubscriberService` (subscribe, confirm, unsubscribe, update, delete) and `CampaignService` (create, update, schedule, cancel, delete)
  - Entities: 3 tables (`newsletter_subscribers`, `newsletter_campaigns`, `newsletter_subscriptions`)
  - Migration `m20261009_000040_create_newsletter_tables` with 6 indexes
  - GraphQL: 4 queries + 10 mutations for subscriber and campaign management
  - Ports: typed port implementations following platform contracts
  - Module registration: `rustok-module.toml`, workspace integration
- Architecture: Newsletter aggregates content from blog/forum/commerce via `NewsletterContentProvider` trait (inversion of control pattern)

#### Content Portability Platform (M-7)
- **New platform capability** for unified content import/export across all modules
- `rustok-content-portability-api` crate with contracts:
  - `ContentImporter<Source, Target>` and `ContentExporter<Source, Target>` traits
  - Format descriptors: JSON, CSV, WordPress XML, Markdown, Custom
  - Context types: `ImportContext`, `ExportContext` with tenant isolation
  - Result types: `ImportResult<T>`, `BatchImportResult<T>`, `ExportResult<T>`
  - Validation framework: `ValidationResult`, `FieldValidation`, `validate_fields!` macro
  - Progress tracking: `ProgressCallback` for batch operations
- `rustok-content-portability` implementation crate:
  - `JsonFormatHandler`: JSON parse/serialize with pretty-print option
  - `CsvFormatHandler`: CSV parse/serialize with custom delimiters and headers
  - `ImportService`: import from JSON/CSV files and bytes
  - `ExportService`: export to JSON/CSV files and bytes
  - File I/O helpers: `read_file`, `write_file`, `file_exists`, `file_size`
  - Comprehensive unit tests for all components
- Architecture: Platform capability (support crate), not tenant-toggled module, following `rustok-api`/`rustok-core` pattern

#### Blog Module Enhancements (M-5)
- **Featured/Pinned Posts** feature:
  - Migration `m20261009_000035_add_blog_post_pinned`: `is_pinned` boolean + `pinned_at` timestamp with composite index
  - Entity fields: `is_pinned: bool`, `pinned_at: Option<DateTimeWithTimeZone>`
  - DTO: `is_pinned` in `CreatePostInput`/`UpdatePostInput`, `is_pinned`+`pinned_at` in `PostResponse`/`PostSummary`
  - Service: `pin_post`, `unpin_post`, `set_pinned` methods with validation (only published posts can be pinned)
  - Queries: public listing sorts by `is_pinned DESC, pinned_at DESC, published_at DESC, id DESC`
  - GraphQL: `isPinned: Boolean!`, `pinnedAt: String`, `pinPost(id)`, `unpinPost(id)` mutations
  - REST: `POST /api/blog/posts/{id}/pin` and `POST /api/blog/posts/{id}/unpin` endpoints
  - All test files updated with `is_pinned: None` field

### Added (continued)
- Blog post slug redirects use the shared canonical URL registry of `rustok-content`
  (`canonical_url` / `url_alias`) through `CanonicalUrlWriter`. Renaming a post keeps its old
  route as an alias, public reads resolve it to the current post, a new post may take a retired
  slug, and deleting a post purges its routes. Blog keeps no slug-history table. Content
  orchestration uses the same Blog route definition.
- Blog post slugs for non-ASCII titles are transliterated through the shared Taxonomy route-key
  normalizer, so Cyrillic titles no longer fail with "Slug cannot be empty".
- Page Builder scenario-baseline revision history: every accepted baseline mutation (`create`,
  `replace`, `delete`) appends a record to `page_builder_scenario_baseline_revisions` inside the
  same transaction, and `pageBuilderScenarioBaselineHistory` /
  `PageBuilderScenarioBaselineService::history` read the trail back. The migration that creates the
  table is registered for the first time and applies on next startup; it creates an empty table, so
  there is no backfill and existing baselines are untouched.

### Changed
- Blog `published_at` is the first publication time: republishing, unpublishing, archiving, and
  restoring no longer change or clear it. `updated_at` tracks the last change.
- Blog public list, RSS feed, and related-article reads use the requested locale instead of the
  tenant default locale.
- Blog post SEO fallbacks use `seoTitle`/`seoDescription` first and a localized default description;
  Article JSON-LD adds `dateModified` and an absolute URL.

### Removed
- Blog `view_count` field from the entity, REST/GraphQL-facing DTOs, search projection, and schema
  (migration `m20261008_000031`). The field had no writer and always returned 0.

### Fixed
- Blog `featured_image_url` is validated on create and update: only absolute `http`/`https` URLs or
  root-relative paths, at most 2048 characters. Other schemes were previously stored and reached SEO
  and JSON-LD output.
- Blog RSS feed escapes a literal `]]>` inside CDATA sections, which previously broke the XML.
- The scenario-baseline revision history existed as schema only: the migration was not in the
  `PagesModule` migration list and its entity was not part of the module tree, so the promotion
  trail the schema promised was never written, with or without a reader. The migration is now
  registered as `m20261008_000001_create_scenario_baseline_revision_history` — renamed from the
  `m20260714_000003` it was authored as, because the composed migration plan is name-sorted and
  must stay append-only, and it had never been applied anywhere under the old name. See
  `docs/audits/page-subsystem-engineering-audit-2026-10-08.md` (F-9).

### Deprecated
- _No unreleased deprecations yet._

### Removed
- _No unreleased removals yet._

### Security
- _No unreleased security updates yet._

---

## Release note policy

- Keep this file release-oriented: each released version gets one section
  `## [X.Y.Z] - YYYY-MM-DD` with standard subsections.
- Avoid sprint diaries and implementation logs in this file.
- For deep implementation context, link to canonical docs in `docs/` and `DECISIONS/`.
- Do not reference missing files; every link must resolve inside the repository.

## Version section template

```md
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Deprecated
- ...

### Removed
- ...

### Security
- ...
```

## Hotspot contract (DOC-12 / H5)

- Hotspot: `H5` (Release and compatibility communication).
- Doc contracts updated: `CHANGELOG.md`.
- Owner scope: platform docs owner.
- Residual drift risk:
  - root onboarding docs (`README.md`, `README.ru.md`) may update earlier
    than release note section in this file;
  - without mandatory release cutover checklist, risk of stale compatibility notes
    remains until complete DOC-12 (B14) closure.
