# Implementation plan for `rustok-media`

Status: basic media runtime is already working; local documentation is aligned and
the module is maintained in the scoped audit path.

## Execution checkpoint

- Current phase: media GraphQL field owner and CLI adapter in `rustok-media`
- Last checkpoint: `rustok-module.toml` declares `controllers::axum_router`, which builds `MediaHttpRuntime` from `HostRuntimeContext` plus typed `StorageService` and is merged once by generated host composition. REST handlers use `rustok_web::HttpError`; the domain crate no longer depends on Loco or exposes a route-state adapter. Native admin server functions also consume `rustok_api::HostRuntimeContext`, receive `StorageService` through the neutral typed host-handle snapshot and no longer depend on `loco-rs`. GraphQL field `mediaUsage` and DTO `MediaUsageStats` moved from `apps/server::SystemQuery` to `rustok-media::graphql::MediaQuery`; server is left only as a schema composition point. `rustok-media-cli` now provides `media cleanup`, explicitly composing database and storage from `RuntimeComposition` rather than accessing the server shared store; cleanup policy and persistence stay in `MediaService`.
- Next step: remove the legacy Loco media cleanup task after targeted CLI/provider verification, then continue moving remaining module GraphQL artifacts from the server; for Flex, a separate runtime-handle over `FieldDefinitionCachePort`, `FlexStandaloneService` and event publishing is needed before removing `apps/server/src/graphql/flex`.
- Open blockers: compile/test evidence deferred due to explicit iteration constraint: no compilations.
- Hand-off notes for next agent: keep `MediaImageDescriptor` as the single image payload for cross-module SEO/runtime integrations; admin UI should go through `core` + `transport`, leave Leptos-only code in `ui/leptos.rs`, and transport-specific code in dedicated adapter files.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch owner gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-media/contracts/evidence/media-provider-runtime-order-smoke.json`: shared read policy helper, tenant/list validation order, owner `MediaService` invocation, typed error mapping, descriptor materialization and parity of five degraded modes; status remains `in_progress` until live provider execution;
  - module plan synchronized with central FFA/FBA readiness board; media admin surface is already published and managed in migration/backlog rhythm;
  - native admin transport cut over from Loco `AppContext` to `HostRuntimeContext`; DB is read through the neutral runtime and storage is supplied as a typed host-provided handle, keeping `rustok-media-admin` free of `loco-rs`;
  - FFA admin slice: `admin/src/core.rs` owns Leptos-free form/presentation/state helpers (`non_empty_option`, dimensions label, pagination label, translation form state, usage stat cards, upload success state, busy-key policy, detail-line/list-card view-models and context-error message policy) with unit tests;
  - `admin/src/transport/` owns the current build-profile-selected native/GraphQL transport facade plus REST upload without changing external GraphQL/REST contracts; facade split is locked through `graphql_adapter.rs`, `rest_adapter.rs` and `native_server_adapter.rs`;
  - `admin/src/ui/leptos.rs` is the explicit Leptos render adapter, and the crate root only wires modules and re-exports `MediaAdmin`;
  - runtime hardening slice added service-level cleanup report/decision helpers and targeted unit coverage for upload policy + storage cleanup classification without transport changes;
  - GraphQL ownership boundary: `MediaQuery::media_usage` and `MediaUsageStats` live in `crates/rustok-media/src/graphql`; `apps/server::SystemQuery` no longer imports `rustok_media`; server boundary guard checks this without compilation;
  - FBA provider metadata now exposes the media asset read boundary through `MediaAssetReadPort` / `media.asset_read.v1`: `crates/rustok-media/contracts/media-fba-registry.json`, `crates/rustok-media/contracts/evidence/media-contract-test-static-matrix.json`, source-locked fallback smoke `crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json`, source-locked typed error matrix `crates/rustok-media/contracts/evidence/media-port-error-matrix.json` and `scripts/verify/verify-media-fba.mjs` lock shared `PortCallPolicy::read()` deadline semantics, tenant UUID context validation, typed `PortError` retryability, SEO descriptor fallback/degraded profiles, storage-relative proxy policy and consumer metadata without promoting beyond `in_progress` before executable runtime smoke.

## Scope of work

- keep `rustok-media` as a domain-owned media module on top of `rustok-storage`;
- synchronize upload/translation/storage contracts and local docs;
- evolve admin/runtime surfaces without blurring ownership between module and host wiring.

## Current state

- `MediaService`, entities, DTOs and transport adapters are already implemented;
- `load_media_usage_snapshot` is used through owner-owned field `MediaQuery::media_usage`, and
  `apps/server::SystemQuery` no longer contains media resolver/DTO/imports;
- media metadata is stored in module-owned tables, while binary files remain in `rustok-storage`;
- upload remains REST-owned, GraphQL covers read/write flows without multipart semantics;
- module-owned admin UI and observability surface are already part of the module contract;
- `MediaAssetSummary` introduced for kind/usage classification without raw blob coupling; typed `MediaImageDescriptor` introduced as a cross-module boundary for SEO image payload (`url/alt/size/mime` + derived helpers), complemented with delivery profile policy (`absolute/root-relative public URL`, `storage-relative path`, `opaque reference`) and public URL policy (`direct public`, `proxy required`, `not addressable`), covered by edge-case normalization tests for explicit MIME, invalid dimensions, query/fragment cleanup and proxy-required storage paths.

## Stages

### 1. Contract stability

- [x] lock upload/list/delete/translation runtime contract;
- [x] keep tenant isolation and MIME/size validation inside the module;
- [x] keep media storage metadata and physical storage boundary explicit;
- [~] maintain sync between runtime contracts, admin UI and module metadata; current FFA admin slice moved Leptos-free helpers to `admin/src/core.rs`, including upload/detail/list-card/error state policy, transport facade to `admin/src/transport/`, explicit render adapter to `admin/src/ui/leptos.rs` and fast boundary guardrail `scripts/verify/verify-media-admin-boundary.mjs`; FBA contract sync additionally locked with `media-port-error-matrix.json` and check through `verify-media-fba.mjs`.

### 2. Runtime hardening

- [~] cover cleanup task, storage failures and translation edge-cases with targeted integration tests; translation boundary has unit coverage for locale/text normalization, upload policy and cleanup probe classification are covered by service-level unit tests, DB-backed cleanup integration remains open;
- [~] expose owner-owned maintenance through `rustok-media-cli`; `media cleanup` is registered through module metadata, explicitly initializes storage from the CLI settings snapshot, and reports inspected/deleted/kept/retry counts while the legacy Loco task awaits removal after targeted verification;
- [ ] evolve richer metadata/use-case surfaces only through module-owned service layer; current no-compile slice added `get_asset_summary`/`list_asset_summaries` and DTO-level `MediaAssetSummary`;
- [ ] clarify long-term policy for public URLs and storage-driver-specific guarantees.

### 3. Operability

- [ ] keep Prometheus metrics and storage health semantics production-ready;
- [ ] document cleanup/invalidation/runbook guarantees together with runtime changes;
- [ ] synchronize local docs, README and manifest metadata when module surface changes.

## Verification

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests for upload policy, translation normalization/persistence, cleanup task classification and storage error handling

## Update rules

1. When changing media runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing storage contract or admin UI expectations, update related docs in `rustok-storage` and host docs.


## Quality backlog

- [~] Update test coverage for key module scenarios: FBA static matrix, source-locked fallback smoke, public URL policy / asset summary static evidence and source-locked port error matrix are closed; executable runtime smoke and DB-backed cleanup integration remain open.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
