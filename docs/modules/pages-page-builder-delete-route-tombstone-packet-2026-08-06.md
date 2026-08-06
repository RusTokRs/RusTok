# Pages / Page Builder delete route tombstone packet

Date: 2026-08-06  
Status: source-ready / execution-pending

## Rechecked boundary

Current `main` already contains localized published slug redirects, a transport-neutral canonical/redirect/gone resolver, and host composition for `308`, `410`, `404` and `409`. The missing source slice was lifecycle ownership for a page that had been public, later left `published`, and was then physically deleted.

A never-published draft must not reserve a public route merely because it was deleted. Conversely, every localized route that was actually public must remain claimed and resolve as gone after deletion.

## Published route snapshot ledger

The forward-only `page_route_publications` table retains one immutable `(tenant_id, locale, slug)` claim and the owning page. It has no foreign key to `pages`, so the evidence survives physical page deletion.

When a page transitions from `published` to `draft` through unpublish, or from `published` to `archived`, `PageService` records its current localized routes before changing lifecycle state. Draft-to-archive and never-published draft deletion record no public history.

This forward owner path covers new lifecycle transitions. Historical route backfill/import policy remains open as a separate source slice.

## Delete owner composition

`PageService::delete` still rejects a currently published page. For an admitted non-published delete it now performs, in the same transaction:

```text
lock page and authorize delete
  → load retained public route snapshots
  → insert missing immutable gone aliases with reason "Page deleted"
  → preserve existing redirect aliases without rewriting them
  → delete bodies and translations
  → delete page
  → write NodeDeleted
  → commit
```

Exact existing gone rows are idempotent. Claim ownership or payload drift fails closed with the existing route resolution conflict.

## Redirect history after delete

A published slug rename already creates an immutable redirect to the page's current canonical route. Deletion does not rewrite that redirect row. Once the target page is physically absent and at least one retained target tombstone exists, `PageRouteService::resolve` folds the historical redirect into `Gone` rather than returning an unresolved redirect or exposing a stale canonical target.

Thus old redirects remain auditable history while every formerly public route produces the host's existing `410 Gone` response.

## Source evidence

- `crates/rustok-pages/src/migrations/m20260806_000011_create_page_route_publications.rs`;
- `crates/rustok-pages/src/entities/page_route_publication.rs`;
- `crates/rustok-pages/src/services/page/route.rs`;
- `crates/rustok-pages/src/services/page/lifecycle.rs`;
- `crates/rustok-pages/tests/page_delete_route_tombstone_sqlite.rs`;
- `crates/rustok-pages/contracts/evidence/pages-delete-route-tombstone-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs`.

## Deliberate limits

This slice does not change Page Builder/Fly behavior, page bodies, immutable artifacts, publish or rollback receipts, GraphQL/REST schemas, cache policy, event schemas, optional external event infrastructure, or FFA/FBA status.

It does not claim historical backfill, PostgreSQL execution, mounted host execution, browser evidence, workflows, CI or tenant rollout.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs
cargo test -p rustok-pages \
  --test page_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-pages \
  --test page_published_slug_route_alias_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Execution evidence remains pending.
