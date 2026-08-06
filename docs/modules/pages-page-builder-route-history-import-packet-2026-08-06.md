# Pages / Page Builder historical route import packet

Date: 2026-08-06  
Status: source-ready / execution-pending

## Rechecked source gap

The forward lifecycle path now records public route snapshots before unpublish or archive and writes `gone` aliases before physical delete. That path cannot reconstruct older history automatically:

- deleted Pages have already lost their current translations and page row;
- old non-builder publication did not retain a complete immutable route receipt;
- current draft or archived translations do not prove that a slug was public;
- Page Builder artifacts are not a complete source for every historical Pages route and are deleted with the page.

For those reasons, this slice deliberately rejects heuristic scans. Historical route recovery is an explicit operator import backed by external provenance.

## Owner command

`PageRouteHistoryImportService::import_public_routes` requires `pages:manage` and accepts one normalized source plus 1–100 route items:

```text
source
items[]:
  source_record_id
  page_id
  locale
  slug
```

Source names are lowercased and restricted to ASCII letters, digits, dots, underscores and dashes. Source record identifiers are trimmed, bounded and reject control characters. Locale and slug use the existing Pages normalization rules.

The entire batch runs in one transaction.

## Immutable provenance

Each accepted item creates a forward-only `page_route_history_imports` receipt keyed by:

```text
(tenant_id, source, source_record_id)
```

The receipt stores the normalized route payload, a canonical SHA-256 request hash, whether the page was already missing at first import, the importing actor and timestamp. It intentionally has no foreign key to `pages`, so provenance survives physical deletion.

Exact replay verifies the same payload and route ownership without adding another receipt. Reusing a provenance key with another page, locale or slug fails closed with `PAGE_ROUTE_HISTORY_IMPORT_CONFLICT`.

## Route composition

For every item the owner verifies, in order:

1. no current translation route is owned by another page;
2. no retained publication snapshot is owned by another page;
3. no incompatible immutable alias owns the claim;
4. the exact `page_route_publications` snapshot exists;
5. when the page is missing, the route is terminally `gone` or preserves an exact same-page redirect.

An existing page receives only the retained publication snapshot. It does not become `gone` while the page still exists. If that page is later deleted, the existing delete owner consumes the imported snapshot and writes the tombstone in the delete transaction.

A page already missing at import receives a direct `gone` alias for an unclaimed route. Existing same-page published-slug redirects are preserved rather than rewritten. A redirect-only missing-page import is accepted only when the page already has, or the same batch adds, at least one direct terminal `gone` route. This guarantees that the current resolver can fold every preserved redirect into `Gone` without guessing a canonical target.

## Fail-closed boundaries

The batch rolls back when it encounters:

- duplicate provenance identifiers inside the input;
- a provenance key bound to another payload;
- another tenant owning the supplied page identifier;
- current, snapshot or alias ownership overlap;
- an orphan current translation for a missing page;
- a missing page with redirect history but no terminal `gone` anchor.

No route, snapshot or receipt from a failed batch commits.

## Source evidence

- `crates/rustok-pages/src/migrations/m20260806_000012_create_page_route_history_imports.rs`;
- `crates/rustok-pages/src/entities/page_route_history_import.rs`;
- `crates/rustok-pages/src/services/page/route_history_import.rs`;
- `crates/rustok-pages/tests/page_route_history_import_sqlite.rs`;
- `crates/rustok-pages/contracts/evidence/pages-route-history-import-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-route-history-import.mjs`.

## Deliberate limits

This slice adds no automatic historical inference, GraphQL or REST mutation, admin UI, scheduled job, host route, cache policy, event schema or Page Builder/Fly behavior.

It does not claim that an external ledger is trustworthy. The operator remains responsible for validating source provenance before calling the owner. The Pages owner guarantees only normalized, bounded, auditable and conflict-safe composition of the supplied records.

No FFA/FBA promotion or runtime execution is claimed.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-route-history-import.mjs
cargo test -p rustok-pages \
  --test page_route_history_import_sqlite -- --nocapture
cargo test -p rustok-pages \
  --test page_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-pages \
  --test page_published_slug_route_alias_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Execution evidence remains pending.
