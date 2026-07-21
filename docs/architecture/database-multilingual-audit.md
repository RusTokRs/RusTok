# Multilingual database contract audit

This audit applies the accepted language-agnostic storage contract from
`database.md`, `i18n.md`, the parallel-localized-records ADR, and the truthful
legacy-locale-provenance ADR to current write-side migrations.

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
- Legacy text with known provenance keeps its normalized locale. Unknown legacy
  text uses `locale = NULL` with explicit provenance where the schema supports it,
  or the storage-only BCP47 tag `und` in a non-null translation table.
- `PLATFORM_FALLBACK_LOCALE`, a tenant's current default locale, and hardcoded
  `en` are runtime policy, not evidence of a legacy row's original language.

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
- **Groups** — `groups` owns only language-neutral identity/policy state and
  `group_translations` owns title, summary, and body under the unique
  `(tenant_id, group_id, locale)` key. PostgreSQL CHECK constraints and SQLite
  insert/update triggers enforce canonical normalized locale tags and localized
  presentation shape. They also keep `groups.metadata`,
  `group_memberships.metadata`, and `group_feature_bindings.configuration`
  language-agnostic by rejecting reserved top-level presentation fields while
  permitting nested technical provider-schema configuration. The service accepts
  the host-resolved effective locale, requires the exact translation row, scopes
  catalog/search to that locale, counts title/summary limits as Unicode scalar
  values, and contains no English or arbitrary first-row fallback.
- **Product catalog** — the product-owned schema verifier remains the delegated
  guard for translation ownership and locale widening.
- **Search locale attribution** — clean query-log storage uses `VARCHAR(32)` and
  a registered forward migration widens PostgreSQL/MySQL schemas. SQLite keeps
  TEXT affinity and therefore requires no destructive table rebuild merely to
  change the declared length.
- **Content, blog, taxonomy, comments, and profiles locale widths** — registered
  forward-only owner migrations widen their localized/preference columns to
  `VARCHAR(32)` without narrowing rollback. These owner slices currently express
  the production width change through PostgreSQL DDL; SQLite does not enforce
  declared `VARCHAR` lengths.
- **Blog canonical routing** — `blog_posts.slug` is an explicitly locale-neutral
  canonical route identifier. Localized post display copy stays in
  `blog_post_translations`; localized category names, slugs, and descriptions
  stay in `blog_category_translations`.
- **Profiles localized copy** — `profiles` now owns only language-neutral identity,
  media, visibility, status, and locale preference. `display_name` and `bio` are
  authoritative only in `profile_translations`. The migration retains unmatched
  legacy base copy under storage-only locale `und`, blocks a conflicting existing
  `und` row, and drops `profiles.display_name`. Runtime reads do not treat `und`
  as request fallback and fail closed when no requested/preferred/tenant-default
  translation is available. Writes require a host-resolved effective locale;
  changing `preferred_locale` changes selection policy only and never copies text
  into a different locale as a fabricated translation.
- **Marketplace Seller prose** — `marketplace_sellers` now stores only
  language-neutral seller state. Onboarding notes and suspension reasons flow
  directly from the receipted command into immutable, locale-attributed seller
  events in the same transaction. Reads project prose only from the event log;
  the preceding migration preserves legacy snapshots and the final migration
  drops `onboarding_note` and `suspension_reason` from the base table.
- **OAuth applications** — `oauth_apps` now owns protocol identity, credentials,
  grants, redirect URIs, status, and configuration only. Human-facing name and
  description live in `oauth_app_translations` under
  `(tenant_id, app_id, locale)`. Legacy copy is retained as `und` before base
  columns are dropped. Admin GraphQL propagates the host effective locale and
  requires an exact translation; `und` is deleted once a command supplies known
  locale provenance.
- **Registry publish/release copy** — runtime default locale remains `en`, while
  historical copy with unknown provenance is stored as `und`. The migration no
  longer asserts that legacy text was English merely because English is the
  runtime fallback.
- **Commerce collections/categories** — a forward-only PostgreSQL/MySQL/SQLite
  migration widens collection and product-category translation locales to
  `VARCHAR(32)` and is registered after both owner tables.
- **Cart/order line copy** — localized titles live in parallel translation rows,
  locale storage is `VARCHAR(32)`, and the base line-item title columns are
  removed.
- **Customer locale policy** — clean and upgraded schemas use `VARCHAR(32)`;
  PostgreSQL and SQLite validation enforce the same 32-byte contract, and the
  upgraded migration also declares MySQL widening.
- **Legacy ecommerce cutovers** — region, stock-location, shipping-option,
  price-list, and shipping-profile migrations preserve unknown source language
  as `und` rather than fabricating English provenance.

## Open owner cutovers

No known multilingual write-side storage exceptions remain in the executable
registry. Newly discovered exceptions must be added back as explicit owner-owned
migration targets rather than hidden behind runtime fallback.

## Interpretation rules

A string in a base row is not automatically a violation. Stable handles,
canonical keys, protocol identifiers, legal identity fields, external
references, and explicitly locale-neutral route identifiers may remain in base
rows. The owner must state that semantic contract. Human-facing display copy
must not be reclassified as an identifier merely to avoid a translation-table
cutover.

JSONB remains valid for configuration, internal audit payloads, and flexible
non-copy data. It is not a canonical replacement for parallel localized
business records. A reserved top-level presentation field is rejected in Groups
base JSON, but a nested provider-schema key with the same technical name is not
automatically localized copy.

An `und` row is retained data with unknown language provenance. It is not a
request fallback and must not be silently returned for another requested locale.
Already-applied databases that used synthetic `en` cannot be bulk-rewritten
safely without an operator audit because a row may have been edited later as
real English copy.

## Verification

- `npm run verify:i18n:contract`
- `node scripts/verify/verify-db-multilingual-contract.mjs`
- owner migration tests and PostgreSQL/MySQL/SQLite migration compatibility checks

The static verifier proves declared source contracts. Runtime schema inspection
and migration execution remain separate evidence and are intentionally not
claimed by this audit.
