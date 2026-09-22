---
id: doc://docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md
kind: implementation_plan
language: markdown
source_language: markdown
status: active
---
# Platform Hardening Implementation Plan

## Purpose

This document is the execution plan for moving RusToK from an ambitious development platform to a reproducible, production-ready platform with explicit security, tenancy, external compatibility, release and scale contracts. It also owns the repository-wide canonical-code cleanup track: internal versions, compatibility layers, suppressed dead code, placeholders, and duplicate implementations are removed rather than promoted into permanent architecture.

The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-18 after completing the typed tenant profile, cross-transport tenant isolation coverage, bounded CSP violation collection, server-hosted and standalone script/style-element nonce enforcement, production WSS-only browser connections, zeroing the Rust-hosted and Next/React inline-style baselines, protecting the classic admin bootstrap, removing runtime style generation from the reviewed Next roots and completing dependency feature cleanup.

## Current Revalidation Summary

### Confirmed open high-risk findings

1. The enforced UI Content Security Policy still permits inline style attributes through the explicit `style-src-attr 'unsafe-inline'` rollout boundary. Both exception registers are empty; the Rust gate is `0/0` and the Next gate is `0 props / 0 files / 0 runtime style elements / 0 DOM style writes`. Cross-stack browser evidence and the subsequent enforced promotion remain required before `HARD-101` is complete.
2. Browser E2E runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
3. Two dependency waivers remain until `Cargo.lock` is regenerated after disabling the unused SeaORM migration CLI/MySQL and Postcard heapless default features.
4. Production JWT bootstrap policy validates algorithm-specific key material, issuer, audience and HS256 secret quality; operational key rotation and emergency revocation remain separate production-readiness work.
5. Several source verifiers positively require legacy modules, deprecated methods, dual delivery, compatibility fallbacks, or `#[allow(dead_code)]`. These checks preserve the defect they were intended to monitor.
6. Repository-owned modules maintain internal contract-version families in manifests, evidence, storage envelopes, routes, and generated parity checks even though there is only one current implementation and no released internal consumer to protect.
7. A broad production-source scan found 151 `#[allow(dead_code)]` attributes across 66 files. The largest clusters correspond to incomplete transport, owner, and UI wiring rather than unavoidable compiler blind spots.
8. Some default production compositions use no-op publishers, activation hooks, telemetry, or persistence adapters while reporting successful completion. These are functional gaps, not harmless test doubles.

### Findings closed or materially reduced

1. Plaintext `http:` was removed from the enforced UI CSP `connect-src`, and object/plugin content is blocked.
2. `unsafe-eval` was removed from the enforced UI CSP and the hardening gate prevents its reintroduction.
3. Server-hosted UI responses use one UUIDv4-derived nonce in both the CSP header and request extensions; `script-src 'unsafe-inline'` and inline event handlers are blocked.
4. Style elements share the same request nonce; the broader `style-src 'unsafe-inline'` source was replaced by nonce-bearing `style-src` plus an isolated temporary `style-src-attr` allowance.
5. Embedded admin script and style elements receive a nonce only while rendering the immutable bundled `index.html`; tenant or user-authored HTML is never blanket-authorized.
6. Storefront JSON-LD receives a nonce only through the exact trusted SEO-renderer opening tag; arbitrary script markup remains without a nonce.
7. The standalone admin SSR host installs its own nonce CSP and security headers, validates production HTTPS/HSTS, and applies the request nonce to its only inline auth bootstrap script, including fallback renders.
8. Production UI `connect-src` is restricted to same-origin, HTTPS and WSS on both server-hosted and standalone admin surfaces; plaintext `ws:` is retained only for non-production development profiles.
9. A strict CSP report-only policy contains no `unsafe-inline`, `unsafe-eval`, plaintext HTTP or plaintext WebSocket source and explicitly reports style attributes through `style-src-attr 'none'`.
10. CSP reports are collected through a bounded pre-auth/pre-tenant endpoint with legacy and Reporting API support, origin-only logging, bounded metric labels and a reviewed migration inventory. The standalone admin does not advertise a collector it does not own.
11. The Rust-hosted inline-style exception register is empty. Its gate rejects every new Rust UI `style=` site and has a non-increasing `0/0` ratchet.
12. The Next/React exception register is empty. Its gate scans all reviewed JSX/TSX roots and enforces zero JSX style props, zero registered files, zero runtime style elements and zero direct DOM style writes, with regression markers for every completed migration.
13. The classic bundled admin bootstrap no longer writes `document.documentElement.style`; it toggles the `dark` class while static CSS owns the light/dark color-scheme declarations.
14. The static modular Page Builder grid moved from a style attribute to a Tailwind grid class and is no longer part of the exception surface.
15. Persisted forum category colors are validated before persistence and normalized to the strict `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA` grammar before Rust UI rendering; CSS declaration injection is rejected or falls back instead of being concatenated into `background`.
16. The unreferenced legacy Page Builder `admin_canvas.rs` duplicate was removed after confirming it had no module declaration, path override or source reference.
17. Modular Page Builder layer indentation uses a bounded nine-step class scale instead of an inline `padding-left` declaration.
18. Page Builder hover, selection and insertion overlays use SVG geometry attributes instead of CSS positioning text.
19. Page Builder resize preview and handles use SVG geometry and a closed cursor-class map while retaining pointer capture.
20. Storefront and forum-admin category accents use a finite, build-time-visible class palette selected from validated hex colors and attach no CSS declaration to the DOM.
21. Page Builder custom viewport dimensions and continuous zoom use native SVG dimensions, `viewBox` and `foreignObject` geometry rather than CSS sizing or `transform:scale`.
22. The admin module build indicator uses native `<progress max="100">` and clamps transport progress to `0..=100`.
23. Tenant resolution is a typed enum with an exhaustive canonical resolver; unknown modes fail configuration deserialization and cannot reach a default-tenant catch-all.
24. `DefaultTenant` fallback is forbidden in production, rejected outside header mode and emits dedicated telemetry plus a warning only when it is actually selected.
25. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.
26. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.
27. Tenant runtime behavior is selected by an explicit `multi_tenant`, `single_tenant` or `development` profile; the development profile is forbidden in production.
28. Tenant resolution uses the dedicated `rustok_tenant_resolutions_total` metric with bounded transport, typed source and outcome labels rather than cache-operation telemetry.
29. Negative tenant isolation coverage rejects missing, malformed, unknown, conflicting and disabled tenant assertions across REST, GraphQL HTTP, GraphQL WebSocket and storefront paths.
30. Subdomain tenant resolution requires at least one configured base domain at bootstrap.
31. Production startup requires an explicit HTTPS deployment declaration, and HSTS flag parsing is normalized.
32. The `/catalog*` bypass was reviewed and documented as a global read-only registry boundary; `/v2/catalog/*` mutation routes remain tenant-bound.
33. `modules.toml.example` and `docs/modules/overview.md` were synchronized with `modules.toml`, and an automated drift gate protects them.
34. The stale `quick-xml` advisory waivers were removed after confirming that the package is absent from the resolved `Cargo.lock` graph.
35. Three stale `rustls-webpki` waivers were removed because the resolved version is `0.103.13`, which meets all three patched thresholds.
36. Both `deny.toml` and `.cargo/audit.toml` ignores are governed by the same expiry-enforcing exception register.
37. Unused SeaORM migration CLI/MySQL and Postcard heapless defaults are disabled at workspace level and protected from member override by a repository gate.
38. A dedicated browser Playwright matrix runs smoke tests for `next-admin` and `next-frontend`.
39. Durable tenant cache generation publication aborts and logs an error rather than emitting timestamp zero on a pre-epoch clock anomaly.
40. Production JWT claims cannot use framework defaults; HS256 requires at least 64 bytes and rejects common placeholder or low-diversity secrets.

