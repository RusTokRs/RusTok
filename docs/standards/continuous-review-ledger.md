---
id: doc://docs/standards/continuous-review-ledger.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: active
---

# Continuous Code Review & Remediation Ledger (ACRE)

Tracking persistent progress across cyclical review rounds for all modules in RusToK.

## Current Cycle Status
- **Active Round:** Round 1
- **Cycle Started:** `2026-09-18T18:10:03Z`
- **Progress:** `24 / 218` components audited (**11%**)
- **Total Workspace Codebase:** `1,854,597` LOC across `218` modules/apps

---

## Components Review Status

| Status | Component | Category | Files | LOC | Last Audited | Notes |
|:---:|---|---|---:|---:|---|---|
| [ ] | [admin](../../apps/admin) | `apps` | 134 | 22,234 | None |  |
| [ ] | [next-admin](../../apps/next-admin) | `apps` | 148 | 13,845 | None |  |
| [ ] | [ai](../../apps/next-admin/packages/ai) | `apps` | 10 | 1,489 | None |  |
| [ ] | [blog](../../apps/next-admin/packages/blog) | `apps` | 17 | 1,777 | None |  |
| [ ] | [commerce](../../apps/next-admin/packages/commerce) | `apps` | 7 | 2,309 | None |  |
| [ ] | [email](../../apps/next-admin/packages/email) | `apps` | 5 | 289 | None |  |
| [ ] | [rbac](../../apps/next-admin/packages/rbac) | `apps` | 5 | 211 | None |  |
| [ ] | [rustok-ai](../../apps/next-admin/packages/rustok-ai) | `apps` | 1 | 4,055 | None |  |
| [ ] | [rustok-mcp](../../apps/next-admin/packages/rustok-mcp) | `apps` | 1 | 1,041 | None |  |
| [ ] | [rustok-product](../../apps/next-admin/packages/rustok-product) | `apps` | 1 | 340 | None |  |
| [ ] | [search](../../apps/next-admin/packages/search) | `apps` | 1 | 2,913 | None |  |
| [ ] | [translation](../../apps/next-admin/packages/translation) | `apps` | 4 | 5,421 | None |  |
| [ ] | [workflow](../../apps/next-admin/packages/workflow) | `apps` | 10 | 1,393 | None |  |
| [ ] | [next-frontend](../../apps/next-frontend) | `apps` | 42 | 3,668 | None |  |
| [ ] | [rustok-blog](../../apps/next-frontend/packages/rustok-blog) | `apps` | 5 | 368 | None |  |
| [ ] | [rustok-comments](../../apps/next-frontend/packages/rustok-comments) | `apps` | 2 | 111 | None |  |
| [ ] | [rustok-product](../../apps/next-frontend/packages/rustok-product) | `apps` | 1 | 70 | None |  |
| [ ] | [search](../../apps/next-frontend/packages/search) | `apps` | 1 | 681 | None |  |
| [ ] | [server](../../apps/server) | `apps` | 477 | 177,052 | None |  |
| [ ] | [storefront](../../apps/storefront) | `apps` | 34 | 5,059 | None |  |
| [x] | [rustok-api](../../crates/libs/rustok-api) | `libs` | 35 | 6,627 | 2026-09-18 19:46 | Audited richtext.rs: eliminated unwrap panic on empty strings, documented schema invariants |
| [x] | [rustok-core](../../crates/libs/rustok-core) | `libs` | 64 | 16,913 | 2026-09-19 05:57 | Eliminated Tier 0 unwraps/panics with documented invariants, converted DatabaseHealthCheck to typed std::error::Error trait, fixed bulkhead doc print |
| [x] | [rustok-events](../../crates/libs/rustok-events) | `libs` | 27 | 10,971 | 2026-09-19 06:10 | Audited schema.rs: documented structural JSON serialization invariants for root events, envelopes, contracts, and digests |
| [x] | [rustok-fba](../../crates/libs/rustok-fba) | `libs` | 1 | 82 | 2026-09-18 18:10 | Verified clean, 1 file, 82 LOC |
| [x] | [rustok-runtime](../../crates/libs/rustok-runtime) | `libs` | 4 | 2,424 | 2026-09-19 06:20 | Audited deployment.rs and layout.rs: replaced expect calls with typed error propagation through Receipt and InvalidMarker variants |
| [x] | [rustok-telemetry](../../crates/libs/rustok-telemetry) | `libs` | 15 | 3,121 | 2026-09-19 06:40 | Centralized 100+ Prometheus metric declarations through typed factory helpers with documented compile-time invariants |
| [x] | [rustok-web](../../crates/libs/rustok-web) | `libs` | 2 | 742 | 2026-09-19 06:50 | Audited lib.rs and browser_assets.rs: eliminated panics and expects in response builders and hex formatting |
| [x] | [alloy](../../crates/modules/alloy) | `modules` | 70 | 24,622 | 2026-09-19 16:15 | Audited memory.rs, sea_orm.rs, runner.rs: eliminated Tier 0 bare expects with typed error propagation and documented concurrency invariants, replaced untyped String cron errors with ScriptError::InvalidTrigger, verified multi-tenancy scoping |
| [x] | [flex](../../crates/modules/flex) | `modules` | 51 | 16,885 | 2026-09-19 16:25 | Audited translation targets and mutation.rs: replaced expects with typed PortErrors in identity builders, documented static contract invariants in descriptor values, enriched GraphQL doc comments |
| [x] | [rustok-ai](../../crates/modules/rustok-ai) | `modules` | 72 | 42,257 | 2026-09-20 07:10 | Audited inference.rs, direct_order_tasks.rs, mcp.rs, policy.rs, metrics.rs, service/types.rs, graphql/types.rs: eliminated Tier 0 bare expects and panics with typed AiError propagation and graceful fallback, removed dead-code suppressions and made lineage public, verified multi-tenancy scoping |
| [x] | [rustok-ai-alloy](../../crates/modules/rustok-ai-alloy) | `modules` | 1 | 384 | 2026-09-20 07:33 | Verified clean, 1 file, 384 LOC, all 7 tests passed |
| [x] | [rustok-ai-athanor](../../crates/modules/rustok-ai-athanor) | `modules` | 2 | 546 | 2026-09-20 07:33 | Verified clean adapter architecture under athanor feature gate, all integration tests passed |
| [x] | [rustok-ai-content](../../crates/modules/rustok-ai-content) | `modules` | 1 | 326 | 2026-09-20 07:33 | Verified clean, 1 file, 326 LOC, all 10 tests passed |
| [x] | [rustok-ai-media](../../crates/modules/rustok-ai-media) | `modules` | 1 | 71 | 2026-09-20 07:33 | Verified clean, 1 file, 71 LOC, all tests passed |
| [x] | [rustok-ai-order](../../crates/modules/rustok-ai-order) | `modules` | 1 | 255 | 2026-09-20 07:33 | Verified clean, 1 file, 255 LOC, all 9 tests passed |
| [x] | [rustok-ai-product](../../crates/modules/rustok-ai-product) | `modules` | 1 | 307 | 2026-09-20 07:33 | Verified clean, 1 file, 307 LOC, all 6 tests passed |
| [x] | [rustok-ai-translation](../../crates/modules/rustok-ai-translation) | `modules` | 1 | 1,657 | 2026-09-20 07:33 | Audited lib.rs: eliminated Tier 0 bare expects on schema serialization and manifest hashing with safe fallbacks, all 18 tests passed |
| [x] | [admin](../../crates/modules/rustok-ai/admin) | `modules` | 17 | 8,440 | 2026-09-20 07:10 | Audited native_server_adapter.rs, transport/mod.rs, ui/leptos.rs: removed dead_code suppression on cancel_run and re-exported in transport facade, safely handled optional browser window/location in WebSocket URL builder |
| [x] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 50 | 11,107 | 2026-09-20 07:44 | Audited config.rs: eliminated Tier 0 unwrap in RS256 key pair validation via pattern matching, all 36 tests passed |
| [x] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 18 | 5,550 | 2026-09-20 07:44 | Audited native_server_adapter.rs: removed allow(dead_code) suppression on change-password REST response status and validated payload, all 16 tests passed |
| [x] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 1 | 333 | 2026-09-20 07:44 | Verified clean CLI module, 1 file, 333 LOC, all 4 tests passed |
| [x] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 126 | 29,040 | 2026-09-20 09:02 | Audited blog family: fixed channel visibility queries, typed error conversions, integration tests, private entities facade, all 146 tests passed |
| [x] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 16 | 4,914 | 2026-09-20 09:02 | Audited leptos UI invariant documentation, all 22 tests passed |
| [x] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 12 | 2,601 | 2026-09-20 09:02 | Eliminated bare expect with localized fallback in leptos UI, all 35 tests passed |
| [ ] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 22 | 3,111 | None |  |
| [ ] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 9 | 1,806 | None |  |
| [ ] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 37 | 14,548 | None |  |
| [ ] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 72 | 16,524 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 18 | 3,643 | None |  |
| [ ] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 47 | 11,238 | None |  |
| [ ] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 14 | 5,226 | None |  |
| [ ] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 38 | 8,889 | None |  |
| [ ] | [rustok-comments-storefront-support](../../crates/modules/rustok-comments-storefront-support) | `modules` | 3 | 171 | None |  |
| [ ] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 8 | 1,379 | None |  |
| [ ] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 295 | 123,544 | None |  |
| [ ] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 20 | 1,044 | None |  |
| [ ] | [admin](../../crates/modules/rustok-commerce/admin) | `modules` | 21 | 5,825 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-commerce/storefront) | `modules` | 16 | 2,023 | None |  |
| [ ] | [rustok-content](../../crates/modules/rustok-content) | `modules` | 48 | 7,003 | None |  |
| [ ] | [rustok-content-orchestration](../../crates/modules/rustok-content-orchestration) | `modules` | 2 | 2,467 | None |  |
| [ ] | [rustok-customer](../../crates/modules/rustok-customer) | `modules` | 26 | 5,022 | None |  |
| [ ] | [admin](../../crates/modules/rustok-customer/admin) | `modules` | 9 | 2,476 | None |  |
| [ ] | [rustok-distribution](../../crates/modules/rustok-distribution) | `modules` | 34 | 15,734 | None |  |
| [ ] | [rustok-email](../../crates/modules/rustok-email) | `modules` | 6 | 702 | None |  |
| [ ] | [rustok-events-module](../../crates/modules/rustok-events-module) | `modules` | 14 | 847 | None |  |
| [ ] | [admin](../../crates/modules/rustok-events-module/admin) | `modules` | 7 | 506 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-events-module/next-admin) | `modules` | 6 | 278 | None |  |
| [ ] | [rustok-forum](../../crates/modules/rustok-forum) | `modules` | 591 | 151,991 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forum/admin) | `modules` | 35 | 12,236 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-forum/storefront) | `modules` | 19 | 4,164 | None |  |
| [ ] | [rustok-fulfillment](../../crates/modules/rustok-fulfillment) | `modules` | 71 | 17,271 | None |  |
| [ ] | [admin](../../crates/modules/rustok-fulfillment/admin) | `modules` | 8 | 1,454 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-fulfillment/storefront) | `modules` | 12 | 1,206 | None |  |
| [ ] | [rustok-groups](../../crates/modules/rustok-groups) | `modules` | 112 | 31,916 | None |  |
| [ ] | [admin](../../crates/modules/rustok-groups/admin) | `modules` | 34 | 8,520 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-groups/storefront) | `modules` | 17 | 3,123 | None |  |
| [ ] | [rustok-iggy](../../crates/modules/rustok-iggy) | `modules` | 32 | 8,279 | None |  |
| [ ] | [rustok-iggy-connector](../../crates/modules/rustok-iggy-connector) | `modules` | 21 | 4,972 | None |  |
| [ ] | [admin](../../crates/modules/rustok-iggy-connector/admin) | `modules` | 7 | 612 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-iggy-connector/next-admin) | `modules` | 5 | 367 | None |  |
| [ ] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 138 | 57,313 | None |  |
| [ ] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 9 | 2,180 | None |  |
| [ ] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 35 | 12,002 | None |  |
| [ ] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 10 | 4,267 | None |  |
| [ ] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 10 | 1,485 | None |  |
| [ ] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 12 | 1,476 | None |  |
| [ ] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 16 | 2,586 | None |  |
| [ ] | [rustok-marketplace-ledger](../../crates/modules/rustok-marketplace-ledger) | `modules` | 28 | 6,426 | None |  |
| [ ] | [rustok-marketplace-listing](../../crates/modules/rustok-marketplace-listing) | `modules` | 31 | 6,385 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-listing/admin) | `modules` | 9 | 1,903 | None |  |
| [ ] | [rustok-marketplace-payout](../../crates/modules/rustok-marketplace-payout) | `modules` | 13 | 1,641 | None |  |
| [ ] | [rustok-marketplace-seller](../../crates/modules/rustok-marketplace-seller) | `modules` | 48 | 10,637 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-seller/admin) | `modules` | 13 | 2,378 | None |  |
| [ ] | [rustok-mcp](../../crates/modules/rustok-mcp) | `modules` | 26 | 6,856 | None |  |
| [ ] | [admin](../../crates/modules/rustok-mcp/admin) | `modules` | 7 | 1,829 | None |  |
| [ ] | [rustok-media](../../crates/modules/rustok-media) | `modules` | 42 | 12,022 | None |  |
| [ ] | [rustok-media-transport](../../crates/modules/rustok-media-transport) | `modules` | 6 | 1,532 | None |  |
| [ ] | [admin](../../crates/modules/rustok-media/admin) | `modules` | 10 | 2,124 | None |  |
| [ ] | [cli](../../crates/modules/rustok-media/cli) | `modules` | 1 | 175 | None |  |
| [ ] | [rustok-moderation](../../crates/modules/rustok-moderation) | `modules` | 36 | 9,113 | None |  |
| [ ] | [rustok-moderation-api](../../crates/modules/rustok-moderation-api) | `modules` | 3 | 766 | None |  |
| [ ] | [rustok-modules](../../crates/modules/rustok-modules) | `modules` | 175 | 129,930 | None |  |
| [ ] | [rustok-modules-translation](../../crates/modules/rustok-modules-translation) | `modules` | 1 | 822 | None |  |
| [ ] | [cli](../../crates/modules/rustok-modules/cli) | `modules` | 1 | 1,813 | None |  |
| [ ] | [rustok-navigation](../../crates/modules/rustok-navigation) | `modules` | 39 | 5,274 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-navigation/storefront) | `modules` | 8 | 494 | None |  |
| [ ] | [rustok-notifications](../../crates/modules/rustok-notifications) | `modules` | 75 | 21,825 | None |  |
| [ ] | [rustok-notifications-api](../../crates/modules/rustok-notifications-api) | `modules` | 4 | 1,119 | None |  |
| [ ] | [admin](../../crates/modules/rustok-notifications/admin) | `modules` | 6 | 127 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-notifications/storefront) | `modules` | 16 | 3,139 | None |  |
| [ ] | [rustok-order](../../crates/modules/rustok-order) | `modules` | 80 | 18,288 | None |  |
| [ ] | [admin](../../crates/modules/rustok-order/admin) | `modules` | 13 | 1,998 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-order/storefront) | `modules` | 11 | 930 | None |  |
| [ ] | [rustok-outbox](../../crates/modules/rustok-outbox) | `modules` | 25 | 4,285 | None |  |
| [ ] | [admin](../../crates/modules/rustok-outbox/admin) | `modules` | 8 | 493 | None |  |
| [ ] | [rustok-page-builder](../../crates/modules/rustok-page-builder) | `modules` | 100 | 27,600 | None |  |
| [ ] | [rustok-page-builder-storefront](../../crates/modules/rustok-page-builder-storefront) | `modules` | 3 | 1,192 | None |  |
| [ ] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 69 | 18,637 | None |  |
| [ ] | [rustok-pages](../../crates/modules/rustok-pages) | `modules` | 170 | 46,835 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pages/admin) | `modules` | 24 | 5,904 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pages/storefront) | `modules` | 17 | 4,717 | None |  |
| [ ] | [rustok-payment](../../crates/modules/rustok-payment) | `modules` | 85 | 20,644 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-payment/storefront) | `modules` | 11 | 1,316 | None |  |
| [ ] | [rustok-pricing](../../crates/modules/rustok-pricing) | `modules` | 47 | 18,461 | None |  |
| [ ] | [rustok-pricing-persistence](../../crates/modules/rustok-pricing-persistence) | `modules` | 5 | 190 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pricing/admin) | `modules` | 13 | 7,537 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pricing/storefront) | `modules` | 10 | 2,465 | None |  |
| [ ] | [rustok-product](../../crates/modules/rustok-product) | `modules` | 168 | 62,712 | None |  |
| [ ] | [rustok-product-bundles](../../crates/modules/rustok-product-bundles) | `modules` | 22 | 3,848 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-bundles/admin) | `modules` | 9 | 2,291 | None |  |
| [ ] | [rustok-product-catalog-service](../../crates/modules/rustok-product-catalog-service) | `modules` | 1 | 508 | None |  |
| [ ] | [rustok-product-relations](../../crates/modules/rustok-product-relations) | `modules` | 20 | 2,100 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-relations/admin) | `modules` | 9 | 1,232 | None |  |
| [ ] | [rustok-product-transport](../../crates/modules/rustok-product-transport) | `modules` | 8 | 1,699 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product/admin) | `modules` | 20 | 12,023 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-product/storefront) | `modules` | 12 | 3,098 | None |  |
| [ ] | [rustok-profiles](../../crates/modules/rustok-profiles) | `modules` | 52 | 8,990 | None |  |
| [ ] | [cli](../../crates/modules/rustok-profiles/cli) | `modules` | 1 | 338 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-profiles/storefront) | `modules` | 9 | 1,464 | None |  |
| [ ] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 55 | 11,022 | None |  |
| [ ] | [admin](../../crates/modules/rustok-rbac/admin) | `modules` | 8 | 503 | None |  |
| [ ] | [cli](../../crates/modules/rustok-rbac/cli) | `modules` | 1 | 243 | None |  |
| [ ] | [rustok-reactions](../../crates/modules/rustok-reactions) | `modules` | 7 | 2,836 | None |  |
| [ ] | [rustok-reactions-api](../../crates/modules/rustok-reactions-api) | `modules` | 3 | 1,200 | None |  |
| [ ] | [rustok-reactions-storefront](../../crates/modules/rustok-reactions-storefront) | `modules` | 6 | 616 | None |  |
| [ ] | [rustok-region](../../crates/modules/rustok-region) | `modules` | 39 | 8,559 | None |  |
| [ ] | [admin](../../crates/modules/rustok-region/admin) | `modules` | 8 | 2,944 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-region/storefront) | `modules` | 9 | 1,739 | None |  |
| [ ] | [rustok-search](../../crates/modules/rustok-search) | `modules` | 95 | 30,084 | None |  |
| [ ] | [admin](../../crates/modules/rustok-search/admin) | `modules` | 20 | 6,725 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-search/storefront) | `modules` | 11 | 3,453 | None |  |
| [ ] | [rustok-seo](../../crates/modules/rustok-seo) | `modules` | 67 | 30,112 | None |  |
| [ ] | [rustok-seo-admin-support](../../crates/modules/rustok-seo-admin-support) | `modules` | 6 | 2,627 | None |  |
| [ ] | [rustok-seo-targets](../../crates/modules/rustok-seo-targets) | `modules` | 1 | 1,252 | None |  |
| [ ] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 15 | 4,462 | None |  |
| [ ] | [render](../../crates/modules/rustok-seo/render) | `modules` | 1 | 699 | None |  |
| [ ] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 36 | 8,796 | None |  |
| [ ] | [rustok-social-graph-cli](../../crates/modules/rustok-social-graph-cli) | `modules` | 1 | 324 | None |  |
| [ ] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 5 | 1,586 | None |  |
| [ ] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 51 | 13,035 | None |  |
| [ ] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 28 | 3,836 | None |  |
| [ ] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 8 | 726 | None |  |
| [ ] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 76 | 45,087 | None |  |
| [ ] | [rustok-translation-targets](../../crates/modules/rustok-translation-targets) | `modules` | 3 | 1,729 | None |  |
| [ ] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 9 | 13,412 | None |  |
| [ ] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 52 | 6,794 | None |  |
| [ ] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 14 | 1,120 | None |  |
| [ ] | [fly](../../crates/ui/fly) | `ui` | 65 | 23,474 | None |  |
| [ ] | [fly-browser](../../crates/ui/fly-browser) | `ui` | 9 | 816 | None |  |
| [ ] | [fly-leptos](../../crates/ui/fly-leptos) | `ui` | 5 | 1,691 | None |  |
| [ ] | [fly-ui](../../crates/ui/fly-ui) | `ui` | 20 | 5,225 | None |  |
| [ ] | [leptos-auth](../../crates/ui/leptos-auth) | `ui` | 9 | 1,494 | None |  |
| [ ] | [leptos-forms](../../crates/ui/leptos-forms) | `ui` | 5 | 363 | None |  |
| [ ] | [leptos-shadcn-pagination](../../crates/ui/leptos-shadcn-pagination) | `ui` | 1 | 98 | None |  |
| [ ] | [leptos-table](../../crates/ui/leptos-table) | `ui` | 1 | 48 | None |  |
| [ ] | [leptos-ui](../../crates/ui/leptos-ui) | `ui` | 7 | 503 | None |  |
| [ ] | [leptos-ui-routing](../../crates/ui/leptos-ui-routing) | `ui` | 1 | 233 | None |  |
| [ ] | [leptos-zod](../../crates/ui/leptos-zod) | `ui` | 1 | 22 | None |  |
| [ ] | [leptos-zustand](../../crates/ui/leptos-zustand) | `ui` | 1 | 18 | None |  |
| [ ] | [rustok-graphql](../../crates/ui/rustok-graphql) | `ui` | 1 | 338 | None |  |
| [ ] | [rustok-graphql-leptos](../../crates/ui/rustok-graphql-leptos) | `ui` | 1 | 206 | None |  |
| [ ] | [rustok-ui-auth](../../crates/ui/rustok-ui-auth) | `ui` | 1 | 54 | None |  |
| [ ] | [rustok-ui-core](../../crates/ui/rustok-ui-core) | `ui` | 5 | 1,280 | None |  |
| [ ] | [rustok-ui-forms](../../crates/ui/rustok-ui-forms) | `ui` | 1 | 75 | None |  |
| [ ] | [rustok-ui-i18n](../../crates/ui/rustok-ui-i18n) | `ui` | 26 | 3,489 | None |  |
| [ ] | [rustok-ui-transport](../../crates/ui/rustok-ui-transport) | `ui` | 1 | 266 | None |  |
| [ ] | [rustok-build](../../crates/utils/rustok-build) | `utils` | 12 | 2,033 | None |  |
| [ ] | [rustok-build-publication](../../crates/utils/rustok-build-publication) | `utils` | 4 | 862 | None |  |
| [ ] | [rustok-build-source](../../crates/utils/rustok-build-source) | `utils` | 2 | 1,570 | None |  |
| [ ] | [rustok-cli](../../crates/utils/rustok-cli) | `utils` | 3 | 1,302 | None |  |
| [ ] | [rustok-cli-core](../../crates/utils/rustok-cli-core) | `utils` | 1 | 118 | None |  |
| [ ] | [rustok-cli-platform](../../crates/utils/rustok-cli-platform) | `utils` | 3 | 466 | None |  |
| [ ] | [rustok-cli-registry](../../crates/utils/rustok-cli-registry) | `utils` | 2 | 131 | None |  |
| [ ] | [rustok-installer](../../crates/utils/rustok-installer) | `utils` | 11 | 4,102 | None |  |
| [ ] | [rustok-installer-cli](../../crates/utils/rustok-installer-cli) | `utils` | 1 | 443 | None |  |
| [ ] | [rustok-installer-persistence](../../crates/utils/rustok-installer-persistence) | `utils` | 7 | 1,380 | None |  |
| [ ] | [rustok-migrations](../../crates/utils/rustok-migrations) | `utils` | 65 | 14,997 | None |  |
| [ ] | [rustok-module-sdk](../../crates/utils/rustok-module-sdk) | `utils` | 2 | 60 | None |  |
| [ ] | [rustok-module-template](../../crates/utils/rustok-module-template) | `utils` | 2 | 329 | None |  |
| [ ] | [rustok-secrets](../../crates/utils/rustok-secrets) | `utils` | 3 | 1,306 | None |  |
| [ ] | [rustok-storage](../../crates/utils/rustok-storage) | `utils` | 4 | 774 | None |  |
| [ ] | [rustok-test-utils](../../crates/utils/rustok-test-utils) | `utils` | 7 | 1,917 | None |  |
| [ ] | [utoipa-swagger-ui-vendored](../../crates/utils/utoipa-swagger-ui-vendored) | `utils` | 2 | 34 | None |  |
| [ ] | [rustok-artifact-node-agent](../../crates/workers/rustok-artifact-node-agent) | `workers` | 8 | 2,426 | None |  |
| [ ] | [rustok-artifact-node-controller](../../crates/workers/rustok-artifact-node-controller) | `workers` | 3 | 137 | None |  |
| [ ] | [rustok-artifact-node-reconciler](../../crates/workers/rustok-artifact-node-reconciler) | `workers` | 3 | 158 | None |  |
| [ ] | [rustok-artifact-node-transport](../../crates/workers/rustok-artifact-node-transport) | `workers` | 6 | 1,016 | None |  |
| [ ] | [rustok-module-build-dispatcher](../../crates/workers/rustok-module-build-dispatcher) | `workers` | 3 | 478 | None |  |
| [ ] | [rustok-module-build-transport](../../crates/workers/rustok-module-build-transport) | `workers` | 6 | 325 | None |  |
| [ ] | [rustok-module-build-worker](../../crates/workers/rustok-module-build-worker) | `workers` | 8 | 4,121 | None |  |
| [ ] | [rustok-registry-validation-worker](../../crates/workers/rustok-registry-validation-worker) | `workers` | 2 | 763 | None |  |
| [ ] | [rustok-sandbox](../../crates/workers/rustok-sandbox) | `workers` | 19 | 7,659 | None |  |
| [ ] | [rustok-sandbox-transport](../../crates/workers/rustok-sandbox-transport) | `workers` | 5 | 1,095 | None |  |
| [ ] | [rustok-sandbox-worker](../../crates/workers/rustok-sandbox-worker) | `workers` | 3 | 692 | None |  |
| [ ] | [rustok-static-distribution-worker](../../crates/workers/rustok-static-distribution-worker) | `workers` | 7 | 2,730 | None |  |
| [ ] | [rustok-verification-transport](../../crates/workers/rustok-verification-transport) | `workers` | 4 | 174 | None |  |
| [ ] | [rustok-verification-worker](../../crates/workers/rustok-verification-worker) | `workers` | 5 | 1,080 | None |  |
| [ ] | [rustok-worker-transport](../../crates/workers/rustok-worker-transport) | `workers` | 1 | 472 | None |  |

---

## Completed Rounds Archive
_No completed rounds yet. Round 1 is currently in progress._
