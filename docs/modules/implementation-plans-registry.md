---
id: doc://docs/modules/implementation-plans-registry.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Implementation Plans Registry (crate-level)

This registry is a single operational point for maintaining implementation plans across crates.
Use it as a "single pane of glass": first update the status here, then proceed to the module's local plan.

## Coverage Areas

Each implementation plan in a crate must include two mandatory directions in one document:

- feature delivery (functional stages),
- quality backlog (tests, documentation, DX and quality gates).

A separate second plan for quality **is not needed**: quality is managed in the same `docs/implementation-plan.md` through a separate section/checklist.

## How to Work with the Registry

1. Find the record pointed to by `next_plan_id` in `Cycle state`.
2. Open the linked plan and execute a time-boxed iteration step (recommended 30–60 minutes or 1 PR).
3. Within the iteration, both steps must be done:
   - synchronize the plan with the actual code,
   - execute the next incomplete plan item.
4. Update:
   - the local plan (checkpoint block),
   - this registry (`status`, `progress`, `last_updated_at`, `last_checkpoint`, `next_action`, `blockers`).
5. Move `next_plan_id` to the next record in rotation (even if the current plan is blocked or completed).

## Statuses

- `not_started` — work has not started.
- `in_progress` — active iteration in progress.
- `blocked` — external blocker exists, unblocking required.
- `done` — plan completed, verification passed, docs synchronized.
- `archived` — plan closed/replaced by another document.

## Checkpoint Block Template for Local Plans

Add and maintain the following block at the beginning of each implementation plan:

```md
## Execution checkpoint

- Current phase:
- Last checkpoint:
- Next step:
- Open blockers:
- Hand-off notes for next agent:
- Last updated at (UTC):
```

## Cycle state

| Field | Value | Notes |
|---|---|---|
| `cycle_id` | `2026-Q2-round-robin-v1` | Current cycle identifier |
| `next_plan_id` | `rustok-inventory` | Record ID for the next agent to pick up |
| `last_rotation_at` | `2026-06-26T00:00:00Z` | When the pointer was last moved |
| `rotation_rule` | `strict_round_robin` | Always the next plan in the list, no skipping |

## Global board

