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
  locale columns to `VARCHAR(32)` without narrowing rollback. A later wave
  rollback is also forbidden from returning revision locales to `VARCHAR(16)`.
- **Groups** — `groups` owns only language-neutral identity/policy state and
  `group_translations` owns title, summary, and body under the unique
  `(tenant_id, group_id, locale)` key. PostgreSQL CHECK constraints and SQLite
  triggers enforce canonical locale tags and reject localized presentation keys
  in base metadata. The service consumes the host-resolved effective locale and
  contains no English or arbitrary first-row fallback.
- **Product catalog** — the product-owned schema verifier remains the delegated
  guard for translation ownership and locale widening.
- **Search and Index locale attribution** — query logs and localized content/product
  projections finish at `VARCHAR(32)`. PostgreSQL and MySQL use forward widening;
  SQLite keeps TEXT affinity and therefore needs no destructive rebuild merely to
  change a declared `VARCHAR` length.
- **Content, blog, taxonomy, comments, and profiles locale widths** — registered
  forward-only owner migrations widen localized/preference columns to
  `VARCHAR(32)` without narrowing rollback.
- **Blog canonical routing** — `blog_posts.slug` is an explicitly locale-neutral
  canonical route identifier. Localized post copy stays in
  `blog_post_translations`; localized category names, slugs, and descriptions
  stay in `blog_category_translations`.
- **Profiles localized copy** — `profiles` owns language-neutral identity, media,
  visibility, status, and locale preference only. `display_name` and `bio` are
  authoritative in `profile_translations`. Unmatched legacy copy is retained as
  `und`; runtime reads do not treat `und` as fallback; writes require a
  host-resolved effective locale.
- **Flex localized storage** — schema name/description and localized entry JSON
  are moved out of base rows. Their historical source language was not recorded,
  so clean-install backfills now use `und`, never the current tenant default or
  platform English fallback. Both data-moving migrations are fixture-declared in
  the backfill registry; schema rollback prefers the preserved provenance row.
- **Marketplace Seller prose** — onboarding notes and suspension reasons flow
  directly from receipted commands into immutable locale-attributed events in the
  same transaction. Legacy snapshots are preserved before obsolete base columns
  are dropped.
- **OAuth applications** — protocol identity and credentials remain in
  `oauth_apps`; name and description live in tenant-safe translation rows.
  Legacy copy is retained as `und`. Manual writes require effective locale and
  commit base state plus translation atomically; manifest-generated English copy
  uses explicit `en`; runtime reads never return `und` as a translation fallback.
- **Registry publish/release copy** — runtime default remains `en`, while unknown
  historical copy is stored as `und`. Backfill and rollback placeholders are
  backend-aware, and rollback prefers the provenance row before runtime policy.
  Databases that already applied the former synthetic-`en` migration still need
  an operator audit before relocalization.
- **Commerce collections/categories and transaction line copy** — collection and
  category locale columns finish at `VARCHAR(32)`; cart/order line titles live in
  parallel translation rows and are removed from base lines.
- **Customer locale policy** — clean and upgraded schemas use `VARCHAR(32)`;
  PostgreSQL and SQLite validation enforce the same limit, and MySQL widening is
  declared.
- **Legacy ecommerce cutovers** — region, stock-location, shipping-option,
  price-list, and shipping-profile migrations preserve unknown source language
  as `und` rather than fabricating English provenance.

## Open owner cutovers

These are not accepted exceptions. Each remains an explicit owner migration and
runtime cutover target:

- `alloy-script-display-copy`: `scripts.name` and `scripts.description` still mix
  tenant-visible presentation with identity. Because uniqueness is currently
  based on `name`, Alloy must first define a stable language-neutral key before
  moving copy to translations and backfilling legacy rows with truthful
  provenance.
- `rbac-role-permission-display-copy`: role slug and permission resource/action
  are valid identity, but role name/descriptions and permission descriptions are
  inline UI copy. Built-in seeds need explicit-locale translations; tenant custom
  copy needs effective-locale command identity.
- `channel-display-name`: `channels.slug` is the stable identifier while
  `channels.name` remains human-facing copy in the base table. The Channel owner
  must add translation records and exact-locale transports.
- `workflow-display-copy`: workflow name/description are inline and name is also
  the current uniqueness key. The owner needs a locale-neutral workflow key before
  the atomic storage/runtime cutover.
- `mcp-management-display-copy`: MCP client display name/description remain inline.
  `token_name` must be explicitly classified as a language-neutral operator label
  or moved to locale-attributed presentation storage; it must not remain
  semantically ambiguous.
- `ai-control-plane-display-copy`: provider/tool profile display copy and chat
  session titles remain inline. Profile slugs can remain identity; profile copy
  needs translations, while session titles need source-locale attribution or
  locale-aware rows. Provider errors remain technical facts, not translations.
- `order-change-prose-locale`: `order_changes.description` is immutable human
  prose without source-locale attribution. The Order owner must store normalized
  command locale or move prose into locale-attributed change bodies while
  preserving replay identity and historical snapshots.

## Interpretation rules

A string in a base row is not automatically a violation. Stable handles,
canonical keys, protocol identifiers, legal identity fields, personal names,
external references, and explicitly locale-neutral route identifiers may remain
in base rows. The owner must state that semantic contract. Human-facing display
copy must not be reclassified as an identifier merely to avoid a translation
cutover.

JSONB remains valid for configuration, internal audit payloads, and flexible
non-copy data. It is not a canonical replacement for parallel localized business
records. A reserved top-level presentation field may be rejected in base JSON
while a nested provider-schema key with the same technical name remains valid.

An `und` row is retained data with unknown language provenance. It is not a
request fallback and must not be silently returned for another requested locale.
Already-applied databases that used synthetic `en` cannot be bulk-rewritten
safely without an operator audit because a row may have been edited later as real
English copy.

## Verification

- `npm run verify:i18n:contract`
- `node scripts/verify/verify-db-multilingual-contract.mjs`
- owner migration tests and PostgreSQL/MySQL/SQLite migration compatibility checks

The static verifier proves declared source contracts. Runtime schema inspection
and migration execution remain separate evidence and are intentionally not
claimed by this audit.