## Execution Rules

- `modules.toml` is the canonical platform composition source.
- Security and tenant isolation changes must fail closed in production.
- Every exception must have an owner, rationale, compensating control and expiry date.
- Every public performance claim must be backed by a reproducible benchmark specification and archived result.
- A feature is not complete until its supported Rust, Next.js and mobile surfaces have compatibility evidence.
- Compatibility in this plan means an independently deployed public, wire, provider, package-release, or immutable published-migration boundary. It never authorizes two implementations for repository-owned callers.
- A repository-owned contract changes in place: update every caller and its data atomically, then delete the replaced name, format, route, adapter, verifier marker, and current documentation.
- Static/source evidence may describe an incomplete boundary, but it cannot promote a placeholder to runtime-ready status or require a suppression, no-op, or legacy path as proof of completion.
- Direct pushes to `main` are temporary for the initial stabilization batch only. After Phase 0, protected-branch required checks become mandatory.

## Priority Model

| Priority | Meaning | Target response |
|---|---|---|
| P0 | Cross-tenant exposure, authentication bypass, exploitable browser policy, known critical dependency issue | Fix or explicitly disable affected capability immediately |
| P1 | Production reliability, release integrity, missing required test gate, contract drift | Complete before production-ready declaration |
| P2 | Enterprise operations, compliance evidence, resilience, performance regression prevention | Complete before enterprise support |
| P3 | Hyper-scale isolation, regional topology, workload extraction and advanced automation | Execute after stable production baselines |

## Canonical Code Cleanup Track

### Outcome

RusToK keeps one canonical, unversioned implementation for every repository-owned
contract. A cutover updates code, data, callers, transports, tests, fixtures,
evidence, scripts, and current documentation in the same slice. The replaced
implementation is deleted; it is not renamed to `legacy`, hidden behind a
facade, retained as a fallback, or protected by a source-marker verifier.

This track implements the repository-wide zero-legacy and no-stub policy in
[`AGENTS.md`](../../AGENTS.md). It does not remove real business revisions, optimistic concurrency,
published module releases, or independently deployed wire protocols.

### Audit baseline (2026-08-01)

The following numbers are discovery inputs, not debt baselines that may remain:

| Candidate surface | Snapshot | Interpretation |
|---|---:|---|
| Production Rust files with `#[allow(dead_code)]` | 66 files / 151 attributes | Every occurrence requires wire-or-delete review; no module-wide allowance is acceptable. |
| `rustok-module.toml` files with one or more contract-version fields | 31 | Most describe repository-owned FBA relationships and must become canonical identities rather than compatibility ranges. |
| JSON files with `contract_version` | 87 | Classify by actual deployment boundary; internal registry/evidence versions are cleanup scope. |
| JSON files with `schema_version` | 398 | Not 398 automatic defects. Wire/event/import formats may retain a boundary version; fixed current-only evidence and storage envelopes may not. |
| FBA registries containing `remote_adapter_placeholder` | 29 | A placeholder is not runtime evidence. Twenty-two of these registries also claim `boundary_ready`. |
| Suspicious version/legacy/compatibility filenames | 49 | Includes executable debt, negative tests, documentation, and migration history; each needs classification before rename or deletion. |
| Removed RT JSON family | Forum runtime/UI/storage references are removed | Obsolete shared validator/migration code and remaining current guidance are still cleanup scope; no owner may add a new caller. |
| `grapesjs_v1` | No current executable/configuration occurrence | Removed input alias; remaining references are cleanup inventory or immutable decision evidence. |
| `/api/v1/flex` | 21 matches in 4 files | Public-looking but no independent consumer evidence found; boundary proof is required or the route becomes `/api/flex`. |
| `/v2/catalog` | 195 matches in 26 files | Retained external registry protocol, not an internal route family; see the retained-boundary table below. |

