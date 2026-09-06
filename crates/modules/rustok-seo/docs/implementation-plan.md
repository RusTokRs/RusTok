# rustok-seo implementation plan

## Current state

`rustok-seo` is the tenant-aware SEO runtime for metadata, redirects,
sitemaps/robots, diagnostics, typed schema blocks, bulk remediation, event and
index delivery tracking, and replay control. Entity authoring remains in owner
modules; SEO admin is infrastructure control-plane only. REST and GraphQL are
additive `v1` surfaces, while Rust and Next storefronts consume the canonical
`SeoPageContext` contract.

The redirect read cache is byte-weighted and tenant-scoped. Redirect mutations
write the `SeoRedirectUpserted` / `SeoRedirectDisabled` outbox event and a
`source_kind=redirect` delivery row in the same transaction, then invalidate the
committing process after commit. Every Full/API/SSR serving runtime owns a
supervised 5-second reconciliation worker; registry-only and worker-only hosts do
not poll a cache they cannot serve. Startup reads the append-only redirect-row
count and high-water `(created_at, id)` cursor before clearing all local redirect
entries. Later rows invalidate exact tenants in batches of 256, up to 16 pages
per poll. The independent row count is compared with the number of cursor rows
processed; clock skew, late/out-of-order commits, deletion, or an oversized
backlog creates a mismatch and forces a safe full clear plus reseed. The leading
`(source_kind, created_at, id)` index supports count/cursor scans. Readiness is
healthy only after seed/clear succeeds and becomes critical while the worker is
terminal or a database/query failure is being retried. Cross-replica freshness
therefore no longer depends on local-only module-event delivery.

Source evidence now drives two independent serving contexts against one durable
change log. It proves startup seed-before-clear, exact tenant invalidation, ten
new rows consumed through multiple three-row test pages, an out-of-order row
behind the cursor forcing count-mismatch full recovery, database table outage
and automatic restart recovery, and terminal isolation where one aborted replica
is no longer running or ready while the other remains healthy. The architecture
guard locks the transaction-before-local-invalidation ordering, cursor index,
worker readiness, source evidence markers and permanent cache workflow command.
This evidence is source-complete but is not compiled verified until the targeted
Rust gate passes on the same revision.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- The admin package has core/transport/UI ownership and a neutral
  `HostRuntimeContext` native path; GraphQL and REST remain parallel control
  surfaces. `scripts/verify/verify-seo-admin-boundary.mjs` protects that
  boundary.
- The intended `MediaAssetReadPort` / `media.asset_read.v1` consumer contract
  is implemented through `SeoMediaAssetReadProvider` and the host-composed
  `MediaAssetReadPort`. The shared target image contract supports an optional
  owner-provided media asset UUID, and Product forwards its canonical
  `ProductImageResponse.media_id`. URL-only target records retain their
  owner-provided descriptors. Other target providers and live consumer
  execution remain required before `boundary_ready`.
  The source-locked contract and evidence are
  `crates/rustok-seo/contracts/seo-fba-registry.json` and
  `crates/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json`.

## Next results

1. **Execute the source-complete multi-replica redirect cache gate.** Run startup
   count/cursor-before-clear ordering, exact tenant invalidation, more-than-one-
   batch catch-up, count-mismatch full-clear recovery, database outage/recovery,
   serving-host scoping and terminal-worker readiness on one reconciled `main`
   revision.
   **Depends on:** the permanent cache workflow or another Rust 1.96 environment
   with the SEO cursor index migration applied.
   **Done when:** the server lib test and architecture guard pass on the same
   revision, every discovered failure is fixed, and the verified revision is
   recorded without copying raw logs.

2. **Execute the D8 backend and host matrix.** Capture deployed GraphQL/REST
   parity, outbox/index before-after counters, Next robots/sitemap/metadata,
   Leptos page-context, RBAC/module gating, and replay/idempotency behavior.
   Include the `MediaAssetReadPort` success and degraded cases:
   `omit_image_metadata`, `keep_existing_seo_image`, and relative-URL proxy
   fallback. Done when required live artifacts are attached and high-severity
   parity defects are closed.

3. **Close D9 with operational evidence and owner sign-off.** Run the SEO
   backlog, partial-indexing, and replay/reindex incident procedures against
   the live packet; record redacted evidence, counters, recovery outcomes, and
   signed platform/frontend/operator review. Done when the fixture checklist
   moves from pending static seed to signed without bypassing closeout rules.

4. **Extend storefront SEO only through additive owner routes.** When a new
   Next route owner exists, add its `SeoPageContext` mapping, route matrix
   entry, semantic fallback classification, and Rust/Next fixture evidence.
   Done when no host adds a local target mapping, raw schema handling, or a
   divergent metadata precedence rule.

## Verification

- `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`
- `npm run verify:seo:fba`
- `node scripts/verify/verify-seo-admin-boundary.mjs`
- `cargo test -p rustok-server --test seo_redirect_cache_reconciliation_guard`
- `cargo test -p rustok-server seo_redirect_cache_reconciliation --lib`
- Targeted backend, outbox/index, Next, Leptos, media fallback, cursor/count/index,
  and incident runtime checks defined by the live-evidence template.

## References

- [SEO documentation](./README.md)
- [SEO replay/repair runbook](./replay-repair-runbook.md)
- [SEO operations runbook](./operations-runbook.md)
- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Runtime parity fixtures](../../../apps/next-frontend/contracts/seo/runtime-parity-fixtures.json)
