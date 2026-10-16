## Scoped Flutter i18n audit — 2026-10-13

- User-requested package audit, independent of the sequential FS cycle below; no FS phase is marked complete.
- Base `main`: `85e83666947217e772ff234664692f61938c4d99`; working branch: `arena/7ca57eaa-rustok`. Not integrated.
- Owner/surface: `rustok_flutter_i18n`, core catalog preparation and locale/fallback primitives, package tests/docs, i18n CI and generated cache hygiene.
- Implemented fixes and compatibility notes: [wrapper audit](../../rustok_mobile/packages/rustok_flutter_i18n/README.md#engineering-audit--2026-10-13).

---

## Deep Full-Stack Audit Cycle — 2026-10-17 (Comprehensive Workspace Expansion)

**Status:** ACTIVE  
**Active phase:** Phase FS-02 — Shared Platform Foundation Libraries (`crates/libs/*`)  
**Next pending component:** [`crates/libs/rustok-core`](../../crates/libs/rustok-core)  
**Current main SHA:** `6239e3614`  
**Active branch:** `main`  

**Purpose:** sequential, top-down root-to-leaf audit of the entire RusToK codebase across all **262 components** (~350k LOC) structured by architectural layer: Host -> Foundation Libraries -> Core Platform Modules -> Commerce -> Content & Social -> Catalog & Extensibility -> Search & AI -> Workers & Execution Fleet -> Platform Utilities -> Frontend Framework -> Client Applications.

### Deep Audit Execution Protocol — mandatory for every component

Every audit iteration targets exactly **one primary production crate/package** and executes the strict closed-loop protocol defined by `AGENTS.md` §2.1 and `CONTINUOUS_CODE_REVIEW.md`:

1. **Discovery pass (CodeGraph & Call Flow):** read the complete in-scope crate end-to-end, including domain model, persistence/migrations, writers/readers, events/outbox, transports, context/tenancy/auth, failure paths, and adjacent boundaries.
2. **Invariant map:** verify business rules, multi-tenancy (`WHERE tenant_id = ?`), transaction atomicity, outbox in-tx publishing (`publish_in_tx`), TOCTOU concurrency guards, and type safety (Newtypes, typed `thiserror` enums).
3. **Zero AI Slop:** eliminate `include!("...")` macros, phantom facades, nested pyramid `match`/`if let` blocks, redundant `.clone()`, and captain-obvious comments.
4. **Atomic Remediation:** fix root causes in the owning crate in the same change set. Never leave a known defect as a follow-up.
5. **FFA Boundary Integrity (for UI packages):** verify framework-agnostic `core/` contains 0 UI framework imports and pure unit tests; verify `ui/leptos` and `ui/dioxus` contain 0 domain or validation logic.
6. **Scoped Verification:** run crate-targeted checks (`cargo check -p <crate>`, `cargo test -p <crate>`); run `python scripts/verify/verify-remediation-gate.py` on modified files.
7. **Commit & Ledger Update:** commit cleanly, update this ledger, and mark the component completed in `.review_ledger.json`.

### Canonical Trigger
The single canonical trigger to continue this audit sequence is:
```text
реализуй план аудита
```

---

### Phase Backlog — Architectural Sequence (262 Components Total)

- [x] **[Phase FS-01 — Entrypoint & Composition Root (`apps/server`)](#fs-01---entrypoint-&-composition-root-appsserver)** (1/1 completed)
- [ ] **[Phase FS-02 — Shared Platform Foundation Libraries (`crates/libs/*`)](#fs-02---shared-platform-foundation-libraries-crateslibs)** (0/10 completed)
- [ ] **[Phase FS-03 — Core Identity, Tenancy & Platform Governance Modules](#fs-03---core-identity,-tenancy-&-platform-governance-modules)** (4/24 completed)
- [ ] **[Phase FS-04 — Commerce, Financial & Marketplace Domain Modules](#fs-04---commerce,-financial-&-marketplace-domain-modules)** (10/37 completed)
- [ ] **[Phase FS-05 — Content, Media, Social & Community Domain Modules](#fs-05---content,-media,-social-&-community-domain-modules)** (17/39 completed)
- [ ] **[Phase FS-06 — Catalog, Taxonomy, Dynamic Extensibility & CMS Pages](#fs-06---catalog,-taxonomy,-dynamic-extensibility-&-cms-pages)** (0/26 completed)
- [ ] **[Phase FS-07 — Search, Indexing, Translation & AI Ecosystem](#fs-07---search,-indexing,-translation-&-ai-ecosystem)** (0/26 completed)
- [ ] **[Phase FS-08 — Background Workers & Execution Fleet (`crates/workers/*`, `rustok-iggy*`)](#fs-08---background-workers-&-execution-fleet-cratesworkers,-rustok-iggy)** (0/19 completed)
- [ ] **[Phase FS-09 — Platform Utilities, Storage & Build Tooling (`crates/utils/*`)](#fs-09---platform-utilities,-storage-&-build-tooling-cratesutils)** (0/19 completed)
- [ ] **[Phase FS-10 — Frontend Framework, UI Primitives & Design System (`crates/ui/*`)](#fs-10---frontend-framework,-ui-primitives-&-design-system-cratesui)** (0/27 completed)
- [ ] **[Phase FS-11 — Client Applications & Host Frontends (`apps/*`)](#fs-11---client-applications-&-host-frontends-apps)** (0/34 completed)

---

### Phase FS-01 — Entrypoint & Composition Root (`apps/server`)
*Host binary, Axum HTTP/gRPC server composition, runtime context, middleware pipeline, global error handling, and background service lifecycle.* (1/1 completed)

- [x] **[`apps/server`](../../apps/server)** (485 files, 190,468 LOC) — Host composition, CLI entrypoint, Axum routing, tenant/auth/channel middleware, GraphQL/REST controllers, event bus, outbox retention worker, watchdog, settings service, and RBAC cache invalidation listener.
  - *Audited:* Audited in FS-01: structured fatal logging before otel shutdown, thread floor minimum; graceful shutdown stopping Axum before draining workers, DB pool min/max connection guards, URL authority validation; secret validation forbidding default/sample credentials and dev JWT in production, SQLite fallback prevention; atomic runtime context taking worker handles without loss; background worker drain with tokio::join! and stop broadcast; global route fallback mapping RecordNotFound to 404 and sanitizing DB errors to 500; fail-closed tenant resolution without RFC 9110 smuggling; cross-tenant channel isolation; BCP 47 locale fallback; duplicate Authorization header rejection; rate-limiting structured JSON responses; security headers API CSP; metrics scrape auth with token quote trimming; registry and artifact HTTP policies; constant-time setup token checks in installer; OpenAPI schema method alignment; event bus reference preservation; outbox retention worker fast-pruning; module watchdog monotonicity; settings service secret redaction; RBAC cache invalidation listener with whitespace trimming.

---

### Phase FS-02 — Shared Platform Foundation Libraries (`crates/libs/*`)
*Core platform abstractions, money/currency primitives, pagination, tenant context, transactional outbox traits, runtime interfaces, and telemetry.* (0/10 completed)

- [ ] **[`crates/libs/rustok-core`](../../crates/libs/rustok-core)** (65 files, 17,627 LOC) — Canonical platform kernel primitives: money/currency math, pagination structures, canonical tenant context, and base error traits.
- [ ] **[`crates/libs/rustok-api`](../../crates/libs/rustok-api)** (35 files, 7,292 LOC) — Public HTTP/gRPC transport contracts, standard REST DTOs, API error body models, and pagination query schemas.
- [ ] **[`crates/libs/rustok-events`](../../crates/libs/rustok-events)** (34 files, 12,081 LOC) — Transactional event bus, outbox event schema contracts, event versioning, and dead-letter queue structures.
- [ ] **[`crates/libs/rustok-runtime`](../../crates/libs/rustok-runtime)** (4 files, 2,423 LOC) — Runtime lifecycle primitives, instance-bound handles, execution environment detection, and shutdown signals.
- [ ] **[`crates/libs/rustok-manifest`](../../crates/libs/rustok-manifest)** (6 files, 832 LOC) — Module manifest (`rustok-module.toml`) schema, dependency validation, capability declarations, and permissions metadata.
- [ ] **[`crates/libs/rustok-telemetry`](../../crates/libs/rustok-telemetry)** (15 files, 3,340 LOC) — Distributed tracing, OpenTelemetry integration, structured JSON logging, metrics collection, and request correlation IDs.
- [ ] **[`crates/libs/rustok-fba`](../../crates/libs/rustok-fba)** (1 files, 82 LOC) — Fluid Backend Architecture (FBA) interfaces, capability profiles, provider descriptors, and consumer dependency injection.
- [ ] **[`crates/libs/rustok-web`](../../crates/libs/rustok-web)** (2 files, 742 LOC) — Shared web utilities, HTTP header parsing, CORS policies, and cookie handling primitives.
- [ ] **[`crates/libs/rustok-content-portability`](../../crates/libs/rustok-content-portability)** (6 files, 617 LOC) — Content export/import formats, archive packing/unpacking, and migration portability interfaces.
- [ ] **[`crates/libs/rustok-content-portability-api`](../../crates/libs/rustok-content-portability-api)** (6 files, 920 LOC) — Public API contracts, DTOs, and traits for content portability.

---

### Phase FS-03 — Core Identity, Tenancy & Platform Governance Modules
*Multi-tenant routing, JWT/session authentication, role-based access control, distributed cache invalidation, channels, outbox dispatch, and workflow automation.* (4/24 completed)

- [x] **[`crates/modules/rustok-tenant`](../../crates/modules/rustok-tenant)** (28 files, 4,305 LOC) — Tenant lifecycle, provisioning, slug/domain resolution, locale policy aggregate, input sanitization, and settings schema.
  - *Audited:* Audited in FS-02.01-03: added Validation variant to TenantError mapped to PortError::validation; trimmed and validated slug/name in create/ensure/update; fast-pathed empty slug/domain lookups to NotFound; clamped pagination; guarded nil UUIDs in TenantReadSelector::Id and parse_context_tenant_id; trimmed ActiveModel::new; bounded settings array elements to 256; rejected PostgreSQL null bytes in keys/values; float finiteness checks; all 24 tests passed.
- [ ] **[`crates/modules/rustok-tenant/admin`](../../crates/modules/rustok-tenant/admin)** (8 files, 835 LOC) — FFA Leptos/Dioxus admin UI components for tenant provisioning, domain management, and locale settings.
- [x] **[`crates/modules/rustok-auth`](../../crates/modules/rustok-auth)** (50 files, 12,952 LOC) — Argon2/SHA256 credentials hashing, JWT encode/decode, token rotation, password reset, bootstrap user provisioning, and auth command models.
  - *Audited:* Audited in FS-02.04-06: enforced nil UUID validation in access token, OAuth token, and access token decoders; trimmed whitespace in token strings; empty/nil email/tenant guards in password reset, verification, and invite token codecs; safe verify_password oracle leak prevention returning Ok(false); HS256 secret length validation; bootstrap user non-nil checks; custom Debug implementations redacting passwords, tokens, auth codes, and client secrets across all auth commands and DTOs; all 62 unit tests pass.
- [ ] **[`crates/modules/rustok-auth/admin`](../../crates/modules/rustok-auth/admin)** (18 files, 6,501 LOC) — FFA admin UI components for user management, password reset requests, session revocation, and security policies.
- [ ] **[`crates/modules/rustok-auth/cli`](../../crates/modules/rustok-auth/cli)** (1 files, 333 LOC) — CLI maintenance commands for auth administration, token generation, and superadmin provisioning.
- [x] **[`crates/modules/rustok-rbac`](../../crates/modules/rustok-rbac)** (56 files, 13,518 LOC) — RBAC domain service, role assignment, policy evaluation, permissions matrix, and runtime cache invalidation.
  - *Audited:* Audited in FS-02.07-08: enriched RbacError taxonomy with domain errors and policy conversion; fast-pathed nil tenant_id and user_id to empty permissions in resolve_permissions_*; guarded load_role_user_ids_on against nil tenant_id; non-nil UUID validation on parse_tenant_id and parse_user_actor; bounded maximum check permissions to 100; guarded RbacRoleAssignmentDbWriter against nil IDs; all 82 tests pass.
- [ ] **[`crates/modules/rustok-rbac/admin`](../../crates/modules/rustok-rbac/admin)** (8 files, 952 LOC) — FFA admin UI components for role management, permission matrices, and user role assignment.
- [ ] **[`crates/modules/rustok-rbac/cli`](../../crates/modules/rustok-rbac/cli)** (1 files, 243 LOC) — CLI maintenance commands for RBAC policy inspection and role assignment.
- [x] **[`crates/modules/rustok-cache`](../../crates/modules/rustok-cache)** (46 files, 15,917 LOC) — Distributed caching, Redis/in-memory backends, stampede protection, and durable invalidation consumer.
  - *Audited:* Audited in FS-02.09-10: unified CacheError domain taxonomy in error.rs with Display, Error, and From conversions; Display and Error on CacheInvalidationMessageError; structured debug tracing on load_or_fill, publish_invalidation, and backend client; structured entry tracing in durable invalidation service, policy, consumer, and versioned invalidation; all 180 unit tests pass.
- [ ] **[`crates/modules/rustok-cache/admin`](../../crates/modules/rustok-cache/admin)** (7 files, 926 LOC) — FFA admin UI components for cache metrics inspection, manual invalidation, and backend health monitoring.
- [ ] **[`crates/modules/rustok-channel`](../../crates/modules/rustok-channel)** (47 files, 11,368 LOC) — Sales channel resolution, domain/subdomain mapping, channel scoping, and channel-specific configuration.
- [ ] **[`crates/modules/rustok-channel/admin`](../../crates/modules/rustok-channel/admin)** (14 files, 5,173 LOC) — FFA admin UI components for sales channel management, domain routing, and currency/locale channel defaults.
- [ ] **[`crates/modules/rustok-outbox`](../../crates/modules/rustok-outbox)** (26 files, 4,758 LOC) — Outbox dispatch engine, event persistence, publisher loop, retry backoff, and DLQ tracking.
- [ ] **[`crates/modules/rustok-outbox/admin`](../../crates/modules/rustok-outbox/admin)** (8 files, 493 LOC) — FFA admin UI components for outbox inspection, dead-letter queue replay, and dispatch metrics.
- [ ] **[`crates/modules/rustok-modules`](../../crates/modules/rustok-modules)** (242 files, 131,685 LOC) — Module registry control-plane, lifecycle state machine, dynamic registration, and migration executor.
- [ ] **[`crates/modules/rustok-modules/cli`](../../crates/modules/rustok-modules/cli)** (1 files, 1,813 LOC) — CLI tool for module scaffolding, manifest verification, and lifecycle management.
- [ ] **[`crates/modules/rustok-modules-translation`](../../crates/modules/rustok-modules-translation)** (1 files, 822 LOC) — Module descriptor translation target bindings and localized manifest metadata.
- [ ] **[`crates/modules/rustok-events-module`](../../crates/modules/rustok-events-module)** (9 files, 571 LOC) — Host runtime adapter for event subscriptions and module event bridge.
- [ ] **[`crates/modules/rustok-events-module/admin`](../../crates/modules/rustok-events-module/admin)** (8 files, 508 LOC) — FFA admin UI for system event monitoring and subscription inspection.
- [ ] **[`crates/modules/rustok-events-module/next-admin`](../../crates/modules/rustok-events-module/next-admin)** (6 files, 278 LOC) — Next.js admin package for event stream visualization and DLQ management.
- [ ] **[`crates/modules/rustok-workflow`](../../crates/modules/rustok-workflow)** (52 files, 7,328 LOC) — Workflow automation engine, state machine transitions, event triggers, and action execution.
- [ ] **[`crates/modules/rustok-workflow/admin`](../../crates/modules/rustok-workflow/admin)** (14 files, 1,434 LOC) — FFA admin UI for workflow rule authoring, trigger configuration, and execution logs.
- [ ] **[`crates/modules/rustok-email`](../../crates/modules/rustok-email)** (6 files, 784 LOC) — Transactional email sending, SMTP/API provider integration, template rendering, and delivery tracking.
- [ ] **[`crates/modules/rustok-accounting`](../../crates/modules/rustok-accounting)** (13 files, 417 LOC) — General ledger, double-entry journal entries, chart of accounts, and financial audit records.

---

### Phase FS-04 — Commerce, Financial & Marketplace Domain Modules
*Cart, checkout pipeline, orders, payments, pricing, inventory reservations, fulfillment, marketplace multi-vendor economics, seller balances, and commissions.* (10/37 completed)

- [x] **[`crates/modules/rustok-commerce-foundation`](../../crates/modules/rustok-commerce-foundation)** (19 files, 1,106 LOC) — Shared commerce error taxonomy, common DTOs, search contracts, and pricing/inventory input validation.
  - *Audited:* Audited in FS-03.01: implemented From<CommerceError> for CoreError with mapping to NotFound, Validation, and Forbidden; validate_invariants on PriceInput and AdjustInventoryInput guarding against negative prices and nil UUIDs; trimmed search patterns in product_translation_title_search_condition avoiding redundant full EXISTS subqueries; all 8 tests pass.
- [x] **[`crates/modules/rustok-commerce`](../../crates/modules/rustok-commerce)** (297 files, 132,631 LOC) — Checkout operation orchestration, stage pipeline, inventory reservation journal, multi-stage compensation sweeps, marketplace paid/reversal inboxes, fulfillment recovery, and REST/GraphQL adapters.
  - *Audited:* Audited in FS-03.02-08: enforced non-nil UUID validation across complete_checkout, begin, checkpoint, ensure_cart_tenant, ensure_order_tenant, ensure_payment_collection_tenant; stage execution in standard and owner-ports pipelines; inventory reservation journal and payment stages; multi-stage compensation and failure sweeps; marketplace paid and reversal event inboxes, financial reconciliation operators, provider event adapters; fulfillment create-label recovery, returns lifecycle; admin and storefront REST controllers and GraphQL helper scopes; all 214 tests pass.
- [ ] **[`crates/modules/rustok-commerce/admin`](../../crates/modules/rustok-commerce/admin)** (21 files, 5,889 LOC) — FFA admin UI for checkout operations, manual order adjustment, and fulfillment management.
- [ ] **[`crates/modules/rustok-commerce/storefront`](../../crates/modules/rustok-commerce/storefront)** (16 files, 2,022 LOC) — Storefront UI components for checkout flow, address input, and order confirmation.
- [x] **[`crates/modules/rustok-cart`](../../crates/modules/rustok-cart)** (73 files, 19,638 LOC) — Cart state machine, line items mutation, price calculations, promotion guards, and checkout snapshot creation.
  - *Audited:* Audited in FS-03.09: enforced non-nil UUID validation for tenant_id, cart_id, line_item_id, channel_id, customer_id, region_id, shipping_option_id across ports, checkout_snapshot, marketplace_snapshot, atomic_checkout_guard, atomic_checkout_port, owner_ports, promotion_guard, guarded_ports; structured debug tracing; all 32 tests pass.
- [ ] **[`crates/modules/rustok-cart/storefront`](../../crates/modules/rustok-cart/storefront)** (18 files, 4,545 LOC) — Storefront UI components for mini-cart, cart drawer, line item management, and promo code entry.
- [x] **[`crates/modules/rustok-order`](../../crates/modules/rustok-order)** (85 files, 22,532 LOC) — Order lifecycle state machine, tax lines, checkout identity, recovery, compensation, and outbox event publishing.
  - *Audited:* Audited in FS-03.10: enforced non-nil UUID validation across ports, order service state machine transitions, line items, tax lines, checkout identity, recovery, compensation, settlement, admin commands, post-order commands, and reads; structured debug tracing across boundaries; all 14 unit tests pass.
- [ ] **[`crates/modules/rustok-order/admin`](../../crates/modules/rustok-order/admin)** (14 files, 2,813 LOC) — FFA admin UI for order detail views, status transitions, fulfillment dispatch, and refund actions.
- [ ] **[`crates/modules/rustok-order/storefront`](../../crates/modules/rustok-order/storefront)** (12 files, 2,171 LOC) — Storefront UI components for customer order history, order status tracking, and receipt download.
- [x] **[`crates/modules/rustok-payment`](../../crates/modules/rustok-payment)** (88 files, 24,276 LOC) — Payment provider integration, transaction lifecycle, idempotency receipts, webhooks, and payment captures/refunds.
  - *Audited:* Audited in FS-03.11: enforced non-nil UUID validation across ports, admin commands/reads, checkout execution/compensation, provider operation/event journals, controllers, and services; bounded pagination parameters; unit tests for non-nil UUID guards and port context validation; all 47 unit tests pass.
- [ ] **[`crates/modules/rustok-payment/storefront`](../../crates/modules/rustok-payment/storefront)** (11 files, 1,315 LOC) — Storefront UI components for payment method selection, card entry, and payment redirects.
- [x] **[`crates/modules/rustok-pricing`](../../crates/modules/rustok-pricing)** (48 files, 19,495 LOC) — Price resolution rules, currency rounding discipline (money::round_to_currency), discounts, and price list evaluation.
  - *Audited:* Audited in FS-03.12: enforced non-nil UUID validation across PricingReadPort, PricingWritePort, PriceListOwnerPort, and PricingService; verified currency rounding discipline via money::round_to_currency; unit tests for port context parsing, non-nil UUID guards, and pricing math; all 17 tests pass.
- [ ] **[`crates/modules/rustok-pricing/admin`](../../crates/modules/rustok-pricing/admin)** (14 files, 7,971 LOC) — FFA admin UI for price lists, currency rules, customer group pricing, and discount rules.
- [ ] **[`crates/modules/rustok-pricing/storefront`](../../crates/modules/rustok-pricing/storefront)** (10 files, 2,487 LOC) — Storefront UI price badges, formatted currency display, and discount indicators.
- [x] **[`crates/modules/rustok-pricing-persistence`](../../crates/modules/rustok-pricing-persistence)** (5 files, 328 LOC) — SeaORM entities, migrations, and price list repository implementations.
  - *Audited:* Audited in FS-03.12: verified price list entities, tenant-scoped schema constraints, and persistence layer invariants; all 5 tests pass.
- [x] **[`crates/modules/rustok-inventory`](../../crates/modules/rustok-inventory)** (35 files, 13,033 LOC) — Stock availability checks, reservation leases, concurrency guards, backorder policies, and stock location translations.
  - *Audited:* Audited in FS-03.13: enforced non-nil UUID validation across InventoryReservationPort, InventoryReservationIdentityPort, reservation contexts, InventoryService, public channel inventory projection, and stock location translations; bounded string inputs; structured debug tracing; all 37 tests pass.
- [ ] **[`crates/modules/rustok-inventory/admin`](../../crates/modules/rustok-inventory/admin)** (10 files, 4,750 LOC) — FFA admin UI for stock adjustments, inventory location tracking, and low-stock alerts.
- [x] **[`crates/modules/rustok-fulfillment`](../../crates/modules/rustok-fulfillment)** (76 files, 26,555 LOC) — Fulfillment orders, shipping carrier integration, package manifests, tracking updates, and split fulfillment.
  - *Audited:* Audited in FS-03.14: enforced non-nil UUID validation across FulfillmentReadPort, FulfillmentAdminCommandPort, FulfillmentAdminCreateCommandPort, ShippingOptionReadPort, ShippingOptionAdminCommandPort, and public ports; bounded string inputs and clamped pagination; all 163 tests pass.
- [ ] **[`crates/modules/rustok-fulfillment/admin`](../../crates/modules/rustok-fulfillment/admin)** (8 files, 1,916 LOC) — FFA admin UI for shipment creation, packing slip generation, and carrier tracking entry.
- [ ] **[`crates/modules/rustok-fulfillment/storefront`](../../crates/modules/rustok-fulfillment/storefront)** (12 files, 1,252 LOC) — Storefront UI components for shipping option selection and tracking links.
- [x] **[`crates/modules/rustok-distribution`](../../crates/modules/rustok-distribution)** (34 files, 15,716 LOC) — Distribution routing, fulfillment center assignment, and carrier API transport adapters.
  - *Audited:* Audited in FS-03.14: verified distribution routes, split fulfillment manifests, carrier integration ports, and non-nil UUID validation; all 10 tests pass.
- [ ] **[`crates/modules/rustok-customer`](../../crates/modules/rustok-customer)** (26 files, 5,273 LOC) — Customer profile management, addresses, customer groups, and lifetime value analytics.
- [ ] **[`crates/modules/rustok-customer/admin`](../../crates/modules/rustok-customer/admin)** (9 files, 2,779 LOC) — FFA admin UI for customer directory, address management, and order history view.
- [ ] **[`crates/modules/rustok-region`](../../crates/modules/rustok-region)** (39 files, 8,566 LOC) — Geographic regions, countries, provinces, postal code rules, and regional currency/tax bindings.
- [ ] **[`crates/modules/rustok-region/admin`](../../crates/modules/rustok-region/admin)** (8 files, 2,944 LOC) — FFA admin UI for region configuration, country enabling, and regional shipping zones.
- [ ] **[`crates/modules/rustok-region/storefront`](../../crates/modules/rustok-region/storefront)** (9 files, 1,738 LOC) — Storefront UI country/region selectors and address format adapters.
- [ ] **[`crates/modules/rustok-tax`](../../crates/modules/rustok-tax)** (5 files, 1,574 LOC) — Tax rate definitions, tax categories, nexus calculation rules, and provider calculation adapters.
- [ ] **[`crates/modules/rustok-marketplace`](../../crates/modules/rustok-marketplace)** (10 files, 1,485 LOC) — Marketplace platform configuration, vendor onboarding rules, and marketplace-wide policies.
- [ ] **[`crates/modules/rustok-marketplace-seller`](../../crates/modules/rustok-marketplace-seller)** (48 files, 10,566 LOC) — Seller directory, vendor profiles, verification status, and store policies.
- [ ] **[`crates/modules/rustok-marketplace-seller/admin`](../../crates/modules/rustok-marketplace-seller/admin)** (13 files, 2,387 LOC) — FFA admin UI for vendor approval, seller profile editing, and store policy configuration.
- [ ] **[`crates/modules/rustok-marketplace-listing`](../../crates/modules/rustok-marketplace-listing)** (31 files, 6,455 LOC) — Vendor product listings, listing approvals, catalog mapping, and seller stock delegation.
- [ ] **[`crates/modules/rustok-marketplace-listing/admin`](../../crates/modules/rustok-marketplace-listing/admin)** (9 files, 1,911 LOC) — FFA admin UI for seller listing moderation, review queue, and listing status overrides.
- [ ] **[`crates/modules/rustok-marketplace-allocation`](../../crates/modules/rustok-marketplace-allocation)** (12 files, 1,486 LOC) — Order item allocation to sellers, multi-vendor cart splitting, and vendor order routing.
- [ ] **[`crates/modules/rustok-marketplace-balance`](../../crates/modules/rustok-marketplace-balance)** (29 files, 6,779 LOC) — Vendor balance accounting, pending vs cleared funds, escrow holds, and balance ledger.
- [ ] **[`crates/modules/rustok-marketplace-commission`](../../crates/modules/rustok-marketplace-commission)** (16 files, 2,662 LOC) — Commission calculation rules, tiered rates, category commission overrides, and fee deduction.
- [ ] **[`crates/modules/rustok-marketplace-ledger`](../../crates/modules/rustok-marketplace-ledger)** (28 files, 6,426 LOC) — Double-entry marketplace financial transactions, payout records, and reconciliation journals.
- [ ] **[`crates/modules/rustok-marketplace-payout`](../../crates/modules/rustok-marketplace-payout)** (13 files, 1,702 LOC) — Vendor payout batching, bank transfer integration, payout scheduling, and payout receipt generation.

---

### Phase FS-05 — Content, Media, Social & Community Domain Modules
*CMS articles, forum discussions, comment trees, emoji reactions, notification dispatch, media asset pipeline, moderation rules, and social network features.* (17/39 completed)

- [x] **[`crates/modules/rustok-blog`](../../crates/modules/rustok-blog)** (158 files, 41,150 LOC) — Blog post lifecycle, publication status machine, author attribution, category/tag taxonomies, revision history, and SEO metadata.
  - *Audited:* Audited in FS-04.01: registered missing blog.post.scheduled and blog.post.schedule_cancelled schemas and updated contract digests in rustok-events; hardened REST controllers and GraphQL query/mutation entrypoints with non-nil UUID validation; added non-nil UUID validation across domain services (PostService, CategoryService, TagService, PreviewTokenService, CommentService, RBAC helpers); bounded token strings and clamped pagination; verified all 150 unit tests pass, module source layout verified.
- [ ] **[`crates/modules/rustok-blog/admin`](../../crates/modules/rustok-blog/admin)** (18 files, 6,393 LOC) — FFA admin UI for rich blog post authoring, publishing workflow, category management, and preview generation.
- [ ] **[`crates/modules/rustok-blog/storefront`](../../crates/modules/rustok-blog/storefront)** (14 files, 3,289 LOC) — Storefront UI for blog post listing, article reading, author cards, and tag filters.
- [x] **[`crates/modules/rustok-forum`](../../crates/modules/rustok-forum)** (628 files, 175,016 LOC) — Forum channels, topics, replies, moderation queue, reputation scoring, full-text search index, topic redirects, and fork/split services.
  - *Audited:* Audited in FS-04.02: enforced non-nil UUID validation across TopicService (slug rename, delete, restore), ReplyService (delete, restore), topic routes (redirects, canonical purging), topic fork storage, topic split services, import writer, topic redirect controller; fail-closed tenant scoping with Column::TenantId filters; implemented canonical From<ForumError> for rustok_core::Error; eliminated dead code; all 241 unit tests pass.
- [ ] **[`crates/modules/rustok-forum/admin`](../../crates/modules/rustok-forum/admin)** (35 files, 13,457 LOC) — FFA admin UI for channel configuration, topic moderation, pinned announcements, and forum permissions.
- [ ] **[`crates/modules/rustok-forum/storefront`](../../crates/modules/rustok-forum/storefront)** (26 files, 7,472 LOC) — Storefront UI for discussion channels, thread views, reply composers, and user badges.
- [x] **[`crates/modules/rustok-content`](../../crates/modules/rustok-content)** (45 files, 6,014 LOC) — CMS content tree, layout block schemas, block persistence, translation slots, dynamic rendering, and route resolution.
  - *Audited:* Audited in FS-04.03: enforced non-nil UUID guards across CMS content tree, layout schema, block persistence, translation slots, dynamic rendering, and GraphQL entrypoints; implemented canonical From<ContentError> for rustok_core::Error; eliminated panic potential in rich text rendering; proptest created_at preservation; all 64 content tests pass.
- [x] **[`crates/modules/rustok-content-orchestration`](../../crates/modules/rustok-content-orchestration)** (18 files, 3,245 LOC) — Content bridge services (topic-to-post promote, demote, split, merge), redirect management, and archive preservation.
  - *Audited:* Audited in FS-04.03: hardened content orchestration bridge (promote_topic_to_post, demote_post_to_topic, split_topic, merge_topics) with non-nil UUID guards and fail-closed tenant-scoped queries/deletions; fixed route resolver to check recorded redirects before live topics and exclude Archived soft-deleted topics; all 4 tests pass.
- [x] **[`crates/integration/rustok-content-revisions`](../../crates/integration/rustok-content-revisions)** (6 files, 903 LOC) — Content revision history integration, snapshot diffing, and audit trail verification.
  - *Audited:* Audited in FS-04.03: verified content revision history integration, snapshot diffing, and audit trail invariants.
- [x] **[`crates/modules/rustok-notifications`](../../crates/modules/rustok-notifications)** (75 files, 22,202 LOC) — Notification template registry, multi-channel dispatch (email/push/in-app), user preferences, and outbox batching/throttling.
  - *Audited:* Audited in FS-04.04: implemented canonical From<NotificationError> for rustok_core::Error; enforced non-nil UUID guards across GraphQL authenticated_scope (tenant_id and user_id), outbox intake process_outbox_event, candidate processing process_candidate_inner, fan-out service materialize_source_event, process_fanout_page, and fanout worker deferred jobs; all 5 tests pass.
- [x] **[`crates/modules/rustok-notifications-api`](../../crates/modules/rustok-notifications-api)** (5 files, 1,196 LOC) — Notification provider seam traits, dispatch commands, and delivery result DTOs.
  - *Audited:* Audited in FS-04.04: implemented canonical From<NotificationProviderError> for rustok_core::Error; verified notification provider seam contracts and DTOs; all 7 tests pass.
- [ ] **[`crates/modules/rustok-notifications/admin`](../../crates/modules/rustok-notifications/admin)** (6 files, 127 LOC) — FFA admin UI for notification template authoring, delivery log inspection, and provider configuration.
- [ ] **[`crates/modules/rustok-notifications/storefront`](../../crates/modules/rustok-notifications/storefront)** (16 files, 3,152 LOC) — Storefront UI notification bell, notification dropdown, and user alert preferences.
- [x] **[`crates/modules/rustok-comments`](../../crates/modules/rustok-comments)** (39 files, 9,823 LOC) — Threaded comment trees, author verification, moderation status transitions, target entity lifecycle hooks, and tenant scoping.
  - *Audited:* Audited in FS-04.05: implemented canonical From<CommentsError> for rustok_core::Error; enforced non-nil UUID validation across CommentsThreadPort and CommentsService (create, get, update, delete, status transitions, threads); non-nil UUID guard to list_public_comments_for_target and CommentTargetDeletionHandler; all 22 tests pass.
- [x] **[`crates/modules/rustok-comments-api`](../../crates/modules/rustok-comments-api)** (3 files, 154 LOC) — Public comment traits, thread DTOs, and target binding contracts.
  - *Audited:* Audited in FS-04.05: verified public comment traits, thread DTOs, and contract invariants.
- [x] **[`crates/modules/rustok-comments-storefront-support`](../../crates/modules/rustok-comments-storefront-support)** (3 files, 171 LOC) — Storefront comment helper utilities, view-models, and formatting primitives.
  - *Audited:* Audited in FS-04.05: verified storefront comment integration helpers and DTOs; all 3 tests pass.
- [ ] **[`crates/modules/rustok-comments/admin`](../../crates/modules/rustok-comments/admin)** (8 files, 1,382 LOC) — FFA admin UI for comment moderation queue, batch approval/deletion, and thread status management.
- [x] **[`crates/modules/rustok-reactions`](../../crates/modules/rustok-reactions)** (8 files, 2,961 LOC) — Polymorphic emoji reactions, counter aggregation, rate limiting, and target entity lifecycle deletion hooks.
  - *Audited:* Audited in FS-04.06: enforced non-nil UUID validation across ReactionsService (parse_tenant_id, authorize_actor), reconciliation (authorize_reconciliation), target lifecycle (ReactionTargetDeletionHandler::handle), and GraphQL entrypoints; all 5 unit tests pass.
- [x] **[`crates/modules/rustok-reactions-api`](../../crates/modules/rustok-reactions-api)** (3 files, 1,262 LOC) — Reaction provider seam contracts, emoji validation rules, and summary DTOs.
  - *Audited:* Audited in FS-04.06: implemented From<ReactionProviderError> and From<ReactionContractError> for rustok_core::Error; verified provider contracts; all 6 tests pass.
- [x] **[`crates/modules/rustok-reactions-storefront`](../../crates/modules/rustok-reactions-storefront)** (6 files, 615 LOC) — Storefront UI reaction buttons, emoji picker popup, and animated counter badges.
  - *Audited:* Audited in FS-04.06: verified storefront reaction rendering components and DTOs; all 7 tests pass.
- [x] **[`crates/modules/rustok-media`](../../crates/modules/rustok-media)** (44 files, 13,332 LOC) — Media asset storage, file upload pipeline, MIME verification, image variant resizing, CDN URL generation, and translation slots.
  - *Audited:* Audited in FS-05.01: implemented canonical From<MediaError> for rustok_core::Error; enforced non-nil UUID validation across MediaAssetReadPort, MediaAssetWritePort, parse_tenant_id, REST controllers, and GraphQL queries/mutations; verified all 49 media tests pass.
- [x] **[`crates/modules/rustok-media-transport`](../../crates/modules/rustok-media-transport)** (6 files, 1,889 LOC) — Media gRPC and REST transport adapters, streaming upload handlers, and ID validation.
  - *Audited:* Audited in FS-05.01: enforced non-nil UUID validation on parse_id in gRPC server and transport adapters; verified all 12 tests pass.
- [ ] **[`crates/modules/rustok-media/admin`](../../crates/modules/rustok-media/admin)** (10 files, 2,125 LOC) — FFA admin UI media library, file uploader, image cropping, and asset translation editor.
- [ ] **[`crates/modules/rustok-media/cli`](../../crates/modules/rustok-media/cli)** (1 files, 175 LOC) — CLI tool for bulk asset ingestion, image optimization, and orphaned file cleanup.
- [x] **[`crates/modules/rustok-moderation`](../../crates/modules/rustok-moderation)** (36 files, 9,292 LOC) — Moderation queue, automated content scanning rules, user reports, case decisions, and penalty enforcement.
  - *Audited:* Audited in FS-05.02: implemented canonical From for rustok_core::Error across ModerationError, ModerationScopeClaimError, ModerationEffectValidationError, ModerationSubjectAdapterBuildError, ModerationSubjectAdapterRegistryError; enforced non-nil UUID validation across ModerationService, ModerationReadPort, command handlers (case_assign, case_decide), and application recovery; all 27 tests pass.
- [x] **[`crates/modules/rustok-moderation-api`](../../crates/modules/rustok-moderation-api)** (3 files, 835 LOC) — Moderation public interfaces, scope claims, effect validation, and case decision DTOs.
  - *Audited:* Audited in FS-05.02: verified moderation public interfaces, scope claims, effect validation, and API contracts; all 5 tests pass.
- [ ] **[`crates/modules/rustok-social-graph`](../../crates/modules/rustok-social-graph)** (36 files, 8,796 LOC) — Follow/unfollow relations, user connections, activity feed fan-out, and social graph queries.
- [ ] **[`crates/modules/rustok-social-graph-cli`](../../crates/modules/rustok-social-graph-cli)** (1 files, 324 LOC) — CLI commands for social graph integrity inspection and graph rebuilding.
- [ ] **[`crates/modules/rustok-groups`](../../crates/modules/rustok-groups)** (112 files, 31,917 LOC) — User groups, community memberships, roles (owner/admin/member), group discussions, and access policies.
- [ ] **[`crates/modules/rustok-groups/admin`](../../crates/modules/rustok-groups/admin)** (34 files, 8,525 LOC) — FFA admin UI for group moderation, community category management, and ownership transfers.
- [ ] **[`crates/modules/rustok-groups/storefront`](../../crates/modules/rustok-groups/storefront)** (17 files, 3,119 LOC) — Storefront UI for group directory, group homepages, member lists, and join requests.
- [ ] **[`crates/modules/rustok-messenger`](../../crates/modules/rustok-messenger)** (49 files, 6,244 LOC) — Direct user messaging, conversation threads, unread message badges, and real-time message events.
- [ ] **[`crates/modules/rustok-messenger/admin`](../../crates/modules/rustok-messenger/admin)** (9 files, 1,189 LOC) — FFA admin UI for message moderation, report review, and conversation audit logs.
- [ ] **[`crates/modules/rustok-profiles`](../../crates/modules/rustok-profiles)** (52 files, 9,224 LOC) — User public profile pages, bios, avatars, badges, social links, and privacy settings.
- [ ] **[`crates/modules/rustok-profiles-api`](../../crates/modules/rustok-profiles-api)** (3 files, 132 LOC) — Public profile contract traits, avatar resolver DTOs, and profile search queries.
- [ ] **[`crates/modules/rustok-profiles/cli`](../../crates/modules/rustok-profiles/cli)** (1 files, 338 LOC) — CLI commands for profile backfills and badge assignments.
- [ ] **[`crates/modules/rustok-profiles/storefront`](../../crates/modules/rustok-profiles/storefront)** (9 files, 1,463 LOC) — Storefront UI user profile cards, activity tabs, and profile editor modal.
- [ ] **[`crates/modules/rustok-newsletter`](../../crates/modules/rustok-newsletter)** (23 files, 2,474 LOC) — Newsletter subscriber management, double opt-in verification, campaign builder, and unsubscribes.
- [ ] **[`crates/modules/rustok-newsletter-api`](../../crates/modules/rustok-newsletter-api)** (4 files, 445 LOC) — Newsletter subscription DTOs, opt-in webhook handlers, and campaign triggers.

---

### Phase FS-06 — Catalog, Taxonomy, Dynamic Extensibility & CMS Pages
*Product catalog, variants, EAV attributes, categories tree closure, brand taxonomy, navigation menus, dynamic page builder, CMS pages, forms, and docs.* (0/26 completed)

- [ ] **[`crates/modules/rustok-product`](../../crates/modules/rustok-product)** (190 files, 81,078 LOC) — Core product catalog, variant matrix, SKU generation, published state machine, category bindings, and EAV attributes.
- [ ] **[`crates/modules/rustok-product/admin`](../../crates/modules/rustok-product/admin)** (29 files, 21,028 LOC) — FFA admin UI for comprehensive product management, variant matrix editor, image gallery, and price rules.
- [ ] **[`crates/modules/rustok-product/storefront`](../../crates/modules/rustok-product/storefront)** (13 files, 4,612 LOC) — Storefront UI product cards, product detail page (PDP), gallery, variant selectors, and stock badges.
- [ ] **[`crates/modules/rustok-product-bundles`](../../crates/modules/rustok-product-bundles)** (23 files, 4,602 LOC) — Product bundle configurations, bundle discounts, component item pricing, and inventory constraints.
- [ ] **[`crates/modules/rustok-product-bundles/admin`](../../crates/modules/rustok-product-bundles/admin)** (9 files, 2,617 LOC) — FFA admin UI for creating and managing bundle sets and discount formulas.
- [ ] **[`crates/modules/rustok-product-relations`](../../crates/modules/rustok-product-relations)** (22 files, 2,720 LOC) — Cross-sell, up-sell, and related product associations with rule-based recommendations.
- [ ] **[`crates/modules/rustok-product-relations/admin`](../../crates/modules/rustok-product-relations/admin)** (9 files, 1,527 LOC) — FFA admin UI for configuring product recommendations and relation rules.
- [ ] **[`crates/modules/rustok-product-transport`](../../crates/modules/rustok-product-transport)** (8 files, 1,704 LOC) — gRPC and REST transport adapters for product catalog queries and mutations.
- [ ] **[`crates/modules/rustok-product-catalog-service`](../../crates/modules/rustok-product-catalog-service)** (1 files, 508 LOC) — In-process catalog read port implementation with price and inventory enrichment.
- [ ] **[`crates/modules/rustok-taxonomy`](../../crates/modules/rustok-taxonomy)** (53 files, 14,828 LOC) — Hierarchical taxonomy trees, terms, faceted navigation filters, and category parent-child closures.
- [ ] **[`crates/modules/rustok-brand`](../../crates/modules/rustok-brand)** (22 files, 3,399 LOC) — Manufacturer and brand directory, brand logos, brand stories, and brand-filtered product listings.
- [ ] **[`crates/modules/rustok-brand/admin`](../../crates/modules/rustok-brand/admin)** (9 files, 2,081 LOC) — FFA admin UI for brand catalog management, logo upload, and brand metadata.
- [ ] **[`crates/modules/rustok-navigation`](../../crates/modules/rustok-navigation)** (40 files, 5,286 LOC) — Hierarchical site menus, header/footer navigation trees, custom URL redirects, and menu item ordering.
- [ ] **[`crates/modules/rustok-navigation/storefront`](../../crates/modules/rustok-navigation/storefront)** (9 files, 495 LOC) — Storefront UI mega-menu, mobile navigation drawer, breadcrumbs, and footer links.
- [ ] **[`crates/modules/alloy`](../../crates/modules/alloy)** (77 files, 25,206 LOC) — Dynamic schema engine, custom entity definitions, relational schemas, and runtime entity validation.
- [ ] **[`crates/modules/flex`](../../crates/modules/flex)** (51 files, 17,159 LOC) — Dynamic custom fields (Flex fields), EAV storage backend, type validation (JSON, text, numbers, relations), and REST endpoints.
- [ ] **[`crates/modules/rustok-page-builder`](../../crates/modules/rustok-page-builder)** (116 files, 35,885 LOC) — Visual page builder layout schema, component tree composition, block configuration, and dynamic block rendering.
- [ ] **[`crates/modules/rustok-page-builder/admin`](../../crates/modules/rustok-page-builder/admin)** (66 files, 19,006 LOC) — FFA admin UI visual drag-and-drop page builder canvas, component palette, and property inspector.
- [ ] **[`crates/modules/rustok-page-builder/storefront`](../../crates/modules/rustok-page-builder/storefront)** (3 files, 1,633 LOC) — Storefront UI block renderer for page-builder compositions and responsive layout grids.
- [ ] **[`crates/modules/rustok-page-builder-storefront`](../../crates/modules/rustok-page-builder-storefront)** (3 files, 1,200 LOC) — Dedicated storefront runtime package for dynamic page-builder page serving.
- [ ] **[`crates/modules/rustok-pages`](../../crates/modules/rustok-pages)** (191 files, 54,667 LOC) — CMS static page management, slug routing, SEO overrides, page revisions, and publish workflow.
- [ ] **[`crates/modules/rustok-pages/admin`](../../crates/modules/rustok-pages/admin)** (25 files, 6,636 LOC) — FFA admin UI for static page editor, metadata fields, and publication scheduling.
- [ ] **[`crates/modules/rustok-pages/storefront`](../../crates/modules/rustok-pages/storefront)** (17 files, 4,935 LOC) — Storefront UI static page renderer, legal pages layout, and about page templates.
- [ ] **[`crates/modules/rustok-forms`](../../crates/modules/rustok-forms)** (17 files, 1,877 LOC) — Dynamic form builder, field validations, submission handling, spam protection, and notification webhooks.
- [ ] **[`crates/modules/rustok-forms/admin`](../../crates/modules/rustok-forms/admin)** (4 files, 274 LOC) — FFA admin UI for form creation, field reordering, and submission inbox review.
- [ ] **[`crates/modules/rustok-docs`](../../crates/modules/rustok-docs)** (21 files, 2,957 LOC) — In-platform technical documentation viewer, markdown parsing, versioned documentation, and doc search.

---

### Phase FS-07 — Search, Indexing, Translation & AI Ecosystem
*Full-text search, faceted search index, SEO metadata, multi-lingual translations, Model Context Protocol (MCP), and AI LLM integrations.* (0/26 completed)

- [ ] **[`crates/modules/rustok-search`](../../crates/modules/rustok-search)** (99 files, 34,372 LOC) — Search execution engine, query parsing, fuzzy matching, faceted filtering, sort ordering, and spellcheck suggestions.
- [ ] **[`crates/modules/rustok-search/admin`](../../crates/modules/rustok-search/admin)** (20 files, 8,298 LOC) — FFA admin UI for search configuration, synonym management, boost rules, and search analytics.
- [ ] **[`crates/modules/rustok-search/storefront`](../../crates/modules/rustok-search/storefront)** (11 files, 4,042 LOC) — Storefront UI search bar with instant autocomplete, search results page, and filter sidebar.
- [ ] **[`crates/modules/rustok-index`](../../crates/modules/rustok-index)** (137 files, 58,160 LOC) — Generic search indexing pipeline, change event intake, document materialization, batch sync, and schema mapping.
- [ ] **[`crates/modules/rustok-index/admin`](../../crates/modules/rustok-index/admin)** (9 files, 2,912 LOC) — FFA admin UI for index status monitoring, manual index rebuilding, and mapping configuration.
- [ ] **[`crates/modules/rustok-seo`](../../crates/modules/rustok-seo)** (82 files, 35,554 LOC) — Automated sitemap.xml generation, robots.txt, canonical URLs, OpenGraph/Twitter card metadata, and structured JSON-LD data.
- [ ] **[`crates/modules/rustok-seo/admin`](../../crates/modules/rustok-seo/admin)** (15 files, 4,557 LOC) — FFA admin UI for global SEO settings, sitemap inspection, and per-entity metadata overrides.
- [ ] **[`crates/modules/rustok-seo/storefront`](../../crates/modules/rustok-seo/storefront)** (6 files, 919 LOC) — Storefront UI meta tag injection, JSON-LD script generation, and canonical link headers.
- [ ] **[`crates/modules/rustok-seo/panel`](../../crates/modules/rustok-seo/panel)** (6 files, 2,754 LOC) — Embedded SEO review panel displaying live snippet preview and score analysis.
- [ ] **[`crates/modules/rustok-seo/render`](../../crates/modules/rustok-seo/render)** (1 files, 17 LOC) — Headless HTML meta rendering engine for SSR bots and crawlers.
- [ ] **[`crates/modules/rustok-seo-admin-support`](../../crates/modules/rustok-seo-admin-support)** (6 files, 2,627 LOC) — Admin integration utilities, score calculation algorithms, and keyword density checks.
- [ ] **[`crates/modules/rustok-seo-targets`](../../crates/modules/rustok-seo-targets)** (1 files, 1,444 LOC) — Contract specifications for SEO target entities across modules.
- [ ] **[`crates/modules/rustok-translation`](../../crates/modules/rustok-translation)** (81 files, 45,996 LOC) — Translation memory, localized string catalogs, fallback chains, translation export/import, and 49-operation admin contract.
- [ ] **[`crates/modules/rustok-translation/admin`](../../crates/modules/rustok-translation/admin)** (9 files, 14,236 LOC) — FFA admin UI for side-by-side translation editor, missing translation filters, and PO/XLIFF export/import.
- [ ] **[`crates/modules/rustok-translation-targets`](../../crates/modules/rustok-translation-targets)** (3 files, 1,729 LOC) — Translation target registration contracts, provider seams, and entity translation keys.
- [ ] **[`crates/modules/rustok-mcp`](../../crates/modules/rustok-mcp)** (26 files, 6,939 LOC) — Model Context Protocol (MCP) server, tool registry, resource providers, context prompts, and secure agent sandboxes.
- [ ] **[`crates/modules/rustok-mcp/admin`](../../crates/modules/rustok-mcp/admin)** (7 files, 1,873 LOC) — FFA admin UI for MCP server status, registered tool catalog, and access token permissions.
- [ ] **[`crates/modules/rustok-ai`](../../crates/modules/rustok-ai)** (74 files, 42,418 LOC) — AI LLM orchestration, provider-neutral embeddings, vector indexing, prompt templates, and streaming completions.
- [ ] **[`crates/modules/rustok-ai/admin`](../../crates/modules/rustok-ai/admin)** (17 files, 8,538 LOC) — FFA admin UI for AI provider credentials, model selection, prompt tuning, and usage metrics.
- [ ] **[`crates/modules/rustok-ai-alloy`](../../crates/modules/rustok-ai-alloy)** (1 files, 384 LOC) — AI assistant integration with Alloy dynamic schemas for natural language data generation.
- [ ] **[`crates/modules/rustok-ai-athanor`](../../crates/modules/rustok-ai-athanor)** (2 files, 548 LOC) — Athanor high-performance data plane for AI vector embeddings and chunked semantic search.
- [ ] **[`crates/modules/rustok-ai-content`](../../crates/modules/rustok-ai-content)** (1 files, 326 LOC) — AI content generator for blog posts, product descriptions, marketing summaries, and SEO tags.
- [ ] **[`crates/modules/rustok-ai-media`](../../crates/modules/rustok-ai-media)** (1 files, 71 LOC) — AI image generation, automated ALT tag generation, and visual similarity search.
- [ ] **[`crates/modules/rustok-ai-order`](../../crates/modules/rustok-ai-order)** (1 files, 255 LOC) — AI customer order assistance, anomaly detection in fraud screening, and predictive reordering.
- [ ] **[`crates/modules/rustok-ai-product`](../../crates/modules/rustok-ai-product)** (1 files, 307 LOC) — AI product categorization, automatic attribute extraction from titles, and variant suggestions.
- [ ] **[`crates/modules/rustok-ai-translation`](../../crates/modules/rustok-ai-translation)** (1 files, 1,655 LOC) — Machine translation bridge integrating OpenAI/DeepL/Claude for batch catalog translation.

---

### Phase FS-08 — Background Workers & Execution Fleet (`crates/workers/*`, `rustok-iggy*`)
*Distributed build workers, artifact node agents, sandbox execution environments, static distribution, verification, and streaming event log infrastructure.* (0/19 completed)

- [ ] **[`crates/workers/rustok-module-build-worker`](../../crates/workers/rustok-module-build-worker)** (8 files, 4,124 LOC) — Compilation worker for native Rust, WASM, and Rhai extension modules with strict caching.
- [ ] **[`crates/workers/rustok-module-build-dispatcher`](../../crates/workers/rustok-module-build-dispatcher)** (3 files, 504 LOC) — Build task scheduler, queue prioritization, node health tracking, and dispatching.
- [ ] **[`crates/workers/rustok-module-build-transport`](../../crates/workers/rustok-module-build-transport)** (6 files, 325 LOC) — gRPC transport contracts for build worker-to-dispatcher communication.
- [ ] **[`crates/workers/rustok-artifact-node-agent`](../../crates/workers/rustok-artifact-node-agent)** (8 files, 2,421 LOC) — Artifact node agent managing local disk caches, signature verification, and binary deployment.
- [ ] **[`crates/workers/rustok-artifact-node-controller`](../../crates/workers/rustok-artifact-node-controller)** (3 files, 137 LOC) — Control-plane controller supervising fleet node topology, heartbeat monitoring, and rollouts.
- [ ] **[`crates/workers/rustok-artifact-node-reconciler`](../../crates/workers/rustok-artifact-node-reconciler)** (3 files, 158 LOC) — Desired vs actual state reconciliation loop for artifact deployments across server clusters.
- [ ] **[`crates/workers/rustok-artifact-node-transport`](../../crates/workers/rustok-artifact-node-transport)** (6 files, 1,016 LOC) — gRPC transport contracts between artifact controllers and fleet node agents.
- [ ] **[`crates/workers/rustok-verification-worker`](../../crates/workers/rustok-verification-worker)** (5 files, 1,121 LOC) — Automated package verification worker validating manifests, checksums, and safety constraints.
- [ ] **[`crates/workers/rustok-verification-transport`](../../crates/workers/rustok-verification-transport)** (4 files, 309 LOC) — gRPC transport for verification worker requests and audit reports.
- [ ] **[`crates/workers/rustok-static-distribution-worker`](../../crates/workers/rustok-static-distribution-worker)** (10 files, 3,397 LOC) — Static asset bundle distribution, compression (gzip/brotli), and edge CDN synchronization.
- [ ] **[`crates/workers/rustok-registry-validation-worker`](../../crates/workers/rustok-registry-validation-worker)** (2 files, 898 LOC) — Marketplace registry package validator enforcing manifest policies and dependency hygiene.
- [ ] **[`crates/workers/rustok-sandbox`](../../crates/workers/rustok-sandbox)** (26 files, 8,071 LOC) — Isolated Rhai/WASM script execution sandbox with memory caps and timeout enforcement.
- [ ] **[`crates/workers/rustok-sandbox-transport`](../../crates/workers/rustok-sandbox-transport)** (5 files, 1,225 LOC) — Streaming bidirectional transport for sandbox command execution and output capture.
- [ ] **[`crates/workers/rustok-sandbox-worker`](../../crates/workers/rustok-sandbox-worker)** (3 files, 730 LOC) — Standalone sandbox worker process executing third-party untrusted scripts safely.
- [ ] **[`crates/workers/rustok-worker-transport`](../../crates/workers/rustok-worker-transport)** (1 files, 472 LOC) — mTLS foundation and encrypted transport layer for worker fleet communication.
- [ ] **[`crates/modules/rustok-iggy`](../../crates/modules/rustok-iggy)** (32 files, 8,270 LOC) — Iggy high-throughput distributed event streaming message bus integration.
- [ ] **[`crates/modules/rustok-iggy-connector`](../../crates/modules/rustok-iggy-connector)** (17 files, 4,593 LOC) — Iggy event sink and source connector translating domain events to Iggy streams.
- [ ] **[`crates/modules/rustok-iggy-connector/admin`](../../crates/modules/rustok-iggy-connector/admin)** (8 files, 614 LOC) — FFA admin UI for Iggy stream health, partition offsets, and consumer lag monitoring.
- [ ] **[`crates/modules/rustok-iggy-connector/next-admin`](../../crates/modules/rustok-iggy-connector/next-admin)** (5 files, 367 LOC) — Next.js admin connector package for Iggy telemetry inspection.

---

### Phase FS-09 — Platform Utilities, Storage & Build Tooling (`crates/utils/*`)
*Installer, migrations engine, secrets manager, object storage abstraction, CLI tooling, test fixtures, and vendored packages.* (0/19 completed)

- [ ] **[`crates/utils/rustok-build`](../../crates/utils/rustok-build)** (12 files, 2,109 LOC) — Build-time code generation scripts, protobuf compilation, and build metadata stamping.
- [ ] **[`crates/utils/rustok-build-publication`](../../crates/utils/rustok-build-publication)** (4 files, 874 LOC) — Cryptographic signing, packaging, and publication pipeline for RusToK packages.
- [ ] **[`crates/utils/rustok-build-source`](../../crates/utils/rustok-build-source)** (2 files, 1,570 LOC) — Deterministic source archive builder, reproducible zip packaging, and SHA256 tree hashing.
- [ ] **[`crates/utils/rustok-cli`](../../crates/utils/rustok-cli)** (3 files, 1,328 LOC) — Main RusToK developer and operations CLI tool for server management, testing, and inspection.
- [ ] **[`crates/utils/rustok-cli-core`](../../crates/utils/rustok-cli-core)** (1 files, 120 LOC) — Shared CLI framework, argument parsing, terminal output formatting, and progress bars.
- [ ] **[`crates/utils/rustok-cli-platform`](../../crates/utils/rustok-cli-platform)** (3 files, 466 LOC) — Platform-level operations commands: database migrations, seed data, and tenant management.
- [ ] **[`crates/utils/rustok-cli-registry`](../../crates/utils/rustok-cli-registry)** (2 files, 131 LOC) — CLI commands for publishing, searching, and managing marketplace registry packages.
- [ ] **[`crates/utils/rustok-installer`](../../crates/utils/rustok-installer)** (11 files, 4,403 LOC) — First-boot interactive and automated installer service provisioning database and superadmin.
- [ ] **[`crates/utils/rustok-installer-cli`](../../crates/utils/rustok-installer-cli)** (1 files, 823 LOC) — CLI binary for running the installer wizard from the terminal.
- [ ] **[`crates/utils/rustok-installer-persistence`](../../crates/utils/rustok-installer-persistence)** (9 files, 1,867 LOC) — Installer state persistence, installation lockfiles, and rollback receipts.
- [ ] **[`crates/utils/rustok-migrations`](../../crates/utils/rustok-migrations)** (70 files, 15,966 LOC) — SeaORM migration scripts, PostgreSQL schema definitions, indices, and migration runner.
- [ ] **[`crates/utils/rustok-module-sdk`](../../crates/utils/rustok-module-sdk)** (2 files, 60 LOC) — Generated guest SDK bindings for developing WASM and native RusToK extensions.
- [ ] **[`crates/utils/rustok-module-template`](../../crates/utils/rustok-module-template)** (2 files, 329 LOC) — Canonical module template structure for scaffolding new RusToK modules.
- [ ] **[`crates/utils/rustok-secrets`](../../crates/utils/rustok-secrets)** (3 files, 1,346 LOC) — Secure secret storage, environment variable resolution, and encryption-at-rest for sensitive configs.
- [ ] **[`crates/utils/rustok-starter`](../../crates/utils/rustok-starter)** (11 files, 1,273 LOC) — Quick-start project generator creating production-ready RusToK deployments.
- [ ] **[`crates/utils/rustok-storage`](../../crates/utils/rustok-storage)** (4 files, 792 LOC) — Unified object storage abstraction supporting Local Filesystem, S3, MinIO, and Azure Blob.
- [ ] **[`crates/utils/rustok-test-utils`](../../crates/utils/rustok-test-utils)** (7 files, 1,949 LOC) — Integration test fixtures, mock database pools, fake event buses, and test assertion helpers.
- [ ] **[`crates/utils/serde-columnar-vendored`](../../crates/utils/serde-columnar-vendored)** (16 files, 2,451 LOC) — Vendored high-performance columnar serialization library for batch data processing.
- [ ] **[`crates/utils/utoipa-swagger-ui-vendored`](../../crates/utils/utoipa-swagger-ui-vendored)** (2 files, 34 LOC) — Vendored Swagger UI assets for embedded OpenAPI interactive documentation.

---

### Phase FS-10 — Frontend Framework, UI Primitives & Design System (`crates/ui/*`)
*Fly design system, core UI component primitives, form controls, data grid, Leptos & Dioxus adapters, and frontend i18n.* (0/27 completed)

- [ ] **[`crates/ui/fly`](../../crates/ui/fly)** (111 files, 42,930 LOC) — Fly Design System: token specifications, typography, color palettes, spacing, and CSS reset.
- [ ] **[`crates/ui/fly-browser`](../../crates/ui/fly-browser)** (9 files, 816 LOC) — Browser-specific DOM manipulation utilities and web platform bindings for Fly.
- [ ] **[`crates/ui/fly-leptos`](../../crates/ui/fly-leptos)** (5 files, 1,691 LOC) — Leptos reactive component bindings for the Fly design system.
- [ ] **[`crates/ui/fly-ui`](../../crates/ui/fly-ui)** (20 files, 5,225 LOC) — Core UI component library for Fly: buttons, inputs, dialogs, badges, and tooltips.
- [ ] **[`crates/ui/fly/browser`](../../crates/ui/fly/browser)** (12 files, 1,585 LOC) — Fly browser-level event handlers, scroll locks, and focus trap primitives.
- [ ] **[`crates/ui/fly/dioxus`](../../crates/ui/fly/dioxus)** (2 files, 393 LOC) — Dioxus component adapter implementations for Fly design system primitives.
- [ ] **[`crates/ui/fly/leptos`](../../crates/ui/fly/leptos)** (5 files, 2,434 LOC) — Leptos component adapter implementations for Fly design system primitives.
- [ ] **[`crates/ui/fly/ui`](../../crates/ui/fly/ui)** (22 files, 6,289 LOC) — Fly UI layout primitives: flex containers, grid rows, cards, and dividers.
- [ ] **[`crates/ui/fly/web`](../../crates/ui/fly/web)** (6 files, 2,234 LOC) — Fly web runtime bindings, SSR hydrators, and stylesheet link injectors.
- [ ] **[`crates/ui/leptos-auth`](../../crates/ui/leptos-auth)** (9 files, 1,506 LOC) — Leptos authentication context, protected route wrappers, and token refresh hooks.
- [ ] **[`crates/ui/leptos-ui`](../../crates/ui/leptos-ui)** (5 files, 415 LOC) — General-purpose Leptos UI components and reactive state utilities.
- [ ] **[`crates/ui/leptos-ui-routing`](../../crates/ui/leptos-ui-routing)** (1 files, 233 LOC) — Leptos router integration, nested sub-routes, and query parameter synchronizers.
- [ ] **[`crates/ui/rustok-forms`](../../crates/ui/rustok-forms)** (21 files, 9,244 LOC) — Fluid Frontend Architecture (FFA) headless form engine: schema validation, field dirty state, and submit handlers.
- [ ] **[`crates/ui/rustok-forms/dioxus`](../../crates/ui/rustok-forms/dioxus)** (6 files, 2,222 LOC) — Dioxus renderer for FFA form schemas and form controls.
- [ ] **[`crates/ui/rustok-forms/leptos`](../../crates/ui/rustok-forms/leptos)** (6 files, 3,126 LOC) — Leptos renderer for FFA form schemas and form controls.
- [ ] **[`crates/ui/rustok-graphql`](../../crates/ui/rustok-graphql)** (1 files, 335 LOC) — Frontend GraphQL client adapter, query builder, and subscription websocket client.
- [ ] **[`crates/ui/rustok-grid`](../../crates/ui/rustok-grid)** (27 files, 6,864 LOC) — Headless high-performance data grid: sorting, multi-column filtering, pagination, and virtual scrolling.
- [ ] **[`crates/ui/rustok-grid/dioxus`](../../crates/ui/rustok-grid/dioxus)** (8 files, 1,726 LOC) — Dioxus virtualized table renderer for rustok-grid.
- [ ] **[`crates/ui/rustok-grid/leptos`](../../crates/ui/rustok-grid/leptos)** (8 files, 1,499 LOC) — Leptos virtualized table renderer for rustok-grid.
- [ ] **[`crates/ui/rustok-ui`](../../crates/ui/rustok-ui)** (46 files, 5,231 LOC) — Shared UI primitives: icons, modal dialogs, drawer sheets, alert toasts, and loading spinners.
- [ ] **[`crates/ui/rustok-ui-auth`](../../crates/ui/rustok-ui-auth)** (1 files, 66 LOC) — Frontend user login, logout, and password recovery UI cards.
- [ ] **[`crates/ui/rustok-ui-core`](../../crates/ui/rustok-ui-core)** (9 files, 2,257 LOC) — Framework-agnostic UI view-models, formatters, and status badge resolvers.
- [ ] **[`crates/ui/rustok-ui-forms`](../../crates/ui/rustok-ui-forms)** (1 files, 75 LOC) — Standardized form control adapters adhering to the RusToK design token system.
- [ ] **[`crates/ui/rustok-ui-i18n`](../../crates/ui/rustok-ui-i18n)** (32 files, 6,793 LOC) — Frontend internationalization: locale switcher, message catalogs, and plural formatting.
- [ ] **[`crates/ui/rustok-ui-transport`](../../crates/ui/rustok-ui-transport)** (1 files, 478 LOC) — Frontend HTTP/GraphQL transport facade abstracting backend API calls.
- [ ] **[`crates/ui/rustok-ui/dioxus`](../../crates/ui/rustok-ui/dioxus)** (20 files, 1,299 LOC) — Dioxus component adapter package for rustok-ui primitives.
- [ ] **[`crates/ui/rustok-ui/leptos`](../../crates/ui/rustok-ui/leptos)** (20 files, 2,046 LOC) — Leptos component adapter package for rustok-ui primitives.

---

### Phase FS-11 — Client Applications & Host Frontends (`apps/*`)
*Full admin dashboards, storefront customer-facing applications, Next.js admin & storefront packages, and Dioxus desktop.* (0/34 completed)

- [ ] **[`apps/admin`](../../apps/admin)** (116 files, 26,837 LOC) — Canonical Rust/Leptos administration dashboard: full navigation, settings, module management, and operator views.
- [ ] **[`apps/storefront`](../../apps/storefront)** (34 files, 5,987 LOC) — Canonical Rust/Leptos e-commerce storefront: catalog navigation, cart drawer, checkout, and account portal.
- [ ] **[`apps/dioxus-admin`](../../apps/dioxus-admin)** (1 files, 13 LOC) — Experimental Dioxus-based desktop/mobile administration client application.
- [ ] **[`apps/next-admin`](../../apps/next-admin)** (540 files, 82,499 LOC) — Next.js administration dashboard application host and composition root.
- [ ] **[`apps/next-admin/packages/blog`](../../apps/next-admin/packages/blog)** (15 files, 2,122 LOC) — Next.js admin package for blog article creation, rich editing, and category management.
- [ ] **[`apps/next-admin/packages/cache`](../../apps/next-admin/packages/cache)** (6 files, 478 LOC) — Next.js admin package for cache inspection, invalidation, and metrics display.
- [ ] **[`apps/next-admin/packages/commerce`](../../apps/next-admin/packages/commerce)** (11 files, 2,737 LOC) — Next.js admin package for orders, payments, fulfillments, and checkout operation inspection.
- [ ] **[`apps/next-admin/packages/email`](../../apps/next-admin/packages/email)** (5 files, 291 LOC) — Next.js admin package for email template management and dispatch logs.
- [ ] **[`apps/next-admin/packages/events`](../../apps/next-admin/packages/events)** (6 files, 350 LOC) — Next.js admin package for event stream monitoring, outbox inspection, and dead-letter queue.
- [ ] **[`apps/next-admin/packages/forms`](../../apps/next-admin/packages/forms)** (5 files, 215 LOC) — Next.js admin package for dynamic form builder and submission reviews.
- [ ] **[`apps/next-admin/packages/forum`](../../apps/next-admin/packages/forum)** (19 files, 4,844 LOC) — Next.js admin package for forum channel configuration and topic moderation.
- [ ] **[`apps/next-admin/packages/iggy-connector`](../../apps/next-admin/packages/iggy-connector)** (5 files, 367 LOC) — Next.js admin package for Iggy streaming broker configuration and metrics.
- [ ] **[`apps/next-admin/packages/media`](../../apps/next-admin/packages/media)** (5 files, 246 LOC) — Next.js admin package for media library management and asset uploads.
- [ ] **[`apps/next-admin/packages/pages`](../../apps/next-admin/packages/pages)** (6 files, 470 LOC) — Next.js admin package for CMS static pages and layout editing.
- [ ] **[`apps/next-admin/packages/rbac`](../../apps/next-admin/packages/rbac)** (13 files, 2,155 LOC) — Next.js admin package for role assignments and permissions matrix editing.
- [ ] **[`apps/next-admin/packages/rustok-ai`](../../apps/next-admin/packages/rustok-ai)** (1 files, 4,055 LOC) — Next.js admin package for AI prompt configuration and LLM provider credentials.
- [ ] **[`apps/next-admin/packages/rustok-mcp`](../../apps/next-admin/packages/rustok-mcp)** (1 files, 1,050 LOC) — Next.js admin package for Model Context Protocol server tools and tokens.
- [ ] **[`apps/next-admin/packages/rustok-product`](../../apps/next-admin/packages/rustok-product)** (37 files, 11,066 LOC) — Next.js admin package for product catalog, variant matrix, and inventory management.
- [ ] **[`apps/next-admin/packages/search`](../../apps/next-admin/packages/search)** (1 files, 2,926 LOC) — Next.js admin package for search index settings and synonym management.
- [ ] **[`apps/next-admin/packages/translation`](../../apps/next-admin/packages/translation)** (4 files, 5,449 LOC) — Next.js admin package for localized translation catalogs and fallback management.
- [ ] **[`apps/next-admin/packages/workflow`](../../apps/next-admin/packages/workflow)** (13 files, 1,529 LOC) — Next.js admin package for workflow trigger configuration and automation rules.
- [ ] **[`apps/next-frontend`](../../apps/next-frontend)** (151 files, 24,872 LOC) — Next.js storefront customer-facing application host and composition root.
- [ ] **[`apps/next-frontend/packages/rustok-blog`](../../apps/next-frontend/packages/rustok-blog)** (16 files, 1,678 LOC) — Next.js storefront blog article pages, reading views, and comments.
- [ ] **[`apps/next-frontend/packages/rustok-cart`](../../apps/next-frontend/packages/rustok-cart)** (7 files, 2,142 LOC) — Next.js storefront shopping cart drawer, line item editing, and promo codes.
- [ ] **[`apps/next-frontend/packages/rustok-comments`](../../apps/next-frontend/packages/rustok-comments)** (2 files, 123 LOC) — Next.js storefront threaded comments widget and submission forms.
- [ ] **[`apps/next-frontend/packages/rustok-forum`](../../apps/next-frontend/packages/rustok-forum)** (20 files, 4,414 LOC) — Next.js storefront community forum discussions, topic views, and replies.
- [ ] **[`apps/next-frontend/packages/rustok-notifications`](../../apps/next-frontend/packages/rustok-notifications)** (7 files, 916 LOC) — Next.js storefront user notification dropdown and preferences.
- [ ] **[`apps/next-frontend/packages/rustok-order`](../../apps/next-frontend/packages/rustok-order)** (5 files, 1,361 LOC) — Next.js storefront order history, order status tracking, and invoices.
- [ ] **[`apps/next-frontend/packages/rustok-pages`](../../apps/next-frontend/packages/rustok-pages)** (8 files, 801 LOC) — Next.js storefront static CMS page rendering and legal terms layouts.
- [ ] **[`apps/next-frontend/packages/rustok-pricing`](../../apps/next-frontend/packages/rustok-pricing)** (7 files, 904 LOC) — Next.js storefront price display, discount badges, and currency selector.
- [ ] **[`apps/next-frontend/packages/rustok-product`](../../apps/next-frontend/packages/rustok-product)** (10 files, 2,252 LOC) — Next.js storefront product grid, search results, and product detail page.
- [ ] **[`apps/next-frontend/packages/rustok-profiles`](../../apps/next-frontend/packages/rustok-profiles)** (7 files, 780 LOC) — Next.js storefront user profile cards, activity history, and settings.
- [ ] **[`apps/next-frontend/packages/rustok-seo`](../../apps/next-frontend/packages/rustok-seo)** (5 files, 1,459 LOC) — Next.js storefront meta tags, structured data, and OpenGraph headers.
- [ ] **[`apps/next-frontend/packages/search`](../../apps/next-frontend/packages/search)** (1 files, 681 LOC) — Next.js storefront search autocomplete bar and search results page.

---

## Components Review Status

| Status | Component | Category | Files | LOC | Last Audited | Notes |
|:---:|---|---|---:|---:|---|---|
| [ ] | [admin](../../apps/admin) | `apps` | 116 | 26,837 | None |  |
| [ ] | [dioxus-admin](../../apps/dioxus-admin) | `apps` | 1 | 13 | None |  |
| [ ] | [next-admin](../../apps/next-admin) | `apps` | 540 | 82,499 | None |  |
| [ ] | [blog](../../apps/next-admin/packages/blog) | `apps` | 15 | 2,122 | None |  |
| [ ] | [cache](../../apps/next-admin/packages/cache) | `apps` | 6 | 478 | None |  |
| [ ] | [commerce](../../apps/next-admin/packages/commerce) | `apps` | 11 | 2,737 | None |  |
| [ ] | [email](../../apps/next-admin/packages/email) | `apps` | 5 | 291 | None |  |
| [ ] | [events](../../apps/next-admin/packages/events) | `apps` | 6 | 350 | None |  |
| [ ] | [forms](../../apps/next-admin/packages/forms) | `apps` | 5 | 215 | None |  |
| [ ] | [forum](../../apps/next-admin/packages/forum) | `apps` | 19 | 4,844 | None |  |
| [ ] | [iggy-connector](../../apps/next-admin/packages/iggy-connector) | `apps` | 5 | 367 | None |  |
| [ ] | [media](../../apps/next-admin/packages/media) | `apps` | 5 | 246 | None |  |
| [ ] | [pages](../../apps/next-admin/packages/pages) | `apps` | 6 | 470 | None |  |
| [ ] | [rbac](../../apps/next-admin/packages/rbac) | `apps` | 13 | 2,155 | None |  |
| [ ] | [rustok-ai](../../apps/next-admin/packages/rustok-ai) | `apps` | 1 | 4,055 | None |  |
| [ ] | [rustok-mcp](../../apps/next-admin/packages/rustok-mcp) | `apps` | 1 | 1,050 | None |  |
| [ ] | [rustok-product](../../apps/next-admin/packages/rustok-product) | `apps` | 37 | 11,066 | None |  |
| [ ] | [search](../../apps/next-admin/packages/search) | `apps` | 1 | 2,926 | None |  |
| [ ] | [translation](../../apps/next-admin/packages/translation) | `apps` | 4 | 5,449 | None |  |
| [ ] | [workflow](../../apps/next-admin/packages/workflow) | `apps` | 13 | 1,529 | None |  |
| [ ] | [next-frontend](../../apps/next-frontend) | `apps` | 151 | 24,872 | None |  |
| [ ] | [rustok-blog](../../apps/next-frontend/packages/rustok-blog) | `apps` | 16 | 1,678 | None |  |
| [ ] | [rustok-cart](../../apps/next-frontend/packages/rustok-cart) | `apps` | 7 | 2,142 | None |  |
| [ ] | [rustok-comments](../../apps/next-frontend/packages/rustok-comments) | `apps` | 2 | 123 | None |  |
| [ ] | [rustok-forum](../../apps/next-frontend/packages/rustok-forum) | `apps` | 20 | 4,414 | None |  |
| [ ] | [rustok-notifications](../../apps/next-frontend/packages/rustok-notifications) | `apps` | 7 | 916 | None |  |
| [ ] | [rustok-order](../../apps/next-frontend/packages/rustok-order) | `apps` | 5 | 1,361 | None |  |
| [ ] | [rustok-pages](../../apps/next-frontend/packages/rustok-pages) | `apps` | 8 | 801 | None |  |
| [ ] | [rustok-pricing](../../apps/next-frontend/packages/rustok-pricing) | `apps` | 7 | 904 | None |  |
| [ ] | [rustok-product](../../apps/next-frontend/packages/rustok-product) | `apps` | 10 | 2,252 | None |  |
| [ ] | [rustok-profiles](../../apps/next-frontend/packages/rustok-profiles) | `apps` | 7 | 780 | None |  |
| [ ] | [rustok-seo](../../apps/next-frontend/packages/rustok-seo) | `apps` | 5 | 1,459 | None |  |
| [ ] | [search](../../apps/next-frontend/packages/search) | `apps` | 1 | 681 | None |  |
| [x] | [server](../../apps/server) | `apps` | 485 | 190,468 | 2026-10-16 | Audited in FS-01: structured fatal logging before otel shutdown, thread floor minimum; graceful shutdown stopping Axum before draining workers, DB pool min/max connection guards, URL authority validation; secret validation forbidding default/sample credentials and dev JWT in production, SQLite fallback prevention; atomic runtime context taking worker handles without loss; background worker drain with tokio::join! and stop broadcast; global route fallback mapping RecordNotFound to 404 and sanitizing DB errors to 500; fail-closed tenant resolution without RFC 9110 smuggling; cross-tenant channel isolation; BCP 47 locale fallback; duplicate Authorization header rejection; rate-limiting structured JSON responses; security headers API CSP; metrics scrape auth with token quote trimming; registry and artifact HTTP policies; constant-time setup token checks in installer; OpenAPI schema method alignment; event bus reference preservation; outbox retention worker fast-pruning; module watchdog monotonicity; settings service secret redaction; RBAC cache invalidation listener with whitespace trimming. |
| [ ] | [storefront](../../apps/storefront) | `apps` | 34 | 5,987 | None |  |
| [x] | [rustok-content-revisions](../../crates/integration/rustok-content-revisions) | `integration` | 6 | 903 | 2026-10-16 | Audited in FS-04.03: verified content revision history integration, snapshot diffing, and audit trail invariants. |
| [ ] | [rustok-api](../../crates/libs/rustok-api) | `libs` | 35 | 7,292 | None |  |
| [ ] | [rustok-content-portability](../../crates/libs/rustok-content-portability) | `libs` | 6 | 617 | None |  |
| [ ] | [rustok-content-portability-api](../../crates/libs/rustok-content-portability-api) | `libs` | 6 | 920 | None |  |
| [ ] | [rustok-core](../../crates/libs/rustok-core) | `libs` | 65 | 17,627 | None |  |
| [ ] | [rustok-events](../../crates/libs/rustok-events) | `libs` | 34 | 12,081 | None |  |
| [ ] | [rustok-fba](../../crates/libs/rustok-fba) | `libs` | 1 | 82 | None |  |
| [ ] | [rustok-manifest](../../crates/libs/rustok-manifest) | `libs` | 6 | 832 | None |  |
| [ ] | [rustok-runtime](../../crates/libs/rustok-runtime) | `libs` | 4 | 2,423 | None |  |
| [ ] | [rustok-telemetry](../../crates/libs/rustok-telemetry) | `libs` | 15 | 3,340 | None |  |
| [ ] | [rustok-web](../../crates/libs/rustok-web) | `libs` | 2 | 742 | None |  |
| [ ] | [alloy](../../crates/modules/alloy) | `modules` | 77 | 25,206 | None |  |
| [ ] | [flex](../../crates/modules/flex) | `modules` | 51 | 17,159 | None |  |
| [ ] | [rustok-accounting](../../crates/modules/rustok-accounting) | `modules` | 13 | 417 | None |  |
| [ ] | [rustok-ai](../../crates/modules/rustok-ai) | `modules` | 74 | 42,418 | None |  |
| [ ] | [rustok-ai-alloy](../../crates/modules/rustok-ai-alloy) | `modules` | 1 | 384 | None |  |
| [ ] | [rustok-ai-athanor](../../crates/modules/rustok-ai-athanor) | `modules` | 2 | 548 | None |  |
| [ ] | [rustok-ai-content](../../crates/modules/rustok-ai-content) | `modules` | 1 | 326 | None |  |
| [ ] | [rustok-ai-media](../../crates/modules/rustok-ai-media) | `modules` | 1 | 71 | None |  |
| [ ] | [rustok-ai-order](../../crates/modules/rustok-ai-order) | `modules` | 1 | 255 | None |  |
| [ ] | [rustok-ai-product](../../crates/modules/rustok-ai-product) | `modules` | 1 | 307 | None |  |
| [ ] | [rustok-ai-translation](../../crates/modules/rustok-ai-translation) | `modules` | 1 | 1,655 | None |  |
| [ ] | [admin](../../crates/modules/rustok-ai/admin) | `modules` | 17 | 8,538 | None |  |
| [x] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 50 | 12,952 | 2026-10-16 | Audited in FS-02.04-06: enforced nil UUID validation in access token, OAuth token, and access token decoders; trimmed whitespace in token strings; empty/nil email/tenant guards in password reset, verification, and invite token codecs; safe verify_password oracle leak prevention returning Ok(false); HS256 secret length validation; bootstrap user non-nil checks; custom Debug implementations redacting passwords, tokens, auth codes, and client secrets across all auth commands and DTOs; all 62 unit tests pass. |
| [ ] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 18 | 6,501 | None |  |
| [ ] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 1 | 333 | None |  |
| [x] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 158 | 41,150 | 2026-10-16 | Audited in FS-04.01: registered missing blog.post.scheduled and blog.post.schedule_cancelled schemas and updated contract digests in rustok-events; hardened REST controllers and GraphQL query/mutation entrypoints with non-nil UUID validation; added non-nil UUID validation across domain services (PostService, CategoryService, TagService, PreviewTokenService, CommentService, RBAC helpers); bounded token strings and clamped pagination; verified all 150 unit tests pass, module source layout verified. |
| [ ] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 18 | 6,393 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 14 | 3,289 | None |  |
| [ ] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 22 | 3,399 | None |  |
| [ ] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 9 | 2,081 | None |  |
| [x] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 46 | 15,917 | 2026-10-16 | Audited in FS-02.09-10: unified CacheError domain taxonomy in error.rs with Display, Error, and From conversions; Display and Error on CacheInvalidationMessageError; structured debug tracing on load_or_fill, publish_invalidation, and backend client; structured entry tracing in durable invalidation service, policy, consumer, and versioned invalidation; all 180 unit tests pass. |
| [ ] | [admin](../../crates/modules/rustok-cache/admin) | `modules` | 7 | 926 | None |  |
| [x] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 73 | 19,638 | 2026-10-16 | Audited in FS-03.09: enforced non-nil UUID validation for tenant_id, cart_id, line_item_id, channel_id, customer_id, region_id, shipping_option_id across ports, checkout_snapshot, marketplace_snapshot, atomic_checkout_guard, atomic_checkout_port, owner_ports, promotion_guard, guarded_ports; structured debug tracing; all 32 tests pass. |
| [ ] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 18 | 4,545 | None |  |
| [ ] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 47 | 11,368 | None |  |
| [ ] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 14 | 5,173 | None |  |
| [x] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 39 | 9,823 | 2026-10-16 | Audited in FS-04.05: implemented canonical From<CommentsError> for rustok_core::Error; enforced non-nil UUID validation across CommentsThreadPort and CommentsService (create, get, update, delete, status transitions, threads); non-nil UUID guard to list_public_comments_for_target and CommentTargetDeletionHandler; all 22 tests pass. |
| [x] | [rustok-comments-api](../../crates/modules/rustok-comments-api) | `modules` | 3 | 154 | 2026-10-16 | Audited in FS-04.05: verified public comment traits, thread DTOs, and contract invariants. |
| [x] | [rustok-comments-storefront-support](../../crates/modules/rustok-comments-storefront-support) | `modules` | 3 | 171 | 2026-10-16 | Audited in FS-04.05: verified storefront comment integration helpers and DTOs; all 3 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 8 | 1,382 | None |  |
| [x] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 297 | 132,631 | 2026-10-16 | Audited in FS-03.02-08: enforced non-nil UUID validation across complete_checkout, begin, checkpoint, ensure_cart_tenant, ensure_order_tenant, ensure_payment_collection_tenant; stage execution in standard and owner-ports pipelines; inventory reservation journal and payment stages; multi-stage compensation and failure sweeps; marketplace paid and reversal event inboxes, financial reconciliation operators, provider event adapters; fulfillment create-label recovery, returns lifecycle; admin and storefront REST controllers and GraphQL helper scopes; all 214 tests pass. |
| [x] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 19 | 1,106 | 2026-10-16 | Audited in FS-03.01: implemented From<CommerceError> for CoreError with mapping to NotFound, Validation, and Forbidden; validate_invariants on PriceInput and AdjustInventoryInput guarding against negative prices and nil UUIDs; trimmed search patterns in product_translation_title_search_condition avoiding redundant full EXISTS subqueries; all 8 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-commerce/admin) | `modules` | 21 | 5,889 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-commerce/storefront) | `modules` | 16 | 2,022 | None |  |
| [x] | [rustok-content](../../crates/modules/rustok-content) | `modules` | 45 | 6,014 | 2026-10-16 | Audited in FS-04.03: enforced non-nil UUID guards across CMS content tree, layout schema, block persistence, translation slots, dynamic rendering, and GraphQL entrypoints; implemented canonical From<ContentError> for rustok_core::Error; eliminated panic potential in rich text rendering; proptest created_at preservation; all 64 content tests pass. |
| [x] | [rustok-content-orchestration](../../crates/modules/rustok-content-orchestration) | `modules` | 18 | 3,245 | 2026-10-16 | Audited in FS-04.03: hardened content orchestration bridge (promote_topic_to_post, demote_post_to_topic, split_topic, merge_topics) with non-nil UUID guards and fail-closed tenant-scoped queries/deletions; fixed route resolver to check recorded redirects before live topics and exclude Archived soft-deleted topics; all 4 tests pass. |
| [ ] | [rustok-customer](../../crates/modules/rustok-customer) | `modules` | 26 | 5,273 | None |  |
| [ ] | [admin](../../crates/modules/rustok-customer/admin) | `modules` | 9 | 2,779 | None |  |
| [x] | [rustok-distribution](../../crates/modules/rustok-distribution) | `modules` | 34 | 15,716 | 2026-10-16 | Audited in FS-03.14: verified distribution routes, split fulfillment manifests, carrier integration ports, and non-nil UUID validation; all 10 tests pass. |
| [ ] | [rustok-docs](../../crates/modules/rustok-docs) | `modules` | 21 | 2,957 | None |  |
| [ ] | [rustok-email](../../crates/modules/rustok-email) | `modules` | 6 | 784 | None |  |
| [ ] | [rustok-events-module](../../crates/modules/rustok-events-module) | `modules` | 9 | 571 | None |  |
| [ ] | [admin](../../crates/modules/rustok-events-module/admin) | `modules` | 8 | 508 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-events-module/next-admin) | `modules` | 6 | 278 | None |  |
| [ ] | [rustok-forms](../../crates/modules/rustok-forms) | `modules` | 17 | 1,877 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forms/admin) | `modules` | 4 | 274 | None |  |
| [x] | [rustok-forum](../../crates/modules/rustok-forum) | `modules` | 628 | 175,016 | 2026-10-16 | Audited in FS-04.02: enforced non-nil UUID validation across TopicService (slug rename, delete, restore), ReplyService (delete, restore), topic routes (redirects, canonical purging), topic fork storage, topic split services, import writer, topic redirect controller; fail-closed tenant scoping with Column::TenantId filters; implemented canonical From<ForumError> for rustok_core::Error; eliminated dead code; all 241 unit tests pass. |
| [ ] | [admin](../../crates/modules/rustok-forum/admin) | `modules` | 35 | 13,457 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-forum/storefront) | `modules` | 26 | 7,472 | None |  |
| [x] | [rustok-fulfillment](../../crates/modules/rustok-fulfillment) | `modules` | 76 | 26,555 | 2026-10-16 | Audited in FS-03.14: enforced non-nil UUID validation across FulfillmentReadPort, FulfillmentAdminCommandPort, FulfillmentAdminCreateCommandPort, ShippingOptionReadPort, ShippingOptionAdminCommandPort, and public ports; bounded string inputs and clamped pagination; all 163 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-fulfillment/admin) | `modules` | 8 | 1,916 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-fulfillment/storefront) | `modules` | 12 | 1,252 | None |  |
| [ ] | [rustok-groups](../../crates/modules/rustok-groups) | `modules` | 112 | 31,917 | None |  |
| [ ] | [admin](../../crates/modules/rustok-groups/admin) | `modules` | 34 | 8,525 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-groups/storefront) | `modules` | 17 | 3,119 | None |  |
| [ ] | [rustok-iggy](../../crates/modules/rustok-iggy) | `modules` | 32 | 8,270 | None |  |
| [ ] | [rustok-iggy-connector](../../crates/modules/rustok-iggy-connector) | `modules` | 17 | 4,593 | None |  |
| [ ] | [admin](../../crates/modules/rustok-iggy-connector/admin) | `modules` | 8 | 614 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-iggy-connector/next-admin) | `modules` | 5 | 367 | None |  |
| [ ] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 137 | 58,160 | None |  |
| [ ] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 9 | 2,912 | None |  |
| [x] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 35 | 13,033 | 2026-10-16 | Audited in FS-03.13: enforced non-nil UUID validation across InventoryReservationPort, InventoryReservationIdentityPort, reservation contexts, InventoryService, public channel inventory projection, and stock location translations; bounded string inputs; structured debug tracing; all 37 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 10 | 4,750 | None |  |
| [ ] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 10 | 1,485 | None |  |
| [ ] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 12 | 1,486 | None |  |
| [ ] | [rustok-marketplace-balance](../../crates/modules/rustok-marketplace-balance) | `modules` | 29 | 6,779 | None |  |
| [ ] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 16 | 2,662 | None |  |
| [ ] | [rustok-marketplace-ledger](../../crates/modules/rustok-marketplace-ledger) | `modules` | 28 | 6,426 | None |  |
| [ ] | [rustok-marketplace-listing](../../crates/modules/rustok-marketplace-listing) | `modules` | 31 | 6,455 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-listing/admin) | `modules` | 9 | 1,911 | None |  |
| [ ] | [rustok-marketplace-payout](../../crates/modules/rustok-marketplace-payout) | `modules` | 13 | 1,702 | None |  |
| [ ] | [rustok-marketplace-seller](../../crates/modules/rustok-marketplace-seller) | `modules` | 48 | 10,566 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-seller/admin) | `modules` | 13 | 2,387 | None |  |
| [ ] | [rustok-mcp](../../crates/modules/rustok-mcp) | `modules` | 26 | 6,939 | None |  |
| [ ] | [admin](../../crates/modules/rustok-mcp/admin) | `modules` | 7 | 1,873 | None |  |
| [x] | [rustok-media](../../crates/modules/rustok-media) | `modules` | 44 | 13,332 | 2026-10-16 | Audited in FS-05.01: implemented canonical From<MediaError> for rustok_core::Error; enforced non-nil UUID validation across MediaAssetReadPort, MediaAssetWritePort, parse_tenant_id, REST controllers, and GraphQL queries/mutations; verified all 49 media tests pass. |
| [x] | [rustok-media-transport](../../crates/modules/rustok-media-transport) | `modules` | 6 | 1,889 | 2026-10-16 | Audited in FS-05.01: enforced non-nil UUID validation on parse_id in gRPC server and transport adapters; verified all 12 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-media/admin) | `modules` | 10 | 2,125 | None |  |
| [ ] | [cli](../../crates/modules/rustok-media/cli) | `modules` | 1 | 175 | None |  |
| [ ] | [rustok-messenger](../../crates/modules/rustok-messenger) | `modules` | 49 | 6,244 | None |  |
| [ ] | [admin](../../crates/modules/rustok-messenger/admin) | `modules` | 9 | 1,189 | None |  |
| [x] | [rustok-moderation](../../crates/modules/rustok-moderation) | `modules` | 36 | 9,292 | 2026-10-16 | Audited in FS-05.02: implemented canonical From for rustok_core::Error across ModerationError, ModerationScopeClaimError, ModerationEffectValidationError, ModerationSubjectAdapterBuildError, ModerationSubjectAdapterRegistryError; enforced non-nil UUID validation across ModerationService, ModerationReadPort, command handlers (case_assign, case_decide), and application recovery; all 27 tests pass. |
| [x] | [rustok-moderation-api](../../crates/modules/rustok-moderation-api) | `modules` | 3 | 835 | 2026-10-16 | Audited in FS-05.02: verified moderation public interfaces, scope claims, effect validation, and API contracts; all 5 tests pass. |
| [ ] | [rustok-modules](../../crates/modules/rustok-modules) | `modules` | 242 | 131,685 | None |  |
| [ ] | [rustok-modules-translation](../../crates/modules/rustok-modules-translation) | `modules` | 1 | 822 | None |  |
| [ ] | [cli](../../crates/modules/rustok-modules/cli) | `modules` | 1 | 1,813 | None |  |
| [ ] | [rustok-navigation](../../crates/modules/rustok-navigation) | `modules` | 40 | 5,286 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-navigation/storefront) | `modules` | 9 | 495 | None |  |
| [ ] | [rustok-newsletter](../../crates/modules/rustok-newsletter) | `modules` | 23 | 2,474 | None |  |
| [ ] | [rustok-newsletter-api](../../crates/modules/rustok-newsletter-api) | `modules` | 4 | 445 | None |  |
| [x] | [rustok-notifications](../../crates/modules/rustok-notifications) | `modules` | 75 | 22,202 | 2026-10-16 | Audited in FS-04.04: implemented canonical From<NotificationError> for rustok_core::Error; enforced non-nil UUID guards across GraphQL authenticated_scope (tenant_id and user_id), outbox intake process_outbox_event, candidate processing process_candidate_inner, fan-out service materialize_source_event, process_fanout_page, and fanout worker deferred jobs; all 5 tests pass. |
| [x] | [rustok-notifications-api](../../crates/modules/rustok-notifications-api) | `modules` | 5 | 1,196 | 2026-10-16 | Audited in FS-04.04: implemented canonical From<NotificationProviderError> for rustok_core::Error; verified notification provider seam contracts and DTOs; all 7 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-notifications/admin) | `modules` | 6 | 127 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-notifications/storefront) | `modules` | 16 | 3,152 | None |  |
| [x] | [rustok-order](../../crates/modules/rustok-order) | `modules` | 85 | 22,532 | 2026-10-16 | Audited in FS-03.10: enforced non-nil UUID validation across ports, order service state machine transitions, line items, tax lines, checkout identity, recovery, compensation, settlement, admin commands, post-order commands, and reads; structured debug tracing across boundaries; all 14 unit tests pass. |
| [ ] | [admin](../../crates/modules/rustok-order/admin) | `modules` | 14 | 2,813 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-order/storefront) | `modules` | 12 | 2,171 | None |  |
| [ ] | [rustok-outbox](../../crates/modules/rustok-outbox) | `modules` | 26 | 4,758 | None |  |
| [ ] | [admin](../../crates/modules/rustok-outbox/admin) | `modules` | 8 | 493 | None |  |
| [ ] | [rustok-page-builder](../../crates/modules/rustok-page-builder) | `modules` | 116 | 35,885 | None |  |
| [ ] | [rustok-page-builder-storefront](../../crates/modules/rustok-page-builder-storefront) | `modules` | 3 | 1,200 | None |  |
| [ ] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 66 | 19,006 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-page-builder/storefront) | `modules` | 3 | 1,633 | None |  |
| [ ] | [rustok-pages](../../crates/modules/rustok-pages) | `modules` | 191 | 54,667 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pages/admin) | `modules` | 25 | 6,636 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pages/storefront) | `modules` | 17 | 4,935 | None |  |
| [x] | [rustok-payment](../../crates/modules/rustok-payment) | `modules` | 88 | 24,276 | 2026-10-16 | Audited in FS-03.11: enforced non-nil UUID validation across ports, admin commands/reads, checkout execution/compensation, provider operation/event journals, controllers, and services; bounded pagination parameters; unit tests for non-nil UUID guards and port context validation; all 47 unit tests pass. |
| [ ] | [storefront](../../crates/modules/rustok-payment/storefront) | `modules` | 11 | 1,315 | None |  |
| [x] | [rustok-pricing](../../crates/modules/rustok-pricing) | `modules` | 48 | 19,495 | 2026-10-16 | Audited in FS-03.12: enforced non-nil UUID validation across PricingReadPort, PricingWritePort, PriceListOwnerPort, and PricingService; verified currency rounding discipline via money::round_to_currency; unit tests for port context parsing, non-nil UUID guards, and pricing math; all 17 tests pass. |
| [x] | [rustok-pricing-persistence](../../crates/modules/rustok-pricing-persistence) | `modules` | 5 | 328 | 2026-10-16 | Audited in FS-03.12: verified price list entities, tenant-scoped schema constraints, and persistence layer invariants; all 5 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-pricing/admin) | `modules` | 14 | 7,971 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pricing/storefront) | `modules` | 10 | 2,487 | None |  |
| [ ] | [rustok-product](../../crates/modules/rustok-product) | `modules` | 190 | 81,078 | None |  |
| [ ] | [rustok-product-bundles](../../crates/modules/rustok-product-bundles) | `modules` | 23 | 4,602 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-bundles/admin) | `modules` | 9 | 2,617 | None |  |
| [ ] | [rustok-product-catalog-service](../../crates/modules/rustok-product-catalog-service) | `modules` | 1 | 508 | None |  |
| [ ] | [rustok-product-relations](../../crates/modules/rustok-product-relations) | `modules` | 22 | 2,720 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-relations/admin) | `modules` | 9 | 1,527 | None |  |
| [ ] | [rustok-product-transport](../../crates/modules/rustok-product-transport) | `modules` | 8 | 1,704 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product/admin) | `modules` | 29 | 21,028 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-product/storefront) | `modules` | 13 | 4,612 | None |  |
| [ ] | [rustok-profiles](../../crates/modules/rustok-profiles) | `modules` | 52 | 9,224 | None |  |
| [ ] | [rustok-profiles-api](../../crates/modules/rustok-profiles-api) | `modules` | 3 | 132 | None |  |
| [ ] | [cli](../../crates/modules/rustok-profiles/cli) | `modules` | 1 | 338 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-profiles/storefront) | `modules` | 9 | 1,463 | None |  |
| [x] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 56 | 13,518 | 2026-10-16 | Audited in FS-02.07-08: enriched RbacError taxonomy with domain errors and policy conversion; fast-pathed nil tenant_id and user_id to empty permissions in resolve_permissions_*; guarded load_role_user_ids_on against nil tenant_id; non-nil UUID validation on parse_tenant_id and parse_user_actor; bounded maximum check permissions to 100; guarded RbacRoleAssignmentDbWriter against nil IDs; all 82 tests pass. |
| [ ] | [admin](../../crates/modules/rustok-rbac/admin) | `modules` | 8 | 952 | None |  |
| [ ] | [cli](../../crates/modules/rustok-rbac/cli) | `modules` | 1 | 243 | None |  |
| [x] | [rustok-reactions](../../crates/modules/rustok-reactions) | `modules` | 8 | 2,961 | 2026-10-16 | Audited in FS-04.06: enforced non-nil UUID validation across ReactionsService (parse_tenant_id, authorize_actor), reconciliation (authorize_reconciliation), target lifecycle (ReactionTargetDeletionHandler::handle), and GraphQL entrypoints; all 5 unit tests pass. |
| [x] | [rustok-reactions-api](../../crates/modules/rustok-reactions-api) | `modules` | 3 | 1,262 | 2026-10-16 | Audited in FS-04.06: implemented From<ReactionProviderError> and From<ReactionContractError> for rustok_core::Error; verified provider contracts; all 6 tests pass. |
| [x] | [rustok-reactions-storefront](../../crates/modules/rustok-reactions-storefront) | `modules` | 6 | 615 | 2026-10-16 | Audited in FS-04.06: verified storefront reaction rendering components and DTOs; all 7 tests pass. |
| [ ] | [rustok-region](../../crates/modules/rustok-region) | `modules` | 39 | 8,566 | None |  |
| [ ] | [admin](../../crates/modules/rustok-region/admin) | `modules` | 8 | 2,944 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-region/storefront) | `modules` | 9 | 1,738 | None |  |
| [ ] | [rustok-search](../../crates/modules/rustok-search) | `modules` | 99 | 34,372 | None |  |
| [ ] | [admin](../../crates/modules/rustok-search/admin) | `modules` | 20 | 8,298 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-search/storefront) | `modules` | 11 | 4,042 | None |  |
| [ ] | [rustok-seo](../../crates/modules/rustok-seo) | `modules` | 82 | 35,554 | None |  |
| [ ] | [rustok-seo-admin-support](../../crates/modules/rustok-seo-admin-support) | `modules` | 6 | 2,627 | None |  |
| [ ] | [rustok-seo-targets](../../crates/modules/rustok-seo-targets) | `modules` | 1 | 1,444 | None |  |
| [ ] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 15 | 4,557 | None |  |
| [ ] | [panel](../../crates/modules/rustok-seo/panel) | `modules` | 6 | 2,754 | None |  |
| [ ] | [render](../../crates/modules/rustok-seo/render) | `modules` | 1 | 17 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-seo/storefront) | `modules` | 6 | 919 | None |  |
| [ ] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 36 | 8,796 | None |  |
| [ ] | [rustok-social-graph-cli](../../crates/modules/rustok-social-graph-cli) | `modules` | 1 | 324 | None |  |
| [ ] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 5 | 1,574 | None |  |
| [ ] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 53 | 14,828 | None |  |
| [x] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 28 | 4,305 | 2026-10-16 | Audited in FS-02.01-03: added Validation variant to TenantError mapped to PortError::validation; trimmed and validated slug/name in create/ensure/update; fast-pathed empty slug/domain lookups to NotFound; clamped pagination; guarded nil UUIDs in TenantReadSelector::Id and parse_context_tenant_id; trimmed ActiveModel::new; bounded settings array elements to 256; rejected PostgreSQL null bytes in keys/values; float finiteness checks; all 24 tests passed. |
| [ ] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 8 | 835 | None |  |
| [ ] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 81 | 45,996 | None |  |
| [ ] | [rustok-translation-targets](../../crates/modules/rustok-translation-targets) | `modules` | 3 | 1,729 | None |  |
| [ ] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 9 | 14,236 | None |  |
| [ ] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 52 | 7,328 | None |  |
| [ ] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 14 | 1,434 | None |  |
| [ ] | [fly](../../crates/ui/fly) | `ui` | 111 | 42,930 | None |  |
| [ ] | [fly-browser](../../crates/ui/fly-browser) | `ui` | 9 | 816 | None |  |
| [ ] | [fly-leptos](../../crates/ui/fly-leptos) | `ui` | 5 | 1,691 | None |  |
| [ ] | [fly-ui](../../crates/ui/fly-ui) | `ui` | 20 | 5,225 | None |  |
| [ ] | [browser](../../crates/ui/fly/browser) | `ui` | 12 | 1,585 | None |  |
| [ ] | [dioxus](../../crates/ui/fly/dioxus) | `ui` | 2 | 393 | None |  |
| [ ] | [leptos](../../crates/ui/fly/leptos) | `ui` | 5 | 2,434 | None |  |
| [ ] | [ui](../../crates/ui/fly/ui) | `ui` | 22 | 6,289 | None |  |
| [ ] | [web](../../crates/ui/fly/web) | `ui` | 6 | 2,234 | None |  |
| [ ] | [leptos-auth](../../crates/ui/leptos-auth) | `ui` | 9 | 1,506 | None |  |
| [ ] | [leptos-ui](../../crates/ui/leptos-ui) | `ui` | 5 | 415 | None |  |
| [ ] | [leptos-ui-routing](../../crates/ui/leptos-ui-routing) | `ui` | 1 | 233 | None |  |
| [ ] | [rustok-forms](../../crates/ui/rustok-forms) | `ui` | 21 | 9,244 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-forms/dioxus) | `ui` | 6 | 2,222 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-forms/leptos) | `ui` | 6 | 3,126 | None |  |
| [ ] | [rustok-graphql](../../crates/ui/rustok-graphql) | `ui` | 1 | 335 | None |  |
| [ ] | [rustok-grid](../../crates/ui/rustok-grid) | `ui` | 27 | 6,864 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-grid/dioxus) | `ui` | 8 | 1,726 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-grid/leptos) | `ui` | 8 | 1,499 | None |  |
| [ ] | [rustok-ui](../../crates/ui/rustok-ui) | `ui` | 46 | 5,231 | None |  |
| [ ] | [rustok-ui-auth](../../crates/ui/rustok-ui-auth) | `ui` | 1 | 66 | None |  |
| [ ] | [rustok-ui-core](../../crates/ui/rustok-ui-core) | `ui` | 9 | 2,257 | None |  |
| [ ] | [rustok-ui-forms](../../crates/ui/rustok-ui-forms) | `ui` | 1 | 75 | None |  |
| [ ] | [rustok-ui-i18n](../../crates/ui/rustok-ui-i18n) | `ui` | 32 | 6,793 | None |  |
| [ ] | [rustok-ui-transport](../../crates/ui/rustok-ui-transport) | `ui` | 1 | 478 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-ui/dioxus) | `ui` | 20 | 1,299 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-ui/leptos) | `ui` | 20 | 2,046 | None |  |
| [ ] | [rustok-build](../../crates/utils/rustok-build) | `utils` | 12 | 2,109 | None |  |
| [ ] | [rustok-build-publication](../../crates/utils/rustok-build-publication) | `utils` | 4 | 874 | None |  |
| [ ] | [rustok-build-source](../../crates/utils/rustok-build-source) | `utils` | 2 | 1,570 | None |  |
| [ ] | [rustok-cli](../../crates/utils/rustok-cli) | `utils` | 3 | 1,328 | None |  |
| [ ] | [rustok-cli-core](../../crates/utils/rustok-cli-core) | `utils` | 1 | 120 | None |  |
| [ ] | [rustok-cli-platform](../../crates/utils/rustok-cli-platform) | `utils` | 3 | 466 | None |  |
| [ ] | [rustok-cli-registry](../../crates/utils/rustok-cli-registry) | `utils` | 2 | 131 | None |  |
| [ ] | [rustok-installer](../../crates/utils/rustok-installer) | `utils` | 11 | 4,403 | None |  |
| [ ] | [rustok-installer-cli](../../crates/utils/rustok-installer-cli) | `utils` | 1 | 823 | None |  |
| [ ] | [rustok-installer-persistence](../../crates/utils/rustok-installer-persistence) | `utils` | 9 | 1,867 | None |  |
| [ ] | [rustok-migrations](../../crates/utils/rustok-migrations) | `utils` | 70 | 15,966 | None |  |
| [ ] | [rustok-module-sdk](../../crates/utils/rustok-module-sdk) | `utils` | 2 | 60 | None |  |
| [ ] | [rustok-module-template](../../crates/utils/rustok-module-template) | `utils` | 2 | 329 | None |  |
| [ ] | [rustok-secrets](../../crates/utils/rustok-secrets) | `utils` | 3 | 1,346 | None |  |
| [ ] | [rustok-starter](../../crates/utils/rustok-starter) | `utils` | 11 | 1,273 | None |  |
| [ ] | [rustok-storage](../../crates/utils/rustok-storage) | `utils` | 4 | 792 | None |  |
| [ ] | [rustok-test-utils](../../crates/utils/rustok-test-utils) | `utils` | 7 | 1,949 | None |  |
| [ ] | [serde-columnar-vendored](../../crates/utils/serde-columnar-vendored) | `utils` | 16 | 2,451 | None |  |
| [ ] | [utoipa-swagger-ui-vendored](../../crates/utils/utoipa-swagger-ui-vendored) | `utils` | 2 | 34 | None |  |
| [ ] | [rustok-artifact-node-agent](../../crates/workers/rustok-artifact-node-agent) | `workers` | 8 | 2,421 | None |  |
| [ ] | [rustok-artifact-node-controller](../../crates/workers/rustok-artifact-node-controller) | `workers` | 3 | 137 | None |  |
| [ ] | [rustok-artifact-node-reconciler](../../crates/workers/rustok-artifact-node-reconciler) | `workers` | 3 | 158 | None |  |
| [ ] | [rustok-artifact-node-transport](../../crates/workers/rustok-artifact-node-transport) | `workers` | 6 | 1,016 | None |  |
| [ ] | [rustok-module-build-dispatcher](../../crates/workers/rustok-module-build-dispatcher) | `workers` | 3 | 504 | None |  |
| [ ] | [rustok-module-build-transport](../../crates/workers/rustok-module-build-transport) | `workers` | 6 | 325 | None |  |
| [ ] | [rustok-module-build-worker](../../crates/workers/rustok-module-build-worker) | `workers` | 8 | 4,124 | None |  |
| [ ] | [rustok-registry-validation-worker](../../crates/workers/rustok-registry-validation-worker) | `workers` | 2 | 898 | None |  |
| [ ] | [rustok-sandbox](../../crates/workers/rustok-sandbox) | `workers` | 26 | 8,071 | None |  |
| [ ] | [rustok-sandbox-transport](../../crates/workers/rustok-sandbox-transport) | `workers` | 5 | 1,225 | None |  |
| [ ] | [rustok-sandbox-worker](../../crates/workers/rustok-sandbox-worker) | `workers` | 3 | 730 | None |  |
| [ ] | [rustok-static-distribution-worker](../../crates/workers/rustok-static-distribution-worker) | `workers` | 10 | 3,397 | None |  |
| [ ] | [rustok-verification-transport](../../crates/workers/rustok-verification-transport) | `workers` | 4 | 309 | None |  |
| [ ] | [rustok-verification-worker](../../crates/workers/rustok-verification-worker) | `workers` | 5 | 1,121 | None |  |
| [ ] | [rustok-worker-transport](../../crates/workers/rustok-worker-transport) | `workers` | 1 | 472 | None |  |

---

## Completed Rounds Archive
_No completed rounds yet. Round 1 is currently in progress._