The identifier scan also found 69 distinct `_vN` spellings after excluding
`Uuid::new_v4`. Some are external model names, migrations, or wire formats; the
rest include live Product Index V1/V2 branches, Page Builder formats, Index job
envelopes, cache namespaces, idempotency strings, and internal evidence packet
identities. No production `todo!()` or `unimplemented!()` implementation was
found; the five textual matches are negative fixture strings in `xtask` tests.
This does not clear behavioral no-ops, which require composition tests.

The existing `node scripts/verify/verify-api-compatibility-contract.mjs` check
currently fails because `.github/workflows/api-compatibility.yml` no longer
contains the literal `api-breaking-approved` marker required by the verifier.
No workflow file is changed by this planning update. Wave 0 must reconcile the
workflow, approval helper, and structure verifier as one actual policy contract.

### Classification gate

| Classification | Decision rule | Required action |
|---|---|---|
| Internal current-only contract | Every producer and consumer is repository-owned, or only one parser/current implementation exists. | Remove the version field/suffix/range, update data and callers atomically, and retain one canonical name. |
| Internal old/new bridge | Dual read/write/publish, retry-to-old transport, deprecated alias, compatibility re-export, shim, or wrapper delegates to a replaced implementation. | Complete the canonical path and delete the bridge in the same slice. |
| Missing functionality | Code is unused, suppressed, no-op, fake, placeholder, or represented only by static evidence. | Wire observable behavior with runtime tests or delete the capability and mark it incomplete. |
| Independent external boundary | A separately deployed public API, worker, event consumer, provider, or published package/migration demonstrably consumes the identity. | Keep the version only at ingress/egress, record owner and evidence, and map immediately to one unversioned internal model. |
| Domain revision | Entity revision, optimistic-lock token, source sequence, workflow revision, cache generation, or release artifact version is business state rather than an implementation family. | Keep the semantic name and tests; do not classify it by regex alone. |
| Historical evidence | An ADR, published migration identity, or externally relevant audit artifact must remain immutable. | Keep it outside executable/current guidance and link to the canonical current contract. |

If external-consumer evidence is absent, the default classification is internal.
An exception register must never become a list of repository debt or a numeric
"no-regression" baseline.

### Retained and pending boundary decisions

| Surface | Decision | Reason and constraint |
|---|---|---|
| Registry `/v2/catalog/*` and its wire `schema_version` | Retain | `modules.rustok.dev`, `xtask`, and remote validation runners are independently deployed; the [Registry V2 ADR](../../DECISIONS/2026-04-19-registry-v2-clean-contract-without-runtime-compat.md) fixes the clean external protocol. There is no V1 runtime branch, and handlers must map to one canonical principal/governance model. |
| Stripe `/v1/*`, Vault/Kubernetes/Google paths, and third-party model names such as `all-minilm-l6-v2` | Retain | Provider-owned identity; never rename or emulate it internally. |
| Event/outbox schema versions consumed by independently deployed processes | Retain at wire boundary | One typed event is allowed. Dual legacy-plus-typed publication and adapter-to-legacy ingress are not. |
| Cargo/npm/package SemVer and installable registry release versions | Retain | These identify released artifacts, not parallel internal code. |
| Immutable published migration order | Retain only with publication evidence | Unreleased corrective migrations are consolidated into the canonical create/cutover migration. |
| `/api/v1/flex/*` | Proof required in Wave 0 | Keep only if an independent deployed consumer is identified; otherwise cut over atomically to `/api/flex/*`. Do not create `/v2`. |
| `payment-provider-webhook-v1.json` | Proof required in Wave 0 | Provider payload versions may exist at ingress, but the provider-neutral normalized RusToK model and its repository contract should be unversioned. |
| Translation interchange, Channel policy schema, Index job envelopes, and Page Builder static artifacts | Proof required in Wave 0 | Retain a version only for a real exchanged/published artifact. Internal DB jobs, cache entries, and evidence files use the one canonical typed shape. |

### Self-preserving verifier blockers

These are P0 cleanup blockers because the verifier currently rewards the
obsolete implementation. Each verifier changes only after its associated
runtime cutover is complete, and then must reject reintroduction of the old
path.

| Owner slice | Positive legacy evidence | Canonical replacement |
|---|---|---|
| Forum richtext | Resolved 2026-08-01: the verifier now requires canonical document writes and rejects the deleted adapter and format strings. | Keep Next/Leptos on the shared frame and `RichTextDocument`/`RichTextView`; do not restore a format adapter. |
| Profiles through Forum | `verify-forum-search-profile-legacy-mutation-deprecation.mjs` requires seven deprecated methods and an `allow(deprecated)`. | Verify event-atomic `ProfileMutationService`; delete old mutators, contract, and deprecation document. |
| Search | `verify-forum-search-rebuild-scope-preservation.mjs` requires `projector_legacy.rs`, its private module, and `allow(dead_code)`. | Move the required SQL/scope behavior into one `SearchProjector`; forbid the old file/module. |
| Inventory/Checkout | `verify-inventory-availability-quantity-context.mjs` uses `#[allow(dead_code)]` as a source boundary and requires the old checkout composition. | Verify selected staged checkout plus identity-based inventory owner port through observable behavior. |
| Groups | Groups access/application verifiers require `LegacyGroupsService`, `self.legacy`, three legacy modules, and `include!("applications_legacy.rs")`. | Move remaining behavior into one `GroupsService` and canonical applications/invitations owners; verify lifecycle state. |
| SEO | Bulk/diagnostics verifiers require legacy includes; diagnostics additionally pins the exact old source hash. | Verify bounded batch behavior and one diagnostics implementation; forbid old includes/files. |
| Product admin | Primary-read/fallback verifiers require an aliased legacy executor and retry-to-GraphQL mutations. | Select exactly one configured transport per call while keeping native and GraphQL surfaces in parity; no runtime old-path retry. |
| Commerce/Order | Checkout/read verifiers require private legacy includes, retained completion code, or unmounted compatibility handlers. | Verify durable staged checkout and typed owner read/write ports; delete retained sources and handlers. |
| Forum Search events | Wire/publisher contracts require legacy root publication before the typed event. | Retain one versioned external event, delete dual publication, legacy ingress, and proof packets for the bridge. |

