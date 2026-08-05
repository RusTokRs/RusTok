# Pages / Page Builder published slug route alias packet

Date: 2026-08-05  
Status: `source-ready / execution-pending`

## Purpose

This slice closes the first production part of the Pages canonical URL, redirect and route-collision gap.

A published slug rename now preserves the old public route in an append-only route ledger. The old claim cannot be reused by another page, and a transport-neutral Pages resolver returns the current localized canonical descriptor.

Page Builder and Fly behavior are unchanged. The ledger belongs to Pages because Pages owns localized slugs, public route identity and lifecycle policy.

## Source behavior

```text
published page
  locale en
  canonical /en/modules/pages?slug=about

metadata patch
  about → about-us
  → validate current translation and historical route claims
  → append redirect alias for en/about
  → replace current translation with en/about-us
  → advance page metadata version
  → emit NodeUpdated
  → commit one transaction

second metadata patch
  about-us → company
  → append redirect alias for en/about-us
  → current canonical becomes /en/modules/pages?slug=company

resolve en/about
  → redirect alias
  → current target page + locale
  → recompute current slug
  → /en/modules/pages?slug=company

resolve en/about-us
  → same current canonical

create another page with en/about
  → rejected as DuplicateSlug because immutable route history owns the claim
```

The target slug is intentionally not persisted in the alias. Resolution recomputes the current canonical descriptor, so multiple published renames do not create redirect chains.

## Draft policy

A draft-only rename does not create public route history. A slug that existed only on an unpublished draft can be reused after that draft moves to another slug.

This distinction prevents internal authoring names from becoming permanent public URL claims before publication.

## Ledger and collision policy

The new `page_route_aliases` table stores:

- tenant, source page, locale and historical slug;
- disposition (`redirect`, with `gone` reserved for the deletion slice);
- target page and target locale;
- immutable reason and timestamp.

The unique claim is:

```text
(tenant_id, locale, slug)
```

The ledger has no foreign key to the current `pages` row. Historical route identity must survive future deletion/tombstone work.

Current translation claims are checked first, then historical alias claims. If current and historical ownership somehow overlap, route resolution fails closed with `PAGE_ROUTE_RESOLUTION_CONFLICT`.

Missing routes use `PAGE_ROUTE_NOT_FOUND`.

## Canonical SEO route

Pages SEO canonical and alternate routes now publish localized host paths:

```text
/{locale}/modules/pages?slug={slug}
```

The existing unprefixed `/modules/pages?slug=` shape remains parseable as a legacy request, but is no longer emitted as canonical.

## Source evidence

- `crates/rustok-pages/src/migrations/m20260805_000010_create_page_route_aliases.rs`;
- `crates/rustok-pages/src/entities/page_route_alias.rs`;
- `crates/rustok-pages/src/services/page/route.rs`;
- `crates/rustok-pages/src/services/page/metadata.rs`;
- `crates/rustok-pages/src/services/page/persistence.rs`;
- `crates/rustok-pages/src/seo_targets.rs`;
- `crates/rustok-pages/tests/page_published_slug_route_alias_sqlite.rs`;
- `crates/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-published-slug-route-alias.mjs`.

## Boundaries

This slice does not:

- mount redirect or `410 Gone` responses in the public host;
- create deletion tombstones;
- backfill historical slug changes;
- add a new HTTP, GraphQL or server-function route;
- change Page Builder/Fly documents, publication, artifacts or bindings;
- change cache namespaces, key shape, generation policy or TTL;
- change event schemas or optional event infrastructure;
- promote FFA or FBA.

The existing `NodeUpdated` event remains the route-generation invalidation cause for published metadata changes.

## Remaining routing work

1. Mount `PageRouteService::resolve` in the public host and return canonical, redirect and gone responses after channel/module admission.
2. Record deletion tombstones without losing prior redirect history.
3. Define historical backfill/import policy.
4. Retain SQLite, PostgreSQL, host and browser execution evidence.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-published-slug-route-alias.mjs
cargo test -p rustok-pages \
  --test page_published_slug_route_alias_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Execution evidence remains pending.
