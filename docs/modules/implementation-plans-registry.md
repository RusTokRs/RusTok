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
| `alloy` | [plan](../../crates/alloy/docs/implementation-plan.md) | ``in_progress`` | Promote remaining static route/schema/pagination/scheduler/hook source locks into executable router/schema/runtime integration checks where host test fixtures permit, then continue MCP/Admin Alloy draft-review surface work. |
| `flex` | [plan](../../crates/flex/docs/implementation-plan.md) | ``in_progress`` | Remove remaining Flex transport artifacts from server beyond Loco/Axum REST handler, SeaORM/bootstrap adapter layer; after compilations are allowed, run targeted Flex tests and record evidence. |
| `leptos-auth` | [plan](../../crates/leptos-auth/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `leptos-hook-form` | [plan](../../crates/leptos-hook-form/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `leptos-shadcn-pagination` | [plan](../../crates/leptos-shadcn-pagination/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `leptos-table` | [plan](../../crates/leptos-table/docs/implementation-plan.md) | ``not_started`` | Keep the public table API aligned with consumers and remove any framework-specific policy that belongs in a host or module package. |
| `leptos-zod` | [plan](../../crates/leptos-zod/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `leptos-zustand` | [plan](../../crates/leptos-zustand/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `ai` | [plan](../../crates/rustok-ai/docs/implementation-plan.md) | ``in_progress`` | Continue extracting remaining AI-specific host artifacts from apps/server, keeping only composition adapters there; keep GraphQL parity and live runtime evidence in the next transport-verification slices. |
| `ai-alloy` | [plan](../../crates/rustok-ai-alloy/docs/implementation-plan.md) | ``in_progress`` | when compilations are allowed, run targeted Rust tests for validateruntimepayload, descriptor policy and allowedoperations; until then, source/static evidence lock remains the primary gate. |
| `ai-content` | [plan](../../crates/rustok-ai-content/docs/implementation-plan.md) | ``in_progress`` | Add executable targeted verification evidence when compilations are allowed. |
| `ai-media` | [plan](../../crates/rustok-ai-media/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `ai-order` | [plan](../../crates/rustok-ai-order/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `ai-product` | [plan](../../crates/rustok-ai-product/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `api` | [plan](../../crates/rustok-api/docs/implementation-plan.md) | ``in_progress`` | Keep new module ports on rustokapi::ports and reject runtime-specific dependencies in the default contract surface. |
| `auth` | [plan](../../crates/rustok-auth/docs/implementation-plan.md) | ``not_started`` | Record browser/runtime parity evidence for the auth admin user and OAuth mutation flows before promoting to parityverified. |
| `blog` | [plan](../../crates/rustok-blog/docs/implementation-plan.md) | ``in_progress`` | Continue small admin render/input fragments without changing the dual-path contract, or add real runtime contract execution against the comments port when compilation/runtime checks are allowed. |
| `cache` | [plan](../../crates/rustok-cache/docs/implementation-plan.md) | ``in_progress`` | Add compile/test evidence when the compilation restriction is lifted and run the ignored real-Redis scenario with RUSTOKCACHEREALREDISURL over the channel-scoped subscription contract. |
| `cart` | [plan](../../crates/rustok-cart/docs/implementation-plan.md) | ``not_started`` | Continue only with owner-module checkout handoff slices that remove real umbrella presentation/read leakage, or return to parity/evidence hardening for SSR native path, GraphQL selected path, headless cart mutation contracts and DOM evidence. |
| `channel` | [plan](../../crates/rustok-channel/docs/implementation-plan.md) | ``not_started`` | Collect full Rust runtime contract evidence for ChannelReadPort and full server middleware test evidence; until Rust runtime evidence FBA remains inprogress, but fallback smoke profiles are now locked by dedicated no-compile executable verifier, resolution-order decision by a fast source verifier, a… |
| `cli` | [plan](../../crates/rustok-cli/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `cli-core` | [plan](../../crates/rustok-cli-core/docs/implementation-plan.md) | ``in_progress`` | connect the first module-local provider that needs database or storage access |
| `cli-platform` | [plan](../../crates/rustok-cli-platform/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `cli-registry` | [plan](../../crates/rustok-cli-registry/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `comments` | [plan](../../crates/rustok-comments/docs/implementation-plan.md) | ``in_progress`` | Close runtime contract execution/fallback smoke for CommentsThreadPort and confirm blog embedded/native compatibility snapshots; for FFA, keep the native-only admin exception without new legacy/headless contract while maintaining Loco-free parity/evidence guardrails. |
| `commerce` | [plan](../../crates/rustok-commerce/docs/implementation-plan.md) | ``in_progress`` | Reduce aggregate cart projection only as a whole owner-handoff package; production provider adapter wiring should be done separately from the storefront boundary. |
| `commerce-foundation` | [plan](../../crates/rustok-commerce-foundation/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `content` | [plan](../../crates/rustok-content/docs/implementation-plan.md) | ``in_progress`` | Close reindex drift evidence and expand conversion bridge contract coverage without returning GraphQL resolver/DTO and content analytics SQL to apps/server. |
| `core` | [plan](../../crates/rustok-core/docs/implementation-plan.md) | ``not_started`` | Run the documented module verification gates when compilation is allowed and continue extending targeted coverage around dispatcher latency metric hooks. |
| `customer` | [plan](../../crates/rustok-customer/docs/implementation-plan.md) | ``not_started`` | When compilation is allowed again, run targeted customer service/port tests for normalized identity guards and read-projection runtime smoke, including verification of PortCallPolicy::read() deadline semantics, then decide whether FBA can move above inprogress; until then, keep fast no-compile gates… |
| `email` | [plan](../../crates/rustok-email/docs/implementation-plan.md) | ``in_progress`` | When compilation is allowed again, run targeted cargo test -p rustok-email ports::tests; current no-compile fallback smoke is locked through npm run verify:foundation:fba-runtime-smoke. |
| `events` | [plan](../../crates/rustok-events/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `fba` | [plan](../../crates/rustok-fba/docs/implementation-plan.md) | ``in_progress`` | migrate repeated provider/consumer registry structs only after two or more module registries need the same typed shape. |
| `forum` | [plan](../../crates/rustok-forum/docs/implementation-plan.md) | ``done`` | Steady-state maintenance: refresh Wave evidence before refreshpolicy.nextdueat, keep no-compile gates and fixture tests green, and integrate only compatible platform features |
| `fulfillment` | [plan](../../crates/rustok-fulfillment/docs/implementation-plan.md) | ``in_progress`` | Continue production carrier adapter wiring separately; keep seller-aware shipping-selection parity locked by the owner storefront guardrail and commerce handoff guardrail. |
| `graphql` | [plan](../../crates/rustok-graphql/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `iggy` | [plan](../../crates/rustok-iggy/docs/implementation-plan.md) | ``in_progress`` | replace simulated connector ack with real SDK subscriber ack/offset commit path and add actual targeted test evidence. |
| `iggy-connector` | [plan](../../crates/rustok-iggy-connector/docs/implementation-plan.md) | ``in_progress`` | connect ConnectorAckToken::iggysdk to the actual SDK subscriber receive/commit path and replace source-level evidence with targeted cargo tests when compilation is allowed. |
| `index` | [plan](../../crates/rustok-index/docs/implementation-plan.md) | ``in_progress`` | Connect persistence-backed adapter over the current in-process seams and collect Rust runtime contract evidence; until then, status remains inprogress. |
| `inventory` | [plan](../../crates/rustok-inventory/docs/implementation-plan.md) | ``in_progress`` | verification/CI evidence slice for InventoryReservationPort: close contract tests/fallback smoke and then prepare promotion to boundaryready; keep the iteration small and do not run long compilation. |
| `mcp` | [plan](../../crates/rustok-mcp/docs/implementation-plan.md) | ``in_progress`` | add authenticated browser-level parity smoke for Next /dashboard/mcp and Leptos /mcp management workflows over the already strengthened draft stage/apply boundary. |
| `media` | [plan](../../crates/rustok-media/docs/implementation-plan.md) | ``in_progress`` | remove the legacy Loco media cleanup task after targeted CLI/provider verification, then continue moving remaining module GraphQL artifacts from the server; for Flex, a separate runtime-handle over FieldDefinitionCachePort, FlexStandaloneService and event publishing is needed before removing apps/se… |
| `order` | [plan](../../crates/rustok-order/docs/implementation-plan.md) | ``in_progress`` | maintain parity of the public GraphQL order contract while post-order surfaces continue moving to owner admin/storefront packages; continue removing remaining module-specific server GraphQL artifacts in small no-compile slices. |
| `outbox` | [plan](../../crates/rustok-outbox/docs/implementation-plan.md) | ``in_progress`` | Expand relay/backlog/DLQ evidence without long full-workspace compilation and then add targeted runtime contract/fallback smoke when compilation is allowed again. |
| `page-builder` | [plan](../../crates/rustok-page-builder/docs/implementation-plan.md) | ``in_progress`` | Phase 3 — integration contract for pages as consumer. |
| `pages` | [plan](../../crates/rustok-pages/docs/implementation-plan.md) | ``in_progress`` | Run a real control-plane Wave 0 dry-run on an internal tenant and replace the synthetic packet with actual before/after snapshots; then replace the Wave 1 readiness draft with a real tenant packet only together with owner sign-off and SLO/smoke evidence.… |
| `payment` | [plan](../../crates/rustok-payment/docs/implementation-plan.md) | ``in_progress`` | Continue production provider adapter wiring separately; owner storefront guardrail must maintain collection/refund read and create/reuse parity as a single boundary. |
| `pricing` | [plan](../../crates/rustok-pricing/docs/implementation-plan.md) | ``not_started`` | Continue small FFA slices only where they reduce Leptos-owned presentation/state policy; do not change the build-profile-selected native/GraphQL transport contract. |
| `product` | [plan](../../crates/rustok-product/docs/implementation-plan.md) | ``not_started`` | Gather live provider execution evidence before promoting product FBA to transportverified. |
| `profiles` | [plan](../../crates/rustok-profiles/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `rbac` | [plan](../../crates/rustok-rbac/docs/implementation-plan.md) | ``not_started`` | Expand operator flows/verification for role and permission management surfaces; add GraphQL/REST secondary path only if such a remote/headless admin contract is approved, and keep the current native-only overview with fast boundary guardrails. |
| `region` | [plan](../../crates/rustok-region/docs/implementation-plan.md) | ``not_started`` | Continue Loco-exit parity/evidence hardening for module-owned native adapters, then gather runtime contract/fallback smoke evidence for shared-context RegionReadPort and storefront native success/native failure + GraphQL success/double-failure error envelope; until runtime evidence, status remains i… |
| `runtime` | [plan](../../crates/rustok-runtime/docs/implementation-plan.md) | ``in_progress`` | pass host-created RuntimeComposition into the first DB-backed module CLI provider. |
| `search` | [plan](../../crates/rustok-search/docs/implementation-plan.md) | ``not_started`` | the next blocker before raising FBA above boundaryready remains live runtime contract execution with real provider invocation. |
| `seo` | [plan](../../crates/rustok-seo/docs/implementation-plan.md) | ``in_progress`` | gather live CI/runtime evidence packet against the deployed backend/hosts, including SEO image descriptor fallback smoke for MediaAssetReadPort by files image-descriptor-in-process.json, provider-unavailable-omit-image-metadata.json, asset-unavailable-keep-existing-seo-image.json, relative-url-proxy… |
| `seo` | [plan](../../crates/rustok-seo/render/docs/implementation-plan.md) | ``in_progress`` | Close D7.2 — expand cross-host fixture matrix Rust renderer vs Next metadata adapter. |
| `seo-admin-support` | [plan](../../crates/rustok-seo-admin-support/docs/implementation-plan.md) | ``in_progress`` | Close D6.2 — transport helpers parity (REST primary + GraphQL secondary path) for diagnostics/sitemap/bulk control-plane read surfaces. |
| `storage` | [plan](../../crates/rustok-storage/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `tax` | [plan](../../crates/rustok-tax/docs/implementation-plan.md) | ``not_started`` | Replace static contract evidence with runtime contract execution and fallback smoke before any boundaryready promotion. |
| `taxonomy` | [plan](../../crates/rustok-taxonomy/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `telemetry` | [plan](../../crates/rustok-telemetry/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `tenant` | [plan](../../crates/rustok-tenant/docs/implementation-plan.md) | ``in_progress`` | Continue Loco-exit parity/evidence hardening for module-owned native adapters and execute authored runtime smoke when compilation is allowed, without mechanical UI expansion. |
| `test-utils` | [plan](../../crates/rustok-test-utils/docs/implementation-plan.md) | ``not_started`` | Synchronize the plan with the current code and select the first incomplete item. |
| `ui-i18n` | [plan](../../crates/rustok-ui-i18n/docs/implementation-plan.md) | ``in_progress`` | Reconcile the module boundary with its current code and contract documentation. |
| `web` | [plan](../../crates/rustok-web/docs/implementation-plan.md) | ``in_progress`` | migrate repeated Loco controller replacement helpers from server/module controllers during Phase 2 of the Loco exit plan. |
| `workflow` | [plan](../../crates/rustok-workflow/docs/implementation-plan.md) | ``not_started`` | Replace compile-free runtime smoke with live backend evidence: native server-function listworkflows over HostRuntimeContext, GraphQL selected-path execution and typed PortError mapping; do not promote FBA above inprogress until live evidence. |

## Maintenance rules

- Keep one row per local plan and update it when the plan path or current status changes.
- Put durable contract facts in the module documentation and architectural decisions in `DECISIONS/`.
- Do not add checkpoints, percentages, execution logs, or duplicated backlog detail to this index.

## Evidence references

- `rustok-iggy-connector`: `ConnectorAckToken`; `node scripts/verify/verify-iggy-connector-source.mjs`.