### Confirmed implementation clusters

| ID | Priority | Scope and evidence | Canonical end state |
|---|---|---|---|
| `CLEAN-001` | P0 | `disableUser` remains in the server schema, a parsing extension rejects it, and Next Admin still invokes it. | Next Admin uses `updateUser(status: INACTIVE)`; remove `disableUser`, `LegacyDisableUserPolicy`, security enum markers, tests, and current docs. |
| `CLEAN-002` | P0 | Build defaults use no-op event publication/activation in production compositions; Page Builder has default no-op telemetry/scenario persistence. | Required operations receive real adapters and observable tests, or the unsupported capability is removed/explicitly disabled rather than reporting success. |
| `CLEAN-010` | P0 | Forum storage, transports, renderer, and lifecycle use one canonical document, stale positive verifier markers are removed, and obsolete core format helpers/migration tooling are deleted. Authoring parity is still partial: Leptos owns topic create/edit and Next owns the reply composer. | Complete matching topic/reply authoring on both hosts and keep the Forum boundary target-only. |
| `CLEAN-011` | P0 | Forum admin retries GraphQL reads through REST and suppresses whole transport modules. | Native `#[server]` is the selected Leptos path, GraphQL remains the parallel headless/CSR contract, and no request retries through a compatibility adapter. |
| `CLEAN-020` | P1 | Product persists `VirtualCategoryRuleV1` with `{version:1}` and registers Product/Variant Index V1 and V2 simultaneously. | Keep the currently richer field/link behavior under `VirtualCategoryRule`, `product_schema`, and `product_variant_schema`; remove the envelope/version branch, rebuild pre-release projections, and register one current schema. Generic Index wire schema capability remains boundary-owned. |
| `CLEAN-021` | P1 | Product admin aliases `transport.rs` as `legacy`, uses native-then-GraphQL fallback, and has eleven fallback mutation files. Product `richtext` attributes are plain textarea strings. | One owner facade selects one transport by host/build policy, both intentional transports reach parity, and aliased/fallback files disappear. Either implement shared richtext for the attribute kind on both hosts or remove the kind. |
| `CLEAN-030` | P1 | Deprecated identity-less Inventory reservation methods, quantity fallback reads, Pricing decimal-plus-cent dual writes, and seven Profiles mutators remain. | Keep identity reservation, inventory levels, decimal prices, and event-atomic profile mutations. Update callers/data first, then delete old ports, columns, fallbacks, methods, and suppressions. |
| `CLEAN-040` | P1 | Search projector wrapper, SEO bounded/unbounded generations, and Groups effective/legacy service families execute in parallel. | Consolidate each owner into one named implementation while preserving bounded reads, scope/ACL behavior, and lifecycle invariants in runtime tests. |
| `CLEAN-050` | P1 | Commerce/Cart/Order contain hardened/legacy financial paths, journaled/legacy fulfillment, mounted/retained storefront checkout, source shims, metadata identity adoption, and duplicate cart guards. | Keep durable staged checkout, typed owner ports, journaled operations, mandatory identities, and one GraphQL owner route; remove shims, metadata adoption, retained completion, and duplicate guards. |
| `CLEAN-051` | P1 | Cart storefront and Commerce admin each maintain separate normal and `_ssr` native adapter files with large divergent implementations. | One native adapter per package with narrow `cfg` sections and shared mapping/error code. |
| `CLEAN-060` | P1 | Auth SSR mirrors LocalStorage into versioned browser cookies; standalone Admin accepts `x-fly-access-token`; `leptos-auth::api` re-exports `transport`. | Server-issued HttpOnly session lifecycle, canonical bearer/session context, direct `leptos_auth::transport` callers, and no browser compatibility token/cookie bridge. |
| `CLEAN-061` | P1 | Workflow list is module-owned while host detail/create/edit code remains duplicated; UI links to a nonexistent old edit route. | `rustok-workflow/admin` owns the full Leptos workflow route family; host duplicates and old links are deleted; the Next owner package remains the sibling adapter. |
| `CLEAN-062` | P1 | Next Admin exports an unused faker-backed production mock database; Notifications exports an unused degraded legacy state. | Delete shipped fakes/sentinels and unused dependencies/types. Tests keep isolated fixtures only. |
| `CLEAN-063` | P1 | Rust Storefront accepts legacy `?lang=` routing and tests/docs preserve it. | Host/server effective locale plus canonical locale path only; delete query parsing and redirect propagation atomically. |
| `CLEAN-070` | P1 | Repository-owned FBA manifests use compatibility families such as `cart.checkout.v2`, `marketplace.family.v3`, version ranges, and compound `*.v1+*.v1` strings. | One unversioned capability/port identity in manifests, DTOs, GraphQL, registries, health evidence, and verifiers. Package/release SemVer remains separate. |
| `CLEAN-071` | P1 | Eight owner contracts still have `-v1`/`-v2` filenames and internal IDs; Page Builder retains other internal version ranges/browser keys, while the `grapesjs_v1` format alias is removed. | Rename remaining files and IDs atomically with every registry/test/doc reference; remove current-only version fields and browser keys. No alias files or redirects. |
| `CLEAN-072` | P1 | Index replay/reconciliation/partition/evidence tools create a large current-only `*_v1` family; moderation stores an internal V1 effect envelope; Pages/SEO cache and operation labels are suffixed. | One typed unversioned internal representation per job/effect/evidence/cache contract. Preserve only proven independently deployed wire identity and historical retained evidence. |
| `CLEAN-080` | P1 | Registry accepts legacy scalar/invalid principals; AI invents legacy resolver config; MCP combines legacy tool filtering with current authorization. | Migrate rows/configuration, require typed principals and canonical security configuration, and keep one fail-closed access policy. Registry external V2 wire identity stays unchanged. |
| `CLEAN-081` | P1 | `rustok-core::events` re-exports contract types from `rustok-events`; `leptos-auth::api` is a compatibility export; the old/new error transition wraps both directions. | Callers import contract types from their owner, runtime types remain with their actual owner, and one domain-to-transport error mapping replaces compatibility re-exports/wrappers. |
| `CLEAN-090` | P1 | Twenty-nine FBA registries advertise `remote_adapter_placeholder`; twenty-two simultaneously claim `boundary_ready` under a temporary static-promotion rule. | Remove placeholder entries from required matrices or implement and test real adapters; downgrade unsupported statuses and remove the temporary promotion rule. |
| `CLEAN-100` | P1 | 151 dead-code suppressions span server, `xtask`, SEO, Search, Forum, Product, Commerce, Inventory, and other UI packages. | Resolve each occurrence by wiring, deletion, `cfg(test)`, or the narrowly documented external-symbol `expect` exception. Final count for repository-owned production code is zero. |

