# rustok-seo implementation plan

## Current state

`rustok-seo` is the tenant-aware SEO runtime for metadata, redirects,
sitemaps/robots, diagnostics, typed schema blocks, bulk remediation, event and
index delivery tracking, and replay control. Entity authoring remains in owner
modules; SEO admin is infrastructure control-plane only. REST and GraphQL are
additive `v1` surfaces, while Rust and Next storefronts consume the canonical
`SeoPageContext` contract.

The redirect read cache is byte-weighted and tenant-scoped. Mutations invalidate
the committing process immediately and emit transactional
`SeoRedirectUpserted` / `SeoRedirectDisabled` events, but the current module
listener bus is local delivery only. Other replicas can therefore retain the
previous redirect set until the 30-second TTL expires. Durable cross-replica
recovery is an explicit open owner result, not an implicit cache guarantee.

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

1. **Make redirect cache invalidation durable across replicas.** Consume the
   already transactional redirect events through an approved inbound delivery
   path on every replica, or reserve a monotonic SEO redirect generation in the
   same database transaction. Seed the consumer from persisted state, invalidate
   the tenant redirect cache before acknowledgement, and perform a full redirect
   cache clear on an unverified first event, offset gap, or listener lag.
   **Depends on:** an inbound transport consumer or persisted generation/offset;
   the current local module-event fan-out alone is insufficient.
   **Done when:** another replica cannot retain a committed redirect change
   beyond the documented reconciliation bound, including Redis/transport
   disconnect and missed-event scenarios.

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
- Targeted backend, outbox/index, Next, Leptos, media fallback, redirect
  multi-replica recovery, and incident runtime checks defined by the
  live-evidence template.

## References

- [SEO documentation](./README.md)
- [SEO replay/repair runbook](./replay-repair-runbook.md)
- [SEO operations runbook](./operations-runbook.md)
- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Runtime parity fixtures](../../../apps/next-frontend/contracts/seo/runtime-parity-fixtures.json)
