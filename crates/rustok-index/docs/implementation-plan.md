# Implementation plan for `rustok-index`

## Current state

`rustok-index` owns ingestion, denormalized read models, indexed document
reads, and rebuild orchestration. It is infrastructure for filtering and
link-aware queries; ranking and product-facing search remain owned by
`rustok-search`.

The module owns event listener registration, including Flex ingestion. Its
admin overview uses a Leptos-free core, native-only transport, and explicit UI
adapter. Source-locked adapters currently cover read/list behavior and a typed
rebuild-disabled fallback; they are not persistence-backed runtime proof.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contracts: `IndexReadModelPort` / `index.read_model.v1` and
  `IndexRebuildPort` / `index.rebuild.v1` in
  `crates/rustok-index/contracts/index-fba-registry.json`.
- Static and fallback evidence:
  `crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json`
  and `crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json`.
- `npm run verify:index:fba` and
  `npm run verify:foundation:fba-runtime-smoke` lock port semantics, fallback
  profiles, and the index/search boundary.

## Deployment relationship with Search

Index remains an ingestion and read-model owner, not a search-engine adapter.
The first extraction pilot isolates the Search query/ingestion service while
Index stays in the monolith. Search may request optional enrichment only
through `IndexReadModelPort`; direct reads of Index tables from an isolated
Search database are prohibited. A later Index worker split requires a
replayable event or gRPC stream, inbox deduplication, lag/rebuild metrics,
tenant/locale preservation, and restart/recovery evidence. Search connectors
must never be implemented in this module or exposed through its ports.

## Open results

1. **Replace source-locked adapters with persistence-backed runtime evidence.**
   Execute read/list/rebuild contracts against the actual indexed storage and
   collect Rust runtime proof before promotion beyond `boundary_ready`.
   **Depends on:** a persistence-backed adapter and compiled runtime test setup.
   **Done when:** tenant, type, locale, selector, bounded-limit, deadline, and
   rebuild-disabled semantics pass in the real provider profiles.

2. **Complete ingestion and rebuild lifecycle operations.** Add bootstrap,
   incremental sync, scheduling/retry, and observable consistency/rebuild/sync
   lag behavior without moving indexing into the server host.
   **Depends on:** event consumers, persistence adapter, and operational
   metrics infrastructure.
   **Done when:** lifecycle failures are measurable, recoverable, and governed
   by module-owned rebuild policy.

3. **Publish the canonical cross-module query surface.** Define filtering and
   count contracts with tenant/locale scoping while preserving the strict
   `index != search` ownership boundary.
   **Depends on:** consumer query requirements and indexed schema stability.
   **Done when:** public consumers use documented read-model contracts and do
   not import ranking or search-engine internals from this module.

4. **Consume canonical plain text for richtext sources.** During the atomic
   [Richtext cutover](../../../docs/modules/rich-text-implementation-plan.md),
   replace raw body/format persistence and JSON indexing with the shared
   `rustok-content::richtext` plain-text projection or an owner-published typed
   projection. Keep the document tree and rendering policy outside Index.
   **Done when:** indexed content contains deterministic prose, not serialized
   JSON, and Index has no local richtext parser or renderer.

## Verification

- Contract tests cover every public use case.
- `npm run verify:index:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `cargo xtask module validate index`
- `cargo xtask module test index`
- Targeted ingestion, rebuild, filtering, consistency-drift, and tenant/locale
  scope tests.

## Change rules

1. Keep indexing and rebuild policy in this module; keep ranking in search.
2. Update local documentation, `rustok-module.toml`, and central index/search
   architecture documentation with a public contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