The version-suffixed owner-contract files in `CLEAN-071` are:

- `marketplace-reversal-recovery-v1.json`;
- `fulfillment-checkout-execution-v1.json`;
- `financial-orchestration-v2.json`;
- `seller-balance-transfer-v1.json`;
- `order-checkout-compensation-v1.json`;
- `order-checkout-payment-settlement-v1.json`;
- `payment-checkout-compensation-v1.json`;
- `payment-checkout-execution-v1.json`.

`payment-provider-webhook-v1.json` remains in the Wave 0 boundary-proof queue
rather than being silently grouped with either outcome.

### Execution waves

#### Wave 0 - Freeze the rule and restore truthful status

1. Record an ADR that distinguishes internal current contracts from independently
   deployed versioned boundaries and links the accepted Registry V2 decision.
2. Complete the pending boundary decisions above. Each retained boundary needs
   an owner, actual consumer/deployment evidence, and the exact internal mapping.
3. Keep `docs/api/compatibility-exceptions.json` and API-diff guidance aligned so
   a pre-release breaking change is approved as an atomic cutover, never by adding
   an internal V2 or permanent exception. The register wording is already aligned;
   workflow/comparator semantics and the currently failing structure check remain
   part of the implementation slice.
4. Remove the temporary static `boundary_ready` promotion rule. Downgrade entries
   supported only by source markers and remove `remote_adapter_placeholder` from
   required runtime matrices unless the adapter is implemented in the same slice.
5. Keep the strengthened root and frontend agent rules as the immediate guard.
   Do not introduce a debt-count baseline while cleanup is in progress.

#### Wave 1 - Remove broken and fake success paths

1. Execute `CLEAN-001` so the visible user-deactivation action works through the
   canonical mutation before deleting the tombstone mutation and rejection layer.
2. Execute `CLEAN-002` for Build and Page Builder default compositions.
3. Delete the production Next mock API and Notifications degraded sentinel from
   `CLEAN-062`.
4. Replace associated source-marker tests with observable failure/success tests.

#### Wave 2 - Complete Forum and richtext

1. Execute `CLEAN-010` and `CLEAN-011` as the Forum owner slice defined by the
   richtext plan: backend/data first, then Next and Leptos authoring/read parity.
2. Convert mentions, deletion lifecycle, revisions, Search/Index projections,
   orchestration, and fixtures before deleting the old envelope.
3. Remove legacy Forum/Profile verifier contracts and retain one typed Search
   invalidation wire event without dual publication.
4. Delete obsolete current docs only after data and runtime verification pass.

#### Wave 3 - Remove internal data/model generations

1. Execute `CLEAN-020` for virtual categories and Product Index, including a
   deterministic projection rebuild.
2. Execute `CLEAN-030` in order: Inventory identity callers, inventory read model,
   Pricing decimal persistence, then Profiles mutations.
3. Run PostgreSQL apply-from-zero and incremental migration smoke plus owner
   concurrency/idempotency tests. Do not mutate an operator database implicitly.

#### Wave 4 - Consolidate owner implementations

1. Consolidate Search, then SEO, then Groups under `CLEAN-040`.
2. Execute Product admin `CLEAN-021` after the Product data contract is singular.
3. For every owner, move required behavior first, run runtime tests, delete the old
   file/module, and invert its verifier to reject reintroduction in the same change.

#### Wave 5 - Checkout and frontend ownership

1. Execute `CLEAN-050` in dependency order: Order identity, Inventory identity,
   Cart guard, Fulfillment/financial journals, Commerce staged checkout, GraphQL.
2. Consolidate duplicate native adapter files through `CLEAN-051`.
3. Complete Auth, Workflow, locale routing, alias fields, and misleading UI states
   through `CLEAN-060`, `CLEAN-061`, and `CLEAN-063`.

#### Wave 6 - Remove the internal compatibility system

1. Execute `CLEAN-070` across manifests, manifest validators, FBA registries,
   health evidence, DTOs, GraphQL fields, and module docs atomically by capability
   family; do not leave version aliases between slices.
2. Execute Page Builder/Pages and owner-contract rename work in `CLEAN-071`.
3. Execute Index, moderation, cache, operation, and evidence identities in
   `CLEAN-072`, preserving only boundaries approved in Wave 0.

#### Wave 7 - Foundation and security bridges

1. Execute `CLEAN-080`: registry principal data migration, AI configuration, and
   MCP access policy.
2. Execute `CLEAN-081`: event contract imports, auth export removal, and error
   ownership. This is a graph-wide caller cutover, not a compatibility-release task.

#### Wave 8 - Suppressions, migrations, documentation, and final gate

1. Resolve the remaining `CLEAN-100` inventory owner by owner. Test-only helpers
   use `cfg(test)`; Serde fields are validated/consumed or removed; target-specific
   code uses correct `cfg` structure.
