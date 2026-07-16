# Implementation plan for `rustok-media`

## Current state

`rustok-media` owns media metadata, upload/translation policy, media reads, and
the module-owned admin surface. Physical binaries stay in `rustok-storage`.
GraphQL owns media read/write flows, while multipart upload remains REST-owned.
`MediaQuery::media_usage` and its DTO are module-owned; the server only composes
the schema.

The public URL policy distinguishes direct public URLs, proxy-required storage
paths, and opaque references.

Admin uses a Leptos-free core, native/GraphQL/REST transport adapters, and an
explicit UI adapter. Native functions use `HostRuntimeContext` plus the typed
storage handle. `MediaImageDescriptor` is the cross-module SEO image boundary;
`MediaAssetSummary` supplies kind/usage classification without raw blob access.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `MediaAssetReadPort` / `media.asset_read.v1` in
  `crates/rustok-media/contracts/media-fba-registry.json`.
- Static, fallback, error, and runtime-order evidence:
  `crates/rustok-media/contracts/evidence/media-contract-test-static-matrix.json`,
  `crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json`,
  `crates/rustok-media/contracts/evidence/media-port-error-matrix.json`, and
  `crates/rustok-media/contracts/evidence/media-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-media-admin-boundary.mjs` and
  `npm run verify:media:fba` lock the owner UI boundary and read-provider order.

## Deployment and extraction track

Media is the first whole-module extraction pilot. The current deployment stays
in the modular monolith; the target remote deployment runs the complete
`rustok-media` owner with its own database/schema, storage credentials, and
`MediaAssetReadPort` gRPC adapter. Consumers continue to use the same port and
must not receive raw storage handles or blob payloads through the cross-module
boundary. See [ADR: Media and Search Extraction Boundaries](../../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md).

The pilot is complete only after loopback transport conformance, isolated
database/storage execution, tenant and security propagation, descriptor/public
URL fallback behavior, restart/retry evidence, and health/metrics proof.
`MediaAssetReadPort` covers current cross-module reads. `MediaAssetWritePort`
now owns upload target preparation, delete, translation, and tenant-scoped
cleanup control. Large binaries remain on Media-owned streaming REST or a
future presigned upload rather than generic gRPC DTOs. The embedded upload
handler remains the current body transport; no remote service cutover is
implied by this control contract.

## Open results

1. **Execute MediaAssetReadPort runtime evidence.** Prove tenant context,
   descriptor materialization, typed error retryability, fallback/degraded
   profiles, and consumer behavior with a real provider before FBA promotion.
   **Depends on:** a runtime-composed media/storage provider and SEO consumers.
   **Done when:** executable provider/consumer evidence covers every published
   read profile without raw blob or server-local media access.

2. **Finish cleanup ownership and storage-failure coverage.** Verify the
   `rustok-media-cli` cleanup path against persistence and storage failures, then
   remove the legacy cleanup task.
   **Depends on:** CLI runtime composition and a DB-backed storage fixture.
   **Done when:** cleanup decisions, inspected/deleted/kept/retry reporting, and
   failure recovery are tested through the owner service.

3. **Publish durable media delivery policy.** Evolve richer metadata and
   storage-driver behavior through the module service, and clarify the public
   URL policy for direct, proxy-required, and opaque references.
   **Depends on:** storage driver requirements and consuming public surfaces.
   **Done when:** `MediaAssetSummary`, `MediaImageDescriptor`, URL policy, and
   operations documentation give consumers one stable delivery contract.

## Verification

- `npm run verify:media:admin-boundary`
- `npm run verify:media:fba`
- `cargo xtask module validate media`
- `cargo xtask module test media`
- Targeted upload policy, translation, cleanup, storage-error, and media-port
  tests.

## Change rules

1. Keep media policy and metadata in this module; keep binaries in storage.
2. Update local docs, `rustok-module.toml`, storage docs, and consumer docs with
   a media delivery contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
