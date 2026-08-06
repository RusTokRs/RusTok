# Page Builder / Pages Parity Actualization

Date: 2026-08-06  
Status: current-source-overlay / delete-route-tombstone-source-ready / execution-and-rollout-open

This overlay rechecks the broad Pages and Page Builder plans against current `main` through PR #3020 and the present delete-route-tombstone slice. It supersedes stale open-checkbox wording where that wording conflicts with merged source, while retaining all execution gates as pending.

## Rechecked completed source

The following source claims remain supported by current code and retained contracts:

- the registered six-field Pages metadata contribution is shared by draft Fly and the published Pages-owned surface;
- the bespoke `PageMetadataEditor` and direct workspace metadata write are absent;
- reviewed publication owns sanitization, runtime materialization, immutable artifact persistence and durable lifecycle receipts;
- rollback selects a prior immutable manifest without compiling the current draft;
- public detail/list locale fallback is requested locale → tenant default → platform fallback;
- published slug changes retain immutable localized redirects and cannot release old public claims;
- the registered host route decision precedes SEO/SSR and composes canonical, `308`, `410`, `404` and `409` outcomes;
- native cache, route admission, immutable artifact selection, production generation gate, event-profile parity, anonymous dependency graph and SSR document boundaries remain source-ready.

No test, verifier, Cargo, database, server-function, browser, workflow, CI or rollout execution is inferred from those source claims.

## Delete route tombstones

Delete route tombstones are now source-ready.

Pages records localized route snapshots only when a page actually leaves `published`. The snapshot ledger survives physical deletion and does not reserve routes for never-published drafts. A later admitted delete writes missing `gone` aliases in the same owner transaction before translations and the page row are removed.

Existing published-slug redirects remain immutable. After their target page is deleted, route resolution uses the target's retained tombstone to return `Gone`, preserving redirect history without exposing a stale canonical target.

Page Builder behavior is unchanged. The slice changes only Pages lifecycle, route-history persistence and route resolution.

## Current parity matrix delta

| Capability | Source state | Execution state |
| --- | --- | --- |
| Published slug aliases and localized canonical routes | Source-ready | SQLite/PostgreSQL/SEO execution pending |
| Host canonical/redirect/gone response | Source-ready | Registered server-function and host SSR execution pending |
| Delete route tombstones for new lifecycle transitions | Source-ready | SQLite/PostgreSQL/host execution pending |
| Historical route backfill/import | Open | Not implemented |
| Authenticated real-DOM inline editing | Open | Not implemented |

## Next cursor

1. Run the delete tombstone verifier and focused SQLite regression.
2. Run the host route response and published slug alias packets.
3. Define a bounded historical route backfill/import policy without weakening immutable claims.
4. Continue the retained locale, native route/cache, production gate, metadata browser and anonymous artifact execution sequence.
5. Complete workflow and observed tenant rollout evidence before FFA/FBA promotion.

## Suggested maintainer validation

```bash
node crates/rustok-pages/scripts/verify/verify-pages-delete-route-tombstone.mjs
cargo test -p rustok-pages \
  --test page_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-pages \
  --test page_published_slug_route_alias_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

These commands were intentionally not run by the implementation agent.