2. Consolidate unreleased corrective migrations after proving no published history.
   Preserve an immutable migration identity only with explicit external evidence.
3. Delete superseded implementation documents, contracts, fixtures, proof packets,
   and current guidance. Update every affected module plan, the implementation-plan
   registry, the FFA/FBA readiness board, and this central ledger.
4. Add one strict cross-platform `xtask` canonical-code check only after cleanup.
   It must have no debt-count baseline and may recognize only the exact retained
   external-boundary categories above.

### Per-slice acceptance contract

Every implementation slice is complete only when all of the following hold:

1. The canonical owner and survivor are named before edits begin.
2. Every repository-owned caller, transport, DTO/schema, persisted row, migration,
   seed, fixture, test, verifier, evidence file, and current document is updated.
3. The old file, symbol, route, field, format, adapter, re-export, and fallback are
   deleted in the same change; no deprecated alias or dual read/write/publish remains.
4. Native Leptos and parallel GraphQL/headless surfaces remain intentional parity,
   but a request selects exactly one path rather than retrying through an old path.
5. Behavior is verified at the narrowest real boundary. Source-marker checks may
   supplement runtime evidence but may not declare functionality complete.
6. Targeted forbidden-name searches return no executable/current-guidance matches.
   Any retained hit is identified as an approved external boundary or historical
   artifact, never ignored because it existed before the slice.
7. Local module documentation, central registry/readiness, and `docs/index.md` are
   synchronized where ownership, transport, API, or UI status changed.

### Verification strategy

Start each slice with targeted owner checks, for example:

```powershell
cargo fmt --all -- --check
cargo xtask module validate <slug>
cargo xtask module test <slug>
cargo test -p <owner-crate>
node scripts/verify/<owner-check>.mjs
git diff --check
```

Data or migration slices additionally run the existing PostgreSQL migration smoke
from zero and incrementally, followed by owner-specific concurrency, replay, and
idempotency tests. Frontend slices run the affected Next tests/build plus Leptos
SSR/hydrate checks and browser evidence where editor/session behavior changes.

The final gate will be exposed as:

```powershell
cargo xtask validate-canonical-code
cargo xtask validate-manifest
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo nextest run --workspace --all-targets --all-features
```

`validate-canonical-code` must reject internal version suffixes/routes/envelopes,
legacy/compatibility implementation names, deprecated repository aliases, broad
dead/unused suppressions, positive legacy verifier markers, production stubs/fakes,
and required placeholder readiness. It must distinguish UUIDs, revisions, SemVer,
external provider identities, published migrations, and approved wire boundaries
structurally rather than through a broad text allowlist.

### Exit criteria

- Repository-owned runtime/API/storage/FBA contracts have one unversioned current
  identity and one implementation.
- No runtime dual read/write/publish, retry-to-old transport, deprecated alias,
  compatibility re-export, legacy shim, or source include remains.
- `rt_json_v1`, `rt_json_v2`, `grapesjs_v1`, `VirtualCategoryRuleV1`, Product/Variant
  V1/V2 branches, and current-only Page Builder/Index/evidence version families are
  absent from executable code and current documentation.
- Repository-owned production code has no `allow(dead_code)`, broad `allow(unused*)`,
  or `allow(deprecated)`. Any compiler-blind external symbol uses the smallest
  `expect(..., reason = "...")` plus an entry-path test.
- No production operation succeeds through a no-op publisher, activation hook,
  persistence adapter, fake database, or degraded sentinel.
- FBA readiness is backed by compiled/live behavior; required matrices contain no
  `remote_adapter_placeholder`.
- No verifier positively requires a removed path or suppression.
- Fresh and incremental PostgreSQL migration checks pass, and no corrective
  pre-release migration remains without a publication-history reason.
- Remaining version identifiers are exactly the reviewed external/domain/historical
  categories in this plan and map to one canonical internal model.

### Cleanup progress ledger

| Item | Status | Evidence / next action |
|---|---|---|
| Zero-legacy, no-internal-version, and no-stub agent rules | Completed in working tree | Root `AGENTS.md` plus all four frontend `AI_AGENT_RULES.md`. |
| Cross-repository discovery audit | Completed | Baseline and confirmed clusters in this section, 2026-08-01. |
| API compatibility exception wording | Completed in working tree | `docs/api/compatibility-exceptions.json` now requires atomic pre-release cutover and forbids internal version families/compatibility layers. |
| API compatibility workflow/verifier structure | Open | `verify-api-compatibility-contract.mjs` currently fails on the missing `api-breaking-approved` workflow marker; reconcile only with an explicitly authorized CI-policy change. |
| External-boundary proof review | In progress | Registry V2 and provider identities classified; Flex, normalized payment webhook, interchange/policy, Index jobs, and Page Builder artifacts remain. |
| Wave 1 | Not started | Begin with `CLEAN-001`, then real Build/Page Builder composition. |
| Wave 2 | Not started | Resume the existing Forum/richtext owner plan. |
| Waves 3-8 | Not started | Execute only after prerequisite owner/data cuts above. |

## Phase 0 — Baseline and Trust Restoration

**Goal:** establish a truthful source of truth and prevent regressions while urgent fixes land.

### Work items

- `HARD-001` Synchronize `modules.toml.example` and central module documentation with `modules.toml`.
- `HARD-002` Add a manifest/documentation drift verifier to `cargo xtask validate-manifest` or a dedicated CI script.
- `HARD-003` Add this plan to the documentation map and create a lightweight status ledger.
- `HARD-004` Create `docs/security/advisory-exceptions.md` with owner, affected dependency path, reachability, compensating controls and expiry.
- `HARD-005` Define branch protection: required `CI Success`, signed commits or verified bot identity, no force push, linear history or documented merge policy.
- `HARD-006` Remove unsupported benchmark numbers from README files until reproducible evidence exists.

### Exit criteria

