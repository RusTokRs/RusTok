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
- Page Builder scenario-baseline revision history: every accepted baseline mutation (`create`,
  `replace`, `delete`) appends a record to `page_builder_scenario_baseline_revisions` inside the
  same transaction, and `pageBuilderScenarioBaselineHistory` /
  `PageBuilderScenarioBaselineService::history` read the trail back. The migration that creates the
  table is registered for the first time and applies on next startup; it creates an empty table, so
  there is no backfill and existing baselines are untouched.

### Changed
- _No unreleased changes yet._

### Fixed
- The scenario-baseline revision history existed as schema only: `m20260714_000003` was not in the
  `PagesModule` migration list and its entity was not part of the module tree, so the promotion
  trail the schema promised was never written, with or without a reader. See
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
