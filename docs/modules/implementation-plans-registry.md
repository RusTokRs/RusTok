---
id: doc://docs/modules/implementation-plans-registry.md
kind: module_plan_index
language: en
status: active
---
# Implementation Plans Registry

This index contains one entry for each live local plan. Local plans own the current verified state, next priorities, and targeted verification; completed-work history does not belong here.

## Live plans

| Module/Crate | Local plan | Status | Nearest priority |
| --- | --- | --- | --- |
| `alloy` | [plan](../../crates/alloy/docs/implementation-plan.md) | `in_progress` | Move Alloy execution through the neutral sandbox, then package source revisions as immutable module artifacts. |
| `flex` | [plan](../../crates/flex/docs/implementation-plan.md) | `in_progress` | Execute the source-complete SQLite owner matrix, PostgreSQL transaction/concurrency/replay and two-replica outage/regression recovery evidence, then finish owner transport extraction. |
| `leptos-auth` | [plan](../../crates/leptos-auth/docs/implementation-plan.md) | `not_started` | Remove the legacy `api` compatibility re-export after migrating callers. |
| `leptos-hook-form` | [plan](../../crates/leptos-hook-form/docs/implementation-plan.md) | `not_started` | Validate the shared form-state contract with concrete consumers. |
| `leptos-shadcn-pagination` | [plan](../../crates/leptos-shadcn-pagination/docs/implementation-plan.md) | `not_started` | Remove package-local pagination copy through the host locale contract. |
| `leptos-table` | [plan](../../crates/leptos-table/docs/implementation-plan.md) | `not_started` | Validate the shared table-state contract with its first concrete consumers. |
| `leptos-zod` | [plan](../../crates/leptos-zod/docs/implementation-plan.md) | `not_started` | Validate the shared validation-envelope contract with concrete consumers. |
| `leptos-zustand` | [plan](../../crates/leptos-zustand/docs/implementation-plan.md) | `not_started` | Decide whether a concrete host workflow warrants this shared state contract, then verify Rust/Next wire compatibility. |
| `ai` | [plan](../../crates/rustok-ai/docs/implementation-plan.md) | `in_progress` | Rig, persisted agents/workflows, typed principal/model-assignment mutations, and owner catalog transport are active. The module-owned Leptos editor uses owner-descriptor, tenant-RBAC, principal, and active-provider catalogs only; permissions remain derived server-side. Generic runtime composition transfers module-owned deployment handles and starts registered workers without AI-specific construction in `apps/server`; remaining work is verification and scheduler lifecycle evidence. |
| `ai-alloy` | [plan](../../crates/rustok-ai-alloy/docs/implementation-plan.md) | `in_progress` | Add composed direct-execution evidence, then specify remote Alloy transport only when its owner selects that product path. |
| `ai-athanor` | [plan](../../crates/rustok-ai-athanor/docs/implementation-plan.md) | `in_progress` | Verify the Athanor library pin, then add source-policy evidence and connect the provider to runtime composition; keep vector retrieval fail-closed until Athanor Phase 9. |
| `ai-content` | [plan](../../crates/rustok-ai-content/docs/implementation-plan.md) | `boundary_ready` | Composed moderation and Blog-draft owner-service evidence pass. Adapter controls are composed by `rustok-ai`; no standalone Blog-, Forum-, or content-owned admin route exists. |
| `ai-media` | [plan](../../crates/rustok-ai-media/docs/implementation-plan.md) | `boundary_ready` | Image direct execution persists through the Media owner service; execute the media consumer contract next. |
| `ai-order` | [plan](../../crates/rustok-ai-order/docs/implementation-plan.md) | `boundary_ready` | Advisory direct execution is verified: analytics and sensitive operations output are review-only and never persist order data. Status enrichment uses the host-composed `CheckoutCompletionPort` with deadline and typed degraded fallback. Adapter controls are composed by the `rustok-ai` owner admin and Next admin paths, with no separate order host screen. |
| `ai-product` | [plan](../../crates/rustok-ai-product/docs/implementation-plan.md) | `boundary_ready` | Product-copy direct persistence preserves non-target locales through the product owner; product attributes consume the host-composed `ProductCatalogReadPort`, degrade to prompt-only review-required output on unavailable/deadline errors, and never write product data. Adapter controls are composed by `rustok-ai`; the remaining product-specific work is workflow evidence. |
| `api` | [plan](../../crates/rustok-api/docs/implementation-plan.md) | `not_started` | Keep shared port policy neutral and reject module/runtime ownership drift. |
| `auth` | [plan](../../crates/rustok-auth/docs/implementation-plan.md) | `not_started` | Record browser/runtime parity evidence for the auth admin user and OAuth mutation flows before promoting to parityverified. |
| `blog` | [plan](../../crates/rustok-blog/docs/implementation-plan.md) | `in_progress` | Add public/write rate limits, verify search projection, and obtain live comments plus host-parity evidence. |
| `build` | [plan](../../crates/rustok-build/docs/implementation-plan.md) | `in_progress` | Move the build queue and executor from the server host, then expose the shared execution service to the platform CLI. |
| `cache` | [plan](../../crates/rustok-cache/docs/implementation-plan.md) | `in_progress` | Execute compiled, PostgreSQL channel/Flex, full local CAS, tenant-locale and isolated Redis latency/restart jobs on one reconciled `main` revision; fix failures and record it. |
| `cart` | [plan](../../crates/rustok-cart/docs/implementation-plan.md) | `not_started` | Continue only with owner-module checkout handoff slices that remove real umbrella presentation/read leakage, or return to parity/evidence hardening for SSR native path, GraphQL selected path, headless cart mutation contracts and DOM evidence. |
| `channel` | [plan](../../crates/rustok-channel/docs/implementation-plan.md) | `in_progress` | Execute the complete durable-generation, lag/value, PostgreSQL, Redis restart and cache-owned latency/circuit evidence on one revision, then continue full locale/OAuth/policy runtime evidence. |
| `cli` | [plan](../../crates/rustok-cli/docs/implementation-plan.md) | `in_progress` | Register the first module-local provider, then migrate one owned server workflow to it. |
| `cli-core` | [plan](../../crates/rustok-cli-core/docs/implementation-plan.md) | `not_started` | Connect the first runtime-aware module-local CLI provider. |
| `cli-platform` | [plan](../../crates/rustok-cli-platform/docs/implementation-plan.md) | `not_started` | Add another command only after confirming it is platform-owned rather than module-owned. |
| `cli-registry` | [plan](../../crates/rustok-cli-registry/docs/implementation-plan.md) | `in_progress` | Add the next approved owner-local provider and collect runtime evidence for media cleanup. |
| `comments` | [plan](../../crates/rustok-comments/docs/implementation-plan.md) | `in_progress` | Close runtime contract execution/fallback smoke for CommentsThreadPort and confirm blog embedded/native compatibility snapshots; for FFA, keep the native-only admin exception without new legacy/headless contract while maintaining host-neutral parity/evidence guardrails. |
| `moderation` | [plan](../../crates/rustok-moderation/docs/implementation-plan.md) | `in_progress` | Add repository-backed receipt-first report and case services, then prove active-case deduplication and revision CAS under PostgreSQL contention. |
| `commerce` | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) | `in_progress` | Complete checkout handoff with live evidence, productionize owner provider adapters, then deliver the next owner-bound ecommerce increments. |
| `commerce-foundation` | [plan](../../crates/rustok-commerce-foundation/docs/implementation-plan.md) | `in_progress` | Set consumer acceptance for shared contract changes and prevent domain execution logic from entering the foundation layer. |
| `content` | [plan](../../crates/rustok-content/docs/implementation-plan.md) | `in_progress` | Close reindex drift evidence and expand conversion bridge contract coverage without returning GraphQL resolver/DTO and content analytics SQL to apps/server. |
| `core` | [plan](../../crates/rustok-core/docs/implementation-plan.md) | `not_started` | Execute current foundation contract and dispatcher verification in a build environment. |
| `customer` | [plan](../../crates/rustok-customer/docs/implementation-plan.md) | `not_started` | When compilation is allowed again, run targeted customer service/port tests for normalized identity guards and read-projection runtime smoke, including verification of PortCallPolicy::read() deadline semantics, then decide whether FBA can move above inprogress; until then, keep fast no-compile gates… |
| `email` | [plan](../../crates/rustok-email/docs/implementation-plan.md) | `in_progress` | When compilation is allowed again, run targeted cargo test -p rustok-email ports::tests; current no-compile fallback smoke is locked through npm run verify:foundation:fba-runtime-smoke. |
| `events` | [plan](../../crates/rustok-events/docs/implementation-plan.md) | `in_progress` | Execute the event-runtime lifecycle gate, then define the inbound persisted-offset consumer contract required for remote replay and owner recovery. |
| `fba` | [plan](../../crates/rustok-fba/docs/implementation-plan.md) | `not_started` | Adopt shared typed metadata only after repeated registry shapes are demonstrated, then lock the first wire contract. |
| `forum` | [plan](../../crates/rustok-forum/docs/implementation-plan.md) | `in_progress` | Replace the synthetic Wave packet with observed forum consumer evidence after the `pages` reference gate. |
| `fulfillment` | [plan](../../crates/rustok-fulfillment/docs/implementation-plan.md) | `in_progress` | Continue production carrier adapter wiring separately; keep seller-aware shipping-selection parity locked by the owner storefront guardrail and commerce handoff guardrail. |
| `graphql` | [plan](../../crates/rustok-graphql/docs/implementation-plan.md) | `not_started` | Keep the shared client neutral; add a Dioxus adapter only for an approved concrete host. |
| `iggy` | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) | `in_progress` | Complete real SDK consume/commit, then execute DLQ/replay and resilience evidence against embedded and remote Iggy. |
| `iggy-connector` | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) | `in_progress` | Wire SDK receive/commit, then harden lifecycle failure behavior and publish operating guarantees. |
| `index` | [plan](../../crates/rustok-index/docs/implementation-plan.md) | `in_progress` | Connect persistence-backed adapter over the current in-process seams and collect Rust runtime contract evidence; until then, status remains inprogress. |
| `inventory` | [plan](../../crates/rustok-inventory/docs/implementation-plan.md) | `in_progress` | verification/CI evidence slice for InventoryReservationPort: close contract tests/fallback smoke and then prepare promotion to boundaryready; keep the iteration small and do not run long compilation. |
| `installer` | [plan](../../crates/rustok-installer/docs/implementation-plan.md) | `in_progress` | Versioned topology, capability-aware shared apply sequencing, server build/release deployment adapter, and per-role receipts are implemented; next is standalone CLI adapter parity and CI topology retry evidence. |
| `mcp` | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) | `in_progress` | Obtain authenticated Next/Leptos parity evidence, then design secure remote MCP transport before expanding protocol capabilities. |
| `modules` | [plan](../../crates/rustok-modules/docs/implementation-plan.md) | `in_progress` | Replace compile-time identity with the artifact definition/dispatch/CAS path, then complete facade cutover, isolated build and verifiable OCI publication. |
| `media` | [plan](../../crates/rustok-media/docs/implementation-plan.md) | `in_progress` | remove the legacy media cleanup task after targeted CLI/provider verification, then continue moving remaining module GraphQL artifacts from the server; for Flex, a separate runtime-handle over FieldDefinitionCachePort, FlexStandaloneService and event publishing is needed before removing apps/se… |
| `order` | [plan](../../crates/rustok-order/docs/implementation-plan.md) | `in_progress` | maintain parity of the public GraphQL order contract while post-order surfaces continue moving to owner admin/storefront packages; continue removing remaining module-specific server GraphQL artifacts in small no-compile slices. |
| `outbox` | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) | `in_progress` | Expand relay/backlog/DLQ evidence without long full-workspace compilation and then add targeted runtime contract/fallback smoke when compilation is allowed again. |
| `page-builder` | [plan](../../crates/rustok-page-builder/docs/implementation-plan.md) | `in_progress` | Bind selected adapters and host endpoints, then replace synthetic rollout evidence with observed tenant packets. |
| `pages` | [plan](../../crates/rustok-pages/docs/implementation-plan.md) | `in_progress` | Replace synthetic Wave 0 evidence with an observed tenant run, then promote the reference consumer through a real Wave 1. |
| `payment` | [plan](../../crates/rustok-payment/docs/implementation-plan.md) | `in_progress` | Continue production provider adapter wiring separately; owner storefront guardrail must maintain collection/refund read and create/reuse parity as a single boundary. |
| `pricing` | [plan](../../crates/rustok-pricing/docs/implementation-plan.md) | `in_progress` | Execute the pricing provider, complete the owner transport handoff, then finish the remaining Pricing 2.0 rule semantics. |
| `product` | [plan](../../crates/rustok-product/docs/implementation-plan.md) | `boundary_ready` | Execute the catalog read provider and its declared consumer fallback profiles before promotion to `transport_verified`. |
| `profiles` | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) | `not_started` | Decide whether downstream summaries need a dedicated read model. |
| `rbac` | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) | `in_progress` | Execute the compiled RBAC/server/CLI verification gate, then prove PostgreSQL concurrency and multi-replica durable-generation recovery before adding observability and operator mutation flows. |
| `region` | [plan](../../crates/rustok-region/docs/implementation-plan.md) | `in_progress` | Execute live `RegionReadPort` and storefront transport evidence before any FBA promotion. |
| `runtime` | [plan](../../crates/rustok-runtime/docs/implementation-plan.md) | `not_started` | Use RuntimeComposition in the first DB-backed module CLI provider. |
| `sandbox` | [plan](../../crates/rustok-sandbox/docs/implementation-plan.md) | `in_progress` | Route Alloy drafts through the implemented Rhai/Wasmtime runtime, then add cancellation, admission, durable audit and capability hardening. |
| `search` | [plan](../../crates/rustok-search/docs/implementation-plan.md) | `boundary_ready` | Execute live provider evidence, harden ingestion/analytics operations, then stage external engines as adapters. |
| `seo` | [plan](../../crates/rustok-seo/docs/implementation-plan.md) | `in_progress` | Execute multi-replica redirect cursor recovery evidence, then complete D8 backend/host/media evidence and D9 incident sign-off. |
| `seo-render` | [plan](../../crates/rustok-seo/render/docs/implementation-plan.md) | `in_progress` | Lock cross-host semantic fixtures, exercise storefront SSR, and harden renderer safety regressions. |
| `seo-admin-support` | [plan](../../crates/rustok-seo-admin-support/docs/implementation-plan.md) | `in_progress` | Lock support/control-plane transport ownership, execute owner-layout coverage, and publish reusable-widget acceptance rules. |
| `storage` | [plan](../../crates/rustok-storage/docs/implementation-plan.md) | `not_started` | Restore the required crate README and keep storage ownership docs synchronized. |
| `tax` | [plan](../../crates/rustok-tax/docs/implementation-plan.md) | `not_started` | Replace static contract evidence with runtime contract execution and fallback smoke before any boundaryready promotion. |
| `taxonomy` | [plan](../../crates/rustok-taxonomy/docs/implementation-plan.md) | `not_started` | Keep dictionary scope and owner-module attachment contracts synchronized. |
| `telemetry` | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) | `in_progress` | Prove bootstrap/shutdown modes, harden shared metric safety, and align module instrumentation with operations. |
| `tenant` | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) | `in_progress` | Execute the source-complete exact/wildcard, lag, missed-publication and Redis state-loss/restoration locale scenarios, then collect composed native-overview parity evidence. |
| `test-utils` | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) | `in_progress` | Finish neutral server-test migration, lock mock/fixture contracts, and publish consumer-backed testing recipes. |
| `ui-i18n` | [plan](../../crates/rustok-ui-i18n/docs/implementation-plan.md) | `in_progress` | Lock shared Leptos catalog adoption; add further adapters or parity APIs only for concrete consumers. |
| `web` | [plan](../../crates/rustok-web/docs/implementation-plan.md) | `not_started` | Consolidate repeated controller helpers through the shared Axum boundary. |
| `workflow` | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) | `not_started` | Replace compile-free runtime smoke with live backend evidence: native server-function listworkflows over HostRuntimeContext, GraphQL selected-path execution and typed PortError mapping; do not promote FBA above inprogress until live evidence. |

## Maintenance rules

- Keep one row per local plan and update it when the plan path or current status changes.
- Put durable contract facts in the module documentation and architectural decisions in `DECISIONS/`.
- Do not add checkpoints, percentages, execution logs, or duplicated backlog detail to this index.

## Evidence references

- `rustok-iggy-connector`: `ConnectorAckToken`; `node scripts/verify/verify-iggy-connector-source.mjs`.