- Canonical module topology and generated documentation are identical.
- Every advisory ignore has time-bounded evidence.
- README claims link to benchmark artifacts or are explicitly labeled as targets.
- Main branch protection is enabled after the initial direct-push stabilization batch.

## Phase 1 — Security and Tenant Isolation

**Goal:** remove fail-open behavior and establish verifiable isolation boundaries.

### Work items

- `HARD-101` Replace UI CSP with nonce/hash-based script and style policies; remove `unsafe-eval`; remove plaintext `http:` from production `connect-src`.
- `HARD-102` Add CSP report-only rollout, violation telemetry and an allowlist inventory before enforcement.
- `HARD-103` Bind HSTS to a validated production HTTPS deployment profile rather than an unvalidated standalone flag.
- `HARD-104` Make unknown tenant resolution modes a bootstrap error and request-time internal error, never a default-tenant fallback.
- `HARD-105` Restrict default-tenant fallback to explicitly declared single-tenant/development profiles and emit metrics whenever it is used.
- `HARD-106` Remove catalog routes from the tenant bypass list unless a reviewed global-catalog data model exists.
- `HARD-107` Add negative integration tests for missing, malformed and attacker-controlled tenant identifiers across REST, GraphQL, WebSocket and storefront paths.
- `HARD-108` Add database-level tenant integrity checks for every tenant-owned relation and validate query filters with integration tests.
- `HARD-109` Make system clock anomalies observable and test cache expiration behavior under clock skew.
- `HARD-110` Validate production JWT policy at bootstrap: allowed algorithms, issuer, audience, key rotation and secret quality.

### Exit criteria

- No request can resolve to a tenant by implicit fallback in production.
- Catalog/global routes have an approved isolation decision and tests.
- Enforced CSP contains no `unsafe-eval`, no inline-script allowance, no blanket inline-style-element allowance, no plaintext production connection source and no inline-style-attribute allowance.
- Tenant isolation tests run as required CI checks.

## Phase 2 — Compatibility, Testing and Release Engineering

**Goal:** turn architecture promises into required, repeatable evidence.

### Work items

- `HARD-201` Add browser E2E jobs for `next-admin` and `next-frontend` to required CI.
- `HARD-202` Add Leptos admin/storefront smoke tests with the same core user journeys.
- `HARD-203` Add mobile package build/analyze/test matrix and API contract smoke tests.
- `HARD-204` Generate and diff OpenAPI and GraphQL compatibility artifacts on every pull request.
- `HARD-205` Add database migration compatibility tests: fresh install, N-1 upgrade, rollback-safe checks and data backfill verification.
- `HARD-206` Establish SemVer tags, signed release artifacts, container publication, checksums, SBOM and provenance attachment.
- `HARD-207` Convert `CHANGELOG.md` to release-oriented entries and move sprint progress to implementation plans or project tracking.
- `HARD-208` Publish a release readiness checklist covering migrations, security exceptions, compatibility, docs and rollback.

### Exit criteria

- Supported hosts have required smoke/E2E evidence.
- Releases are versioned, reproducible and include compatibility and migration notes.
- API breaking changes are detected before merge.

## Phase 3 — Production Readiness and Enterprise Operations

**Goal:** make the platform operable under defined SLOs and failure modes.

### Work items

- `HARD-301` Define SLIs/SLOs for API latency, error rate, availability, outbox lag, search lag, queue depth and tenant-resolution failures.
- `HARD-302` Add bounded concurrency, backpressure, timeouts and cancellation to every worker and outbound integration lane.
- `HARD-303` Add structured audit logs for authentication, authorization, tenant changes, privileged operations and configuration changes.
- `HARD-304` Add backup, point-in-time recovery and restore verification runbooks with scheduled restore drills.
- `HARD-305` Add secret rotation, key rotation and emergency credential revocation runbooks.
- `HARD-306` Add chaos and dependency degradation tests for PostgreSQL, Redis, search, event transport and storage.
- `HARD-307` Add per-tenant quotas, rate limits, storage budgets and noisy-neighbor protection.
- `HARD-308` Produce a compliance evidence pack: threat model, data-flow diagrams, access matrix, dependency inventory and exception register.

### Exit criteria

- Production SLO dashboards and alert policies are live.
- Restore drills and key rotation are tested, not only documented.
- Tenant resource isolation is measurable and enforceable.

## Phase 4 — Hyper-scale Architecture

**Goal:** scale independently without prematurely replacing the modular monolith.

### Work items

- `HARD-401` Profile workload lanes and extract only independently scaling paths: search/index reads, long-running jobs, outbound integrations and AI/operator execution.
- `HARD-402` Introduce durable queue partitioning, idempotency keys, replay policy and poison-message handling.
- `HARD-403` Add regional deployment topology, data residency policy and tenant placement controls.
- `HARD-404` Add cell-based or shard-based tenant placement to limit blast radius.
- `HARD-405` Add capacity models and automated load-shedding based on SLO error budgets.
- `HARD-406` Add continuous performance regression tests for representative read, write, GraphQL and background workloads.

### Exit criteria

- Scaling decisions are driven by measured bottlenecks.
- Failure domains and tenant placement are explicit.
- Performance claims are reproducible across documented hardware and topology profiles.

## Top 20 Ordered Backlog