| Plan ID | Module / crate | Plan doc | Status | Progress | Owner | Last updated (UTC) | Last checkpoint | Next action | Blockers | Verification gate |
|---|---|---|---|---|---|---|---|---|---|---|
| `alloy` | `alloy` | `crates/alloy/docs/implementation-plan.md` | `in_progress` | `85%` | `agent` | `2026-06-30T00:00:00Z` | Restored executable Alloy compile/test evidence: `cargo xtask module test alloy`, `cargo test -p alloy --lib`, `npm run verify:alloy:runtime-contract`, `npm run verify:ai-alloy:policy` passed after native Rhai limit wiring and `rustok-api/server` dependency fix | Promote remaining static route/schema/pagination/scheduler/hook source locks into executable integration checks and continue MCP/Admin Alloy draft-review surface work | `-` | `cargo xtask module test alloy`; `cargo test -p alloy --lib`; `npm run verify:alloy:runtime-contract` |
| `flex` | `flex` | `crates/flex/docs/implementation-plan.md` | `in_progress` | `84%` | `agent` | `2026-06-20T00:00:00Z` | `no-compile increment: standalone schema-definition guardrails tightened for localized map locale keys, empty optional localized maps, select option values and min/max rule shape` | When compilations are allowed, run `cargo test -p flex --lib`, then `cargo test -p rustok-server --lib` + flex-targeted integration and capture evidence | compile/test evidence deferred by explicit iteration constraint: no compilations | `cargo test -p flex --lib` |
| `leptos-auth` | `leptos-auth` | `crates/leptos-auth/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-auth --lib` |
| `rustok-graphql` | `rustok-graphql` | `crates/rustok-graphql/docs/implementation-plan.md` | `in_progress` | `75%` | `agent` | `2026-07-07T00:00:00Z` | `framework-agnostic GraphQL HTTP client extracted from the old Leptos client boundary; current transport adapters import rustok_graphql directly and Leptos hooks live in rustok-graphql-leptos` | Add Dioxus sibling hooks adapter only when Dioxus enters the workspace and keep verifier coverage strict | `-` | `cargo test -p rustok-graphql --lib`; `cargo test -p rustok-graphql-leptos --lib` |
| `leptos-hook-form` | `leptos-hook-form` | `crates/leptos-hook-form/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-hook-form --lib` |
| `leptos-shadcn-pagination` | `leptos-shadcn-pagination` | `crates/leptos-shadcn-pagination/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-shadcn-pagination --lib` |
| `leptos-table` | `leptos-table` | `crates/leptos-table/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-table --lib` |
| `leptos-zod` | `leptos-zod` | `crates/leptos-zod/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-zod --lib` |
| `leptos-zustand` | `leptos-zustand` | `crates/leptos-zustand/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p leptos-zustand --lib` |
| `rustok-ai` | `rustok-ai` | `crates/rustok-ai/docs/implementation-plan.md` | `in_progress` | `85%` | `agent` | `2026-06-23T00:00:00Z` | `compile-free domain vertical static gate now covers product/content/order descriptors, validators, runtime binding seams, and content sensitive-tool policy merge while runtime composition stays in rustok-ai` | Continue executable targeted tests/evidence when compilation is allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:ai:domain-verticals`; `node scripts/verify/verify-ai-content-contract.mjs`; `cargo test -p rustok-ai --lib` |
| `rustok-ai-content` | `rustok-ai-content` | `crates/rustok-ai-content/docs/implementation-plan.md` | `in_progress` | `63%` | `agent` | `2026-06-22T00:00:00Z` | `blog draft contract tests now cover full payloads, empty patch-style payloads, and blank rejection across every optional generated text field with compile-free static gate` | Add executable targeted verification when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `node scripts/verify/verify-ai-content-contract.mjs`; `cargo test -p rustok-ai-content --lib`; `cargo test -p rustok-ai --lib` |
| `rustok-ai-product` | `rustok-ai-product` | `crates/rustok-ai-product/docs/implementation-plan.md` | `in_progress` | `65%` | `agent` | `2026-06-23T00:00:00Z` | `product support crate ownership is locked by compile-free static gate for product_copy/product_attributes descriptors, validators, and rustok-ai binding seam` | Add executable targeted verification when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:ai:domain-verticals`; `cargo test -p rustok-ai-product --lib` |
| `rustok-ai-order` | `rustok-ai-order` | `crates/rustok-ai-order/docs/implementation-plan.md` | `in_progress` | `60%` | `agent` | `2026-06-23T00:00:00Z` | `order support crate ownership is locked by compile-free static gate for order_analytics/order_ops_assistant descriptors, validators, sensitive metadata, and rustok-ai binding seam` | Add executable targeted verification when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:ai:domain-verticals`; `cargo test -p rustok-ai-order --lib` |
| `rustok-ai-media` | `rustok-ai-media` | `crates/rustok-ai-media/docs/implementation-plan.md` | `in_progress` | `55%` | `agent` | `2026-06-20T00:00:00Z` | `media support crate owns image_asset descriptors, handler adapter API, image-size validation, and runtime fallback source-smoke evidence` | Extend media-owned generated artifact contract and executable targeted verification | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:ai-media:fba`; `cargo test -p rustok-ai-media --lib` |
| `rustok-ai-alloy` | `rustok-ai-alloy` | `crates/rustok-ai-alloy/docs/implementation-plan.md` | `in_progress` | `55%` | `agent` | `2026-06-20T00:00:00Z` | `alloy support crate owns alloy_code descriptors, handler adapter API, runtime payload JSON validation, and script execution policy metadata` | Add executable targeted tests/evidence | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:ai-alloy:policy`; `cargo test -p rustok-ai-alloy --lib` |
| `rustok-api` | `rustok-api` | `crates/rustok-api/docs/implementation-plan.md` | `in_progress` | `38%` | `agent` | `2026-06-22T00:00:00Z` | `shared PortContext/PortError baseline now covers read ports plus email/outbox write-oriented FBA ports with PortCallPolicy::write()` | Add targeted verification for migrated email/outbox write ports when compilations are allowed and continue auditing new local shims | compile/test evidence deferred by explicit iteration constraint: no compilations; no-compile FBA verifiers completed | `node scripts/verify/verify-email-fba.mjs`; `node scripts/verify/verify-outbox-fba.mjs`; `cargo test -p rustok-api --lib`; targeted consumer crate tests |
| `rustok-runtime` | `rustok-runtime` | `crates/rustok-runtime/docs/implementation-plan.md` | `in_progress` | `5%` | `agent` | `2026-07-08T07:40:00Z` | `foundation scaffold created for typed host runtime helpers while rustok-api is kept contract-focused` | Migrate repeated backend adapter shared-handle helpers after two or more backend consumers need the same helper | `-` | `cargo test -p rustok-runtime --lib` |
| `rustok-web` | `rustok-web` | `crates/rustok-web/docs/implementation-plan.md` | `in_progress` | `5%` | `agent` | `2026-07-08T07:40:00Z` | `foundation scaffold created for Axum HTTP error/response boundary helpers used by the Loco controller cutover` | Add PortError mapping and controller guardrails during Phase 2 routing migration | `-` | `cargo test -p rustok-web --lib` |
| `rustok-fba` | `rustok-fba` | `crates/rustok-fba/docs/implementation-plan.md` | `in_progress` | `5%` | `agent` | `2026-07-08T07:40:00Z` | `foundation scaffold created for FBA provider/consumer metadata over rustok-api port primitives` | Migrate repeated FBA registry metadata when module provider/consumer registries converge on the shared typed shape | `-` | `cargo test -p rustok-fba --lib` |
| `rustok-cli-core` | `rustok-cli-core` | `crates/rustok-cli-core/docs/implementation-plan.md` | `in_progress` | `15%` | `agent` | `2026-07-08T07:40:00Z` | `foundation scaffold created for platform CLI command provider contracts outside the production server runtime; provider execution now has a typed default contract` | Extend command argument decoding policy when the first legacy task/seed/migration flow moves out of apps/server | `-` | `cargo test -p rustok-cli-core --lib` |
| `rustok-cli-platform` | `rustok-cli-platform` | `crates/rustok-cli-platform/docs/implementation-plan.md` | `in_progress` | `10%` | `agent` | `2026-07-08T13:10:00Z` | `platform provider crate owns core version command and is selected through root cli-registry.toml` | Keep platform commands narrow and add module-owned providers through module manifests | `-` | `cargo test -p rustok-cli-platform --quiet` |
| `rustok-cli-registry` | `rustok-cli-registry` | `crates/rustok-cli-registry/docs/implementation-plan.md` | `in_progress` | `10%` | `agent` | `2026-07-08T12:40:00Z` | `selected distribution registry crate exists and generated source is checked from module [provides.cli] metadata` | Connect the first module-local or platform ops provider through manifest metadata | `-` | `node scripts/generate/generate-cli-registry.mjs --check`; `cargo test -p rustok-cli-registry --quiet` |
| `rustok-cli` | `rustok-cli` | `crates/rustok-cli/docs/implementation-plan.md` | `in_progress` | `20%` | `agent` | `2026-07-08T12:40:00Z` | `user-facing runner exists with list, list --json, namespace filtering, duplicate command rejection, generated selected registry consumption, typed namespace command dispatch, normalized provider args and core version execution` | Connect the first typed ops provider and reuse normalized args for task/seed input | `-` | `cargo test -p rustok-cli --quiet`; `cargo run -p rustok-cli --quiet -- core version` |
| `rustok-ui-i18n` | `rustok-ui-i18n` | `crates/rustok-ui-i18n/docs/implementation-plan.md` | `in_progress` | `75%` | `agent` | `2026-07-07T00:00:00Z` | `framework-agnostic UI message catalog core owns resolution while Leptos boilerplate moved to rustok-ui-i18n-leptos; rustok-api is outside the UI i18n boundary` | Add Dioxus sibling adapter when Dioxus enters the workspace and keep verifier coverage strict | `-` | `cargo test -p rustok-ui-i18n --lib`; `cargo test -p rustok-ui-i18n-leptos --lib`; `cargo test -p rustok-api --lib` |
| `rustok-auth` | `rustok-auth` | `crates/rustok-auth/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-auth --lib` |
| `rustok-blog` | `rustok-blog` | `crates/rustok-blog/docs/implementation-plan.md` | `in_progress` | `62%` | `agent` | `2026-06-22T00:00:00Z` | `admin FFA slice #102 moved editor field class policy into core-owned BlogPostAdminEditorFieldClassesViewModel with fast boundary guardrail evidence` | Continue small admin render/input fragments or add runtime comments contract execution when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `node scripts/verify/verify-blog-admin-boundary.mjs`; `node --test scripts/verify/verify-blog-admin-boundary.test.mjs`; `cargo test -p rustok-blog --lib` |
| `rustok-cache` | `rustok-cache` | `crates/rustok-cache/docs/implementation-plan.md` | `in_progress` | `45%` | `agent` | `2026-06-20T00:00:00Z` | `Cache factory now exposes backend options, configurable Redis circuit breaker path, instrumented stats, and synced local docs` | Implement generic cache loader/coalescing contract and verify targeted tests when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `cargo test -p rustok-cache --lib` |
| `rustok-cart` | `rustok-cart` | `crates/rustok-cart/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-cart --lib` |
| `rustok-channel` | `rustok-channel` | `crates/rustok-channel/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-channel --lib` |
| `rustok-comments` | `rustok-comments` | `crates/rustok-comments/docs/implementation-plan.md` | `in_progress` | `38%` | `agent` | `2026-06-07T00:00:00Z` | `Comments admin core now owns transport request/command DTO construction; transport facade/native adapter accept core DTOs instead of UI-built primitive argument lists` | Add GraphQL/headless selected-path adapter on top of the same core DTOs or contract-freeze evidence for native-only comments admin wave | GraphQL selected path for comments admin not yet implemented; FBA remains not_started until boundary evidence | `cargo check -p rustok-comments-admin --config profile.dev.debug=0`; `cargo test -p rustok-comments-admin --lib --config profile.dev.debug=0`; `npm run verify:ffa:ui:migration` |
| `rustok-commerce` | `rustok-commerce` | `crates/rustok-commerce/docs/implementation-plan.md` | `in_progress` | `54%` | `agent` | `2026-06-20T00:00:00Z` | `Provider SPI runtime-smoke evidence now includes locked live gateway/carrier execution-plan requirements for payment and fulfillment, while commerce remains the checkout/post-order orchestrator.` | Continue compressing remaining commerce compatibility transport paths toward owner payment/order/fulfillment async transports, then execute the locked provider SPI live adapter plan against concrete external adapters | compile/test evidence deferred by explicit iteration constraint: no compilations; no-compile provider SPI evidence verifier completed | `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; `node scripts/verify/verify-ecommerce-provider-spi-evidence.test.mjs`; `cargo check -p rustok-commerce` |
| `rustok-commerce-foundation` | `rustok-commerce-foundation` | `crates/rustok-commerce-foundation/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-commerce-foundation --lib` |
| `rustok-content` | `rustok-content` | `crates/rustok-content/docs/implementation-plan.md` | `in_progress` | `72%` | `agent` | `2026-06-21T00:00:00Z` | `no-compile orchestration hardening: targeted canonical collision and alias-shadow rollback/no-outbox integration evidence added and source-locked by guardrail npm run verify:content:orchestration` | Close reindex drift evidence and extend conversion bridge contract coverage without expanding shared storage ownership | `compile/test evidence deferred by explicit iteration constraint: no compilations` | `npm run verify:content:orchestration`; `cargo test -p rustok-content --test integration` |
| `rustok-core` | `rustok-core` | `crates/rustok-core/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-core --lib` |
| `rustok-customer` | `rustok-customer` | `crates/rustok-customer/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-customer --lib` |
| `rustok-email` | `rustok-email` | `crates/rustok-email/docs/implementation-plan.md` | `in_progress` | `20%` | `agent` | `2026-06-22T00:00:00Z` | `EmailDeliveryPort consumes shared rustok_api::PortContext/PortError and PortCallPolicy::write(); static FBA verifier updated` | Add targeted compile/runtime contract tests for shared write-policy mapping when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `node scripts/verify/verify-email-fba.mjs`; `cargo test -p rustok-email --lib` |
| `rustok-events` | `rustok-events` | `crates/rustok-events/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-events --lib` |
| `rustok-forum` | `rustok-forum` | `crates/rustok-forum/docs/implementation-plan.md` | `done` | `100%` | `agent` | `2026-06-26T00:00:00Z` | `FW-12 refresh history provenance hardening: live Wave 1 evidence now contains mandatory refresh_history.latest_refresh, focused and aggregate no-compile gates check owner-aligned provenance, full list of no-compile gates and refreshed sections, fixture suite covers gate drift.` | `Steady-state: refresh Wave evidence before next_due_at and keep consumer/freshness fixture no-compile gates green.` | `None` | `npm run verify:page-builder:consumer:forum`; `npm run verify:forum:wave-evidence-freshness`; `npm run test:verify:forum:wave-evidence-freshness`; `cargo test -p rustok-forum --lib` |
| `rustok-fulfillment` | `rustok-fulfillment` | `crates/rustok-fulfillment/docs/implementation-plan.md` | `in_progress` | `22%` | `agent` | `2026-06-17T00:00:00Z` | `no-compile storefront transport increment: fulfillment storefront now owns typed select-shipping-option transport errors and build-profile-selected native/GraphQL selected-path policy; commerce compatibility delegates selected-path decisions to the owner facade.` | Move the remaining select-shipping-option server-function endpoint/body from commerce compatibility into a fulfillment-owned SSR adapter, preserve GraphQL selected-path parity, then replace static provider SPI evidence with runtime contract execution. | `compile/test evidence deferred by explicit iteration constraint: no compilations; выполнен fast source guardrail storefront boundary` | `node scripts/verify/verify-fulfillment-admin-boundary.mjs`; `node scripts/verify/verify-fulfillment-storefront-boundary.mjs`; `cargo test -p rustok-fulfillment --lib` |
| `rustok-iggy` | `rustok-iggy` | `crates/rustok-iggy/docs/implementation-plan.md` | `in_progress` | `28%` | `agent` | `2026-06-20T00:00:00Z` | `no-compile: ack_consumed validates connector metadata cursor before ack; source assertions cover stream/topic/partition mismatch guardrail` | Replace simulated connector ack with real SDK subscriber ack/offset commit path and add actual targeted test evidence | `compile/test evidence deferred by explicit iteration constraint: no compilations` | `cargo test -p rustok-iggy --lib` |
| `rustok-iggy-connector` | `rustok-iggy-connector` | `crates/rustok-iggy-connector/docs/implementation-plan.md` | `in_progress` | `28%` | `agent` | `2026-06-20T14:30:00Z` | `no-compile: ConnectorAckToken now covers simulated and real Iggy SDK ack cursor seam, with remote/embedded subscriber scope validation and source guardrail evidence` | Connect `ConnectorAckToken::iggy_sdk` to actual SDK subscriber receive/commit path; replace source-level evidence with targeted cargo tests when compilations are resolved | `compile/test evidence deferred by explicit iteration constraint: no compilations` | `node scripts/verify/verify-iggy-connector-source.mjs`; `cargo test -p rustok-iggy-connector --lib` |
| `rustok-fulfillment` | `rustok-fulfillment` | `crates/rustok-fulfillment/docs/implementation-plan.md` | `in_progress` | `23%` | `agent` | `2026-06-20T00:00:00Z` | `no-compile provider SPI evidence increment: fulfillment-owned external carrier registration/runtime-mode guardrails now also lock the live adapter execution plan, while storefront shipping-selection ownership remains in fulfillment.` | Move the remaining select-shipping-option server-function endpoint/body from commerce compatibility into a fulfillment-owned SSR adapter, preserve GraphQL selected-path parity, then execute the locked live carrier plan against a concrete external adapter. | `compile/test evidence deferred by explicit iteration constraint: no compilations; выполнен no-compile provider SPI evidence verifier` | `node scripts/verify/verify-fulfillment-admin-boundary.mjs`; `node scripts/verify/verify-fulfillment-storefront-boundary.mjs`; `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; `node scripts/verify/verify-ecommerce-provider-spi-evidence.test.mjs`; `cargo test -p rustok-fulfillment --lib` |
| `rustok-iggy` | `rustok-iggy` | `crates/rustok-iggy/docs/implementation-plan.md` | `in_progress` | `24%` | `agent` | `2026-06-15T00:00:00Z` | `no-compile: consume path switched to connector recv_with_metadata; ConsumedEvent carries offset/opaque ack metadata with fake-connector assertions` | Connect metadata-bearing consume path with DLQ/replay movement and real SDK ack override | `compile/test evidence deferred by explicit iteration constraint: no compilations` | `cargo test -p rustok-iggy --lib` |
| `rustok-iggy-connector` | `rustok-iggy-connector` | `crates/rustok-iggy-connector/docs/implementation-plan.md` | `in_progress` | `18%` | `agent` | `2026-06-15T00:00:00Z` | `no-compile: added SubscriberMessage/SubscriberMessageMetadata plus recv_with_metadata and opaque ack hook without transport policy leakage` | Connect metadata with real SDK subscriber path and transport DLQ/replay movement; replace no-compile evidence with targeted tests when compilations are resolved | `compile/test evidence deferred by explicit iteration constraint: no compilations` | `cargo test -p rustok-iggy-connector --lib` |
| `rustok-index` | `rustok-index` | `crates/rustok-index/docs/implementation-plan.md` | `in_progress` | `18%` | `agent` | `2026-06-26T00:00:00Z` | `no-compile in-process adapter seams locked: read/list adapter filters by selector/tenant/type/locale/limit, rebuild-disabled adapter returns typed unavailable error, verify:index:fba source-locks metadata` | Connect persistence-backed adapter and collect Rust runtime contract evidence when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:index:fba`; `node scripts/verify/verify-index-runtime-fallback-smoke.mjs`; `cargo test -p rustok-index --lib` |
| `rustok-inventory` | `rustok-inventory` | `crates/rustok-inventory/docs/implementation-plan.md` | `in_progress` | `62%` | `agent` | `2026-06-07T08:10:00Z` | `Wave 5 inventory admin boundary current scope complete: native-only AdminInventoryReadService/server-function read path, native set/adjust/reserve/release/check-availability facade, removed commerce GraphQL selected path, public-channel availability/projection helpers exported for commerce compatibility` | Move to verification/CI evidence and support new admin operations only through module-owned facade; non-admin/channel-aware availability tail is tracked in rustok-commerce roadmap | First CI migration-smoke run still needs observation; channel-aware availability integration coverage is tracked as commerce compatibility work | `node scripts/verify/verify-inventory-admin-boundary.mjs`; `./scripts/verify/verify-all.sh inventory-admin-boundary`; `node scripts/verify/verify-inventory-admin-boundary.test.mjs`; `cargo test -p rustok-inventory --lib` |
| `rustok-mcp` | `rustok-mcp` | `crates/rustok-mcp/docs/implementation-plan.md` | `in_progress` | `95%` | `agent` | `2026-07-02T00:00:00Z` | MCP GraphQL query/mutation/types belong to owner crate and together with Leptos native adapters delegate to canonical `McpManagementService` via unified `McpManagementPort`; server contains only DB-provider and schema composition | Add authenticated browser parity smoke for Next `/dashboard/mcp` and Leptos `/mcp` | - | `npm run verify:mcp:admin-boundary`; `cargo check -p rustok-admin --offline`; `cargo check -p rustok-admin --no-default-features --features ssr --offline`; `cargo check -p rustok-mcp-admin --features hydrate --target wasm32-unknown-unknown --offline`; `cargo test -p rustok-mcp-admin --lib --offline`; `cargo check -p rustok-server --offline`; `cargo test -p rustok-mcp --lib --offline`; `npm --prefix apps/next-admin run typecheck` |
| `rustok-media` | `rustok-media` | `crates/rustok-media/docs/implementation-plan.md` | `in_progress` | `60%` | `agent` | `2026-06-26T00:00:00Z` | `no-compile slice added MediaAssetSummary kind/usage helpers, storage-relative proxy_path derivation, and source-locked FBA fallback evidence for internal-binary summary degradation` | Run executable runtime contract/fallback smoke and DB-backed cleanup integration when compilations are allowed again | compile/test evidence deferred by explicit iteration constraint: no compilations | `npm run verify:media:fba`; `npm run verify:media:admin-boundary`; `cargo test -p rustok-media --lib` |
| `rustok-order` | `rustok-order` | `crates/rustok-order/docs/implementation-plan.md` | `in_progress` | `40%` | `agent` | `2026-05-28T00:00:00Z` | `Order returns lifecycle foundation: tenant-scoped get/list, complete/cancel transitions, transition guards and targeted tests` | Add item-level return lines and expand docs/README for post-order guarantees | default server OpenAPI test blocked by existing compile errors outside order; targeted lifecycle tests pass | `cargo test -p rustok-order order_return_lifecycle --test order_service_test` |
| `rustok-outbox` | `rustok-outbox` | `crates/rustok-outbox/docs/implementation-plan.md` | `in_progress` | `38%` | `agent` | `2026-06-22T00:00:00Z` | `OutboxRelayPort consumes shared rustok_api::PortContext/PortError and PortCallPolicy::write(); static FBA verifier updated` | Expand relay/backlog/DLQ evidence and add runtime smoke when compilations are allowed | compile/test evidence deferred by explicit iteration constraint: no compilations | `node scripts/verify/verify-outbox-fba.mjs`; `node scripts/verify/verify-outbox-admin-boundary.mjs`; `cargo test -p rustok-outbox --lib` |
| `rustok-page-builder` | `rustok-page-builder` | `crates/rustok-page-builder/docs/implementation-plan.md` | `in_progress` | `45%` | `agent` | `2026-06-21T00:00:00Z` | `no-compile Phase 1 capability API baseline closed by ReferencePageBuilderService: grapesjs_v1 validation, sanitize error mapping, deterministic preview wrapper/properties/publish typed responses without persistence side effects.` | Connect real persistence/rendering adapter and real GraphQL/server-function endpoints on top of existing dispatch envelope; replace draft Wave evidence with actual tenant packet | compile/test evidence deferred by explicit iteration constraint: no compilations; no-compile FBA baseline verifier completed | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages`; `cargo test -p rustok-page-builder --lib` |
| `rustok-pages` | `rustok-pages` | `crates/rustok-pages/docs/implementation-plan.md` | `in_progress` | `82%` | `agent` | `2026-06-20T00:00:00Z` | `no-compile Page Builder baseline hardening: control-plane dry-run contract establishes atomic toggle change-set, before/after snapshots, waiver policy and read-surface guarantees; correlation evidence contract establishes builder write -> pages publish -> storefront read chain; runtime/pages fallback gates converted to source/evidence assertions without Cargo. Flutter Wave hand-off contract establishes device/runtime evidence-only boundary and added to aggregate no-compile baseline.` | `Execute real PB-FBA-1C control-plane dry-run and replace synthetic packet; collect actual Flutter device/runtime packet; start actual tenant SLO/sign-off collection for Wave 1.` | Wave 1 readiness remains `hold` if there is no complete actual evidence packet, tenant SLO traces/sign-off or no-compile FBA baseline fails | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-control-plane-dry-run.mjs`; `node crates/rustok-page-builder/scripts/verify/verify-page-builder-correlation-evidence.mjs`; `node crates/rustok-page-builder/scripts/verify/verify-page-builder-flutter-handoff.mjs`; `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages` |
| `rustok-outbox` | `rustok-outbox` | `crates/rustok-outbox/docs/implementation-plan.md` | `in_progress` | `35%` | `agent` | `2026-06-20T00:00:00Z` | `added no-compile FFA boundary verifier and fixture suite for read-only admin split; package.json JSON restored for npm verifier scripts` | Expand relay/backlog/DLQ evidence without lengthy full-workspace compilation; runtime FBA smoke deferred until compilations resolved | compile/test evidence deferred by explicit iteration constraint: no compilations | `node scripts/verify/verify-outbox-admin-boundary.mjs`; `node scripts/verify/verify-outbox-admin-boundary.test.mjs`; `npm run verify:outbox:fba`; `cargo test -p rustok-outbox --lib` |
| `rustok-pages` | `rustok-pages` | `crates/rustok-pages/docs/implementation-plan.md` | `in_progress` | `80%` | `agent` | `2026-06-14T12:00:00Z` | `added no-compile RBAC Wave 1 readiness guardrail for pages FBA baseline; local/central docs synchronized with coverage markers for draft/status/channel bypass regression.` | `Execute real PB-FBA-1C control-plane dry-run and replace synthetic packet; start executing tasks from detailed Quality Backlog.` | Wave 1 readiness remains `hold` if there is waiver for anti-drift/fallback, no complete actual evidence packet or RBAC Wave 1 readiness guardrail fails | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-pages-rbac-readiness.mjs`; `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` |
| `rustok-payment` | `rustok-payment` | `crates/rustok-payment/docs/implementation-plan.md` | `in_progress` | `18%` | `agent` | `2026-06-20T00:00:00Z` | `Provider SPI static/runtime-smoke evidence now locks payment-owned external gateway registration/runtime-mode guardrails and the live adapter execution plan without compile steps.` | Move async payment collection transport behind `rustok-payment/storefront`, then execute the locked live gateway plan against a concrete external adapter. | `compile/test evidence deferred by explicit iteration constraint: no compilations; выполнен no-compile provider SPI evidence verifier` | `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; `node scripts/verify/verify-ecommerce-provider-spi-evidence.test.mjs`; `cargo test -p rustok-payment --lib` |
| `rustok-pricing` | `rustok-pricing` | `crates/rustok-pricing/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-pricing --lib` |
| `rustok-product` | `rustok-product` | `crates/rustok-product/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-product --lib` |
| `rustok-profiles` | `rustok-profiles` | `crates/rustok-profiles/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-profiles --lib` |
| `rustok-rbac` | `rustok-rbac` | `crates/rustok-rbac/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-rbac --lib` |
| `rustok-region` | `rustok-region` | `crates/rustok-region/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-region --lib` |
| `rustok-search` | `rustok-search` | `crates/rustok-search/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-search --lib` |
| `rustok-seo` | `rustok-seo` | `crates/rustok-seo/docs/implementation-plan.md` | `in_progress` | `98%` | `agent` | `2026-06-17T00:00:00Z` | `no-compile D8 static evidence hardening: verifier enforces RBAC/module gating, replay/index idempotency and host runtime entrypoint matrices alongside existing fixture/docs/source-symbol guards` | Collect live backend/host CI evidence packet for D8.2/D8.3, attach counters/error parity/non-home metadata smoke and then promote D9 owner sign-off rows from pending to signed | compile/test evidence deferred by explicit iteration constraint: no compilations; static evidence does not replace live backend/host runs | `npm --prefix apps/next-frontend run verify:seo-runtime-fixtures`; `cargo check -p rustok-seo --tests --config profile.dev.debug=0` |
| `rustok-seo-render` | `rustok-seo-render` | `crates/rustok-seo/render/docs/implementation-plan.md` | `in_progress` | `35%` | `agent` | `2026-05-28T23:58:00Z` | `plan synchronized with SEO Phase D: added D7/D8 parity snapshots and Rust-vs-Next contract fixture backlog` | Close D7.1: snapshot matrix for canonical/hreflang/robots/JSON-LD ordering | verification blocked in VM (`cargo` missing), plus waiting for stable D4 REST/GraphQL parity contract | `cargo check -p rustok-seo-render --tests --config profile.dev.debug=0` |
| `rustok-seo-admin-support` | `rustok-seo-admin-support` | `crates/rustok-seo-admin-support/docs/implementation-plan.md` | `in_progress` | `55%` | `agent` | `2026-05-30T12:00:00Z` | `plan synchronized with SEO Phase D after D2/D4 increment: REST/GraphQL transport parity endpoint baseline already available, focus shifted to D6.1 owner-side observability/remediation widgets` | Close D6.1: reusable event delivery status cards + diagnostics remediation hints for owner-module panels | verification blocked in VM (`cargo` missing); D6.1 requires owner-side UI wiring in `pages/product/blog/forum` | `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0` |
| `rustok-storage` | `rustok-storage` | `crates/rustok-storage/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-storage --lib` |
| `rustok-tax` | `rustok-tax` | `crates/rustok-tax/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-tax --lib` |
| `rustok-taxonomy` | `rustok-taxonomy` | `crates/rustok-taxonomy/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-taxonomy --lib` |
| `rustok-telemetry` | `rustok-telemetry` | `crates/rustok-telemetry/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-telemetry --lib` |
| `rustok-tenant` | `rustok-tenant` | `crates/rustok-tenant/docs/implementation-plan.md` | `in_progress` | `90%` | `agent` | `2026-05-21T13:30:00Z` | `closed contract-sync between tenant module docs/manifest and server resolver contract; verification gates updated for actual tenant + resolver coverage` | Start Iteration 2: lifecycle hardening (cache invalidation integration coverage for create/update/deactivate/domain-change) | `-` | `cargo test -p rustok-tenant --lib` |
| `rustok-test-utils` | `rustok-test-utils` | `crates/rustok-test-utils/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-test-utils --lib` |
| `rustok-workflow` | `rustok-workflow` | `crates/rustok-workflow/docs/implementation-plan.md` | `not_started` | `0%` | `unassigned` | `-` | `-` | Sync plan with current code and fill in checkpoint | `-` | `cargo test -p rustok-workflow --lib` |

## Round-robin protocol (for agents)

1. Take `next_plan_id` from `Cycle state`.
2. Execute one meaningful increment on the plan (sync + execution).
3. Update the checkpoint in the local plan.
4. Update the status in this registry.
5. Compute the next record from the `Global board` table and write it to `next_plan_id`.
6. If a blocker arises, move the record to `blocked` and explicitly record the unblocking condition.

## Recovery protocol: second agent without context

If a new agent does not know where the previous one stopped:

1. Read `next_plan_id` from `Cycle state` as the single source of truth.
2. Open the row of this `Plan ID` in `Global board` and take the `Plan doc`.
3. In `Plan doc`, read only `Execution checkpoint` and `Quality backlog` (without fully re-reading the entire file).
4. If the checkpoint is empty/stale, do a mini-sync: update the checkpoint, set `in_progress`, set `next_action` and continue the iteration.
5. Upon completion, be sure to move `next_plan_id` to the next record in rotation.

## Cross-module changes policy (minimal)

1. If a plan item requires changes in another/child module, that is allowed.
2. Do only the minimum needed to close the current item (without unnecessary scope).
3. For a shared feature/fix, it is sufficient to briefly note the affected modules in `Last checkpoint` or `Next action`.
4. Run checks for the source and affected modules.

## Bugfix / Refactor policy on plan updates

During a plan iteration, the agent **can and should** fix found errors and do refactoring,
but only in a controlled scope:

1. If the problem directly blocks the current plan item, fix it in the same iteration.
2. If the change is small and local (within the current module/contract), it may be included in the same increment.
3. If the problem is large or cross-cutting, do not silently expand scope: add a separate item to the backlog,
   record it in `blockers`/`next_action` and proceed through round-robin.
4. Any bugfix/refactor marked as `done` must pass the corresponding verification gate.
5. After fixing, be sure to synchronize the local `implementation-plan.md` and checkpoint.

## Definition of done for plan items

A plan item can be marked `done` only if simultaneously:

1. The change is present in the code.
2. The corresponding verification gate has passed.
3. The local `implementation-plan.md` has been updated to the actual state.

## Registry sync on module count changes

We synchronize the `Global board` composition on full cycle completion (not on a calendar basis):

1. Trigger: `end_of_full_cycle` (returned to the starting `Plan ID`).
2. Compare `Global board` with the list of `crates/*/docs/implementation-plan.md`.
3. Add missing rows for new modules/libraries.
4. Delete orphaned rows for deleted modules/libraries.
5. For rename/relocate, update the existing row (`Plan ID`, `Plan doc`, `Verification gate`) without creating a duplicate.

## Weekly sweep

Once a week, a separate agent/responsible person performs a sweep:

- marks stale elements (`last_updated_at` older than 7 days),
- raises priorities for `blocked` records,
- forms a short "next up" list for the new round.

## Hygiene: how to clean the table if it grows too large

To keep the registry working and not grow useless history:

1. Keep only live records in `Global board` (`not_started`, `in_progress`, `blocked`, `done` from the last 14 days).
2. Delete old completed records from the registry (without a separate archive file).
3. Save only truly important context: in `implementation-plan.md` (critical context section) or in `DECISIONS/` for architectural decisions.
4. If a plan's path/name changed, update the current row, do not create a duplicate.
5. On each weekly sweep, delete empty/duplicate rows and check `Plan ID` uniqueness.


## Page Builder Evidence Packages

- `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json` — synthetic dry-run evidence package Wave 0.
- `crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json` — draft readiness package Wave 1; checked by `verify-page-builder-wave1-readiness-draft.mjs` and does not replace actual tenant sign-off.
