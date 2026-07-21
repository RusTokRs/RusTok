# Multilingual database contract audit

This audit applies the accepted language-agnostic storage contract from
`database.md`, `i18n.md`, and the parallel-localized-records ADR to current
write-side migrations.

The executable registry is `database-multilingual-contract.json`. A surface is
listed as `enforced` only when its concrete migration markers and registration
are checked by `scripts/verify/verify-db-multilingual-contract.mjs`.

## Contract

- Base business rows own language-agnostic identity, lifecycle, relations,
  counters, policy, and configuration.
- Localized short business copy belongs to parallel `*_translations` rows.
- Heavy localized content belongs to parallel `*_bodies` rows when appropriate.
- Tenant default/effective locale controls selection and fallback only; it does
  not physically own localized fields.
- Normalized locale columns use a platform-safe width of at least `VARCHAR(32)`.
- A forward widening migration is irreversible: down migrations must not narrow
  locale columns and risk truncating valid BCP47-like tags.

## Enforced surfaces

- **Foundation locale policy** — `tenants.default_locale`,
  `tenant_locales.locale`, and `tenant_locales.fallback_locale` are widened to
  `VARCHAR(32)` by an irreversible PostgreSQL migration.
- **Pages** — base page/menu rows remain language-agnostic; translations and
  bodies are parallel records. A forward migration widens page, body, menu, and
  menu-item locale columns to `VARCHAR(32)`.
- **Forum** — category/topic/reply base rows remain language-agnostic; localized
  records are parallel and the core tenant-integrity migration widens their
  locale columns to `VARCHAR(32)` without narrowing rollback.
- **Groups** — the fresh owner schema uses a language-agnostic `groups` table and
  `group_translations.locale VARCHAR(32)` from creation.
- **Product catalog** — the product-owned schema verifier remains the delegated
  guard for translation ownership and locale widening.

## Open owner cutovers

These are not accepted exceptions. They remain explicit migration targets:

- `auth-oauth-app-copy`: `oauth_apps.name` and `oauth_apps.description` still
  store display copy inline. `rustok-auth` owns the atomic translation-table and
  transport cutover.
- `profiles-display-name`: `profiles.display_name` duplicates localized
  `profile_translations.display_name`; preferred/translation locale columns are
  also narrower than the platform contract. The owner must define whether any
  display name is locale-neutral identity, backfill translations, remove the
  duplicated localized copy, and widen locale columns.
- `content-locale-width`: node/category/meta translations and bodies use legacy
  narrow locale columns. `rustok-content` needs one append-only widening slice
  covering all owner tables.
- `blog-locale-width`: Blog translations use a narrow locale column. The owner
  must also document whether `blog_posts.slug` is a locale-neutral canonical
  route key; otherwise slug ownership must move to translations.
- `taxonomy-locale-width`: term translations and aliases use narrow locale
  columns.
- `comments-locale-width`: comment bodies use a narrow locale column.
- `customer-locale-width`: customer locale preference uses a narrow locale
  column.

## Interpretation rules

A string in a base row is not automatically a violation. Stable handles,
canonical keys, protocol identifiers, external references, and explicitly
locale-neutral route identifiers may remain in base rows. The owner must state
that semantic contract. Human-facing display copy must not be reclassified as
an identifier merely to avoid a translation-table cutover.

JSONB remains valid for configuration, internal audit payloads, and flexible
non-copy data. It is not a canonical replacement for parallel localized
business records.

## Verification

- `npm run verify:i18n:contract`
- `node scripts/verify/verify-db-multilingual-contract.mjs`
- owner migration tests and PostgreSQL migration compatibility checks

The static verifier proves declared source contracts. Runtime schema inspection
and migration execution remain separate evidence and are intentionally not
claimed by this audit.