1. Complete `HARD-101` by capturing cross-stack browser evidence under the strict report-only policy, then promote enforced `style-src-attr` from `'unsafe-inline'` to `'none'` without restoring any source exception.
2. Regenerate `Cargo.lock`, verify `rsa` and `atomic-polyfill` are absent from the selected graph and remove the final two audit waivers.
3. Make `HARD-201` a required branch-protection check.
4. `HARD-204` API compatibility diff gates.
5. `HARD-205` Migration upgrade and rollback verification.
6. `HARD-206` Signed SemVer release workflow and artifacts.
7. `HARD-005` Protected main branch and merge policy.
8. `HARD-006` Benchmark claim evidence cleanup.
9. `HARD-202` Leptos admin/storefront browser smoke coverage.
10. `HARD-305` JWT/key rotation and emergency revocation runbooks.
11. `HARD-301` SLI/SLO definitions and dashboards.
12. `HARD-302` Worker backpressure and cancellation policy.
13. `HARD-307` Per-tenant resource quotas.
14. `HARD-304` Restore drills and disaster recovery evidence.
15. `HARD-306` Dependency degradation and chaos tests.
16. `HARD-406` Reproducible performance regression suite.
17. `HARD-108` Database-level tenant integrity checks for every tenant-owned relation.
18. `HARD-303` Structured audit logs for privileged and tenant-changing operations.
19. `HARD-308` Compliance evidence pack with threat model and data-flow diagrams.
20. `HARD-208` Release readiness checklist with rollback evidence.

## Validation Commands

Run the narrowest checks first, then the full gate:

```bash
cargo fmt --all -- --check
cargo test -p rustok-ui-core
cargo test -p rustok-forum-admin
cargo test -p rustok-forum-storefront
cargo test -p rustok-page-builder-admin
cargo test -p rustok-web
cargo test -p rustok-admin --features ssr app::security
cargo test -p rustok-admin --features ssr app::auth_ssr
cargo test -p rustok-storefront --features ssr
cargo test -p rustok-server services::app_router
cargo test -p rustok-server host::tests
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
cargo test -p rustok-server middleware::tenant
cargo test -p rustok-server --test tenant_resolver_invariants_test
node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
node scripts/verify/verify-csp-next-style-boundary.mjs
node scripts/verify/verify-dependency-feature-hygiene.mjs
node scripts/verify/verify-tenant-resolution-architecture.mjs
node scripts/verify/verify-module-manifest-docs-drift.mjs
node scripts/verify/verify-advisory-exceptions.mjs
cargo generate-lockfile
cargo tree -i rsa --workspace --all-features
cargo tree -i atomic-polyfill --workspace --all-features
cargo audit
cargo xtask validate-manifest
cargo xtask module validate
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo nextest run --workspace --all-targets --all-features
```

For UI compatibility phases:

```bash
npm --prefix apps/next-admin ci
npm --prefix apps/next-admin run test:e2e
npm --prefix apps/next-frontend ci
npm --prefix apps/next-frontend run test:e2e
```

## Progress Ledger

| Work item | Status | Evidence |
|---|---|---|
| `HARD-001` Synchronize manifest documentation | Completed | `f31dc37`, `9303c59` |
| `HARD-002` Automated manifest/docs drift verification | Completed | `f7c1fbe`, `8d6f1fb`, `b579617` |
| `HARD-003` Implementation plan and ledger | Completed | `5eb0687`, this update |
| `HARD-004` Advisory exception governance | Manifest remediation landed; lock refresh pending | Unified register `6b7b6cb`, gate `f9ac9ae`, stale TLS cleanup `c663746`, exact paths `22dcb01`, feature cleanup `c38a8ea`, feature gate `a307cb8`/`0c201ea` |
| `HARD-101` CSP enforcement hardening | Source migration complete; browser evidence and enforced promotion pending | Shared nonce `8492391`; main-server policy `9b1b1af`/`700d4cb`; standalone adapter `8b80543`/`611e50a`; report-only boundary `ac93c41`; Rust `0/0` source gate `e250e42`/`3a61789`; classic admin class-only bootstrap `bf8816e`/`cfc29c4`; Next register/gate `044d4d3`/`4ac34c2`; shell migration `659544b`; chart migration/gate `9187417`/`83aeb33`/`35c546a`; data-table cleanup/gate `7682dc6`/`be1fb0d`/`ba1de73`; storefront search migration `34e8265`; empty Next register and `0/0/0` gate `d54306f`/`a02d69e`; final inventory `266ece9`; browser evidence and enforced promotion remain |
| `HARD-102` CSP report-only and telemetry | Completed | Bounded collector `6c71c30`, minimized telemetry `0990b59`, report headers `ac93c41`, inventory `273ece5`/`8dbd47b`/`c495c1c`/`71522b8`/`cef4a41`/`e81bc42`/`7fff543`, gate `c7436f9`/`85e6e6a`/`389cb07`, middleware test `50ef318` |
| `HARD-103` Production HSTS contract | Completed | `822430e`, `3a9f936`; standalone admin production validation `8b80543`/`611e50a` |
| `HARD-104` Tenant resolution fail-closed | Completed | Typed configuration and canonical resolver `adca4014`; route/header hardening `f3b475e0`; unified HTTP/WS loader `21ad3a99` |
| `HARD-105` Default-tenant fallback restriction | Completed | Explicit runtime profiles, production development-profile ban and fallback/profile validation in tenant hardening batch |
| `HARD-106` Global catalog isolation review | Completed | Boundary test `f1ae6e1`; accepted decision `4d9cbb0`; wrapper parity `8965919` |
| `HARD-107` Negative tenant isolation coverage | Completed | REST, GraphQL HTTP, GraphQL WebSocket and storefront fail-closed tests in tenant hardening batch |
| `HARD-109` Clock anomaly handling | Completed | Durable generation `07ed2ab`; request/cache timestamps return errors; pre-epoch unit coverage |
| Canonical tenant context loading | Completed | Shared HTTP/GraphQL WebSocket read-port pipeline plus dedicated typed-source outcome telemetry |
| `HARD-110` Production JWT bootstrap policy | Implemented; rotation remains operational work | Bootstrap policy `ec5111b`; production example `c6cb4a3` |
| `HARD-201` Browser E2E CI | Implemented, not yet required | Workflow `8982982`; branch-protection requirement unverified |
| Quick-xml advisory debt | Closed | Waivers removed and register entries closed in `0b4d003`, `b988167`, `a6682fc` |
| Rustls-webpki advisory debt | Closed | Patched lock version `0.103.13`; waivers removed in `c663746`; register closed in `22dcb01` |
