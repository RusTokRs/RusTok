# rustok-seo implementation plan

## Current state

`rustok-seo` is the tenant-aware SEO runtime for metadata, redirects,
sitemaps/robots, diagnostics, typed schema blocks, bulk remediation, event and
index delivery tracking, and replay control. Entity authoring remains in owner
modules; SEO admin is infrastructure control-plane only. REST and GraphQL are
additive `v1` surfaces, while Rust and Next storefronts consume the canonical
`SeoPageContext` contract.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- The admin package has core/transport/UI ownership and a neutral
  `HostRuntimeContext` native path; GraphQL and REST remain parallel control
  surfaces. `scripts/verify/verify-seo-admin-boundary.mjs` protects that
  boundary.
- SEO consumes `MediaAssetReadPort` / `media.asset_read.v1` through
  `crates/rustok-seo/contracts/seo-fba-registry.json`. Static evidence is
  `crates/rustok-seo/contracts/evidence/seo-media-consumer-static-matrix.json`
  (`source_locked_pending_consumer_runtime`) and runtime-order evidence is
  `crates/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json`.
  Neither substitutes for live consumer execution.

## Next results

1. **Execute the D8 backend and host matrix.** Capture deployed GraphQL/REST
   parity, outbox/index before-after counters, Next robots/sitemap/metadata,
   Leptos page-context, RBAC/module gating, and replay/idempotency behavior.
   Include the `MediaAssetReadPort` success and degraded cases:
   `omit_image_metadata`, `keep_existing_seo_image`, and relative-URL proxy
   fallback. Done when required live artifacts are attached and high-severity
   parity defects are closed.
2. **Close D9 with operational evidence and owner sign-off.** Run the SEO
   backlog, partial-indexing, and replay/reindex incident procedures against
   the live packet; record redacted evidence, counters, recovery outcomes, and
   signed platform/frontend/operator review. Done when the fixture checklist
   moves from pending static seed to signed without bypassing closeout rules.
3. **Extend storefront SEO only through additive owner routes.** When a new
   Next route owner exists, add its `SeoPageContext` mapping, route matrix
   entry, semantic fallback classification, and Rust/Next fixture evidence.
   Done when no host adds a local target mapping, raw schema handling, or a
   divergent metadata precedence rule.

## Verification

- `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`
- `npm run verify:seo:fba`
- `node scripts/verify/verify-seo-admin-boundary.mjs`
- Targeted backend, outbox/index, Next, Leptos, media fallback, and incident
  runtime checks defined by the live-evidence template.

## References

- [SEO documentation](./README.md)
- [SEO replay/repair runbook](./replay-repair-runbook.md)
- [SEO operations runbook](./operations-runbook.md)
- [Runtime parity fixtures](../../../apps/next-frontend/contracts/seo/runtime-parity-fixtures.json)
