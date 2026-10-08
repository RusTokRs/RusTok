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
- Blog post slug history: `blog_post_slug_history` (migration `m20261008_000030`) records retired
  canonical slugs per tenant. Renaming a post keeps its old slug, public reads resolve retired
  slugs, and the Next storefront redirects permanently to the current slug.
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
