---
id: doc://docs/verification/README.md
doc_type: current_contract
status: current
owner: platform-verification
canonical_for:
  - verification-map
language: markdown
---

# Verification Plans

This section collects verification plans for the main platform circuits and captures the minimum local verification path for the module system.

## Purpose

- store verification plans in one place;
- separate periodic verification from live/remediation documentation;
- provide a single entry point for targeted and broad runs;
- capture mandatory quality gates for platform modules.

Execution plans and remediation backlogs should not live in this section as an endless task list. Only verification rules, target commands, and links to profile plans remain here.

## Main Documents

- [Summary Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)
- [Foundation Layer Verification](./platform-foundation-verification-plan.md)
- [API Surface Verification](./platform-api-surfaces-verification-plan.md)
- [Frontend Surface Verification](./platform-frontend-surfaces-verification-plan.md)
- [Quality and Operational Readiness Verification](./platform-quality-operations-verification-plan.md)
  (including Docs quality gates baseline per DOC-07)
- [Core Integrity Verification](./platform-core-integrity-verification-plan.md)
- [RBAC, Server and Runtime Module Verification](./rbac-server-modules-verification-plan.md)
- [Leptos Library Verification](./leptos-libraries-verification-plan.md)

## Inventory reservation adoption

The [Commerce handoff record](../../crates/modules/rustok-commerce/docs/implementation-plan.md#inventory-adoption-source-evidence)
and [Inventory owner plan](../../crates/modules/rustok-inventory/docs/implementation-plan.md#reservation-adoption-into-order-2026-10-31-source-work)
separate source verification from Rust/SQL/contention/mounted/remote evidence.
Read-only source/contract checks (not tests or owner execution):

```bash
node scripts/verify/verify-commerce-checkout-inventory-adoption.mjs
node scripts/verify/verify-commerce-checkout-completion-cutover.mjs
node scripts/verify/verify-checkout-execution-admission-contract.mjs
node scripts/verify/verify-checkout-operation-event-contract.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-ecommerce-fba-registries.mjs
node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs
```

Maintainer-run regression sources; **not executed by this continuation**:

```bash
cargo test -p rustok-inventory --lib
cargo test -p rustok-commerce --lib
cargo test -p rustok-commerce --test inventory_service_test
cargo test -p rustok-commerce --test checkout_service_test
node --test scripts/verify/verify-commerce-checkout-inventory-adoption.test.mjs
```

Adoption uses the same selected identity capability as reserve. Whole owner binding validation
precedes mutation, complete response correlation precedes the separate Commerce journal, and
unverified false advice parks for reconciliation instead of authorizing compensation. Static
matrix/smoke declarations remain runtime-pending; no readiness or full downstream convergence
is inferred from these checks.

## Product downstream SKU contracts

The bounded Product step-8 contract and its unexecuted release work are tracked in the
[Commerce plan](../../crates/modules/rustok-commerce/docs/implementation-plan.md#product-downstream-continuation-2026-10-30)
and the [unified-axis ADR](../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10e-downstream-sku-contracts-2026-10-30).
These read-only source guards check actual owner call sites, shared UUID/quantity/fulfillment
preconditions and admitted Inventory entry routing; they are not Rust, SQL or runtime proof:

```bash
node scripts/verify/verify-commerce-checkout-plan-product-projection-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-plan-marketplace-snapshot-context.mjs
node scripts/verify/verify-commerce-fulfillment-requirement-boundary.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-inventory-port-diagnostic-safety.mjs
node scripts/verify/verify-pricing-product-identity-context.mjs
```

Maintainer-run regression targets (not executed by this source continuation):

```bash
node --test scripts/verify/verify-commerce-checkout-plan-product-projection-context.test.mjs
node --test scripts/verify/verify-pricing-product-identity-context.test.mjs
cargo test -p rustok-product --lib variant_identity_read_port
cargo test -p rustok-pricing --lib product_identity_tests
cargo test -p rustok-channel --lib
cargo test -p rustok-inventory --lib
cargo test -p rustok-commerce --lib
cargo test -p rustok-commerce --test inventory_service_test
```

The targeted follow-up [review](../../crates/modules/rustok-commerce/docs/implementation-plan.md#targeted-pricing-review-2026-10-30)
found two compilation defects missed by syntax/arity checks (optional locale and an aliased
factory) plus Cart scope/context/retryability bugs. The shared Cart quote adapter and its
regressions are source corrections, not compiler or runtime evidence. Include the actual
mounted paths in read-only review:

```bash
node scripts/verify/verify-commerce-graphql-reprice-typed-error.mjs
node scripts/verify/verify-commerce-storefront-checkout-runtime-error-safety.mjs
node scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs
node scripts/verify/verify-commerce-storefront-cart-http-error-safety.mjs
```

The Pricing identity guard checks selected-capability capture at actual composition roots,
required tenant/SKU response correlation, one admitted facade and bounded waiting. Its companion
and Rust cases are source only: the external fixture deliberately has no Product storage, but
no real remote adapter is executed. Missing capability and external SKU write/preview are explicit
non-retryable refusals, not a native fallback or eligibility/reservation proof. The
[Pricing plan](../../crates/modules/rustok-pricing/docs/implementation-plan.md) records the API
changes and remaining write/CAS/receipt work.

Commerce has `autotests = false`; the channel-allocation persistence file is included by the
registered `inventory_service_test` target. New source fixtures cannot certify selected remote
providers, PostgreSQL/SQLite concurrency, current-policy races, deadline cancellation, immutable
recovery, shipment/return/cancellation stock effects, event/outbox/Index convergence or mounted
Next/Leptos/native/GraphQL auth/tenant/channel/locale behavior. Those require real compile,
database, integration and mounted release gates. The architecture dependency guard requires
`cargo metadata`; absent Cargo is a blocked check, not a passing architecture result.

## Canonical HTTP retry-advice continuation

The [owning HTTP contract](../../crates/libs/rustok-web/CRATE_API.md) and
[Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#canonical-http-retry-advice-2026-10-30)
record exact optional owner advice, its actual serializers/OpenAPI references and the live
Next Admin REST consumer. The existing read-only source gate now pins 32 forwarding points in
31 mapper scopes and the canonical body/schema/client path; it is not behavioral proof:

```bash
node scripts/verify/verify-api-surface-contract.mjs
```

Maintainer-run regression targets below are documented but **not executed** by this continuation:

```bash
cargo test -p rustok-web --lib
cargo test -p rustok-server --lib error::tests
cargo test -p rustok-server --test commerce_openapi_contract
node --experimental-strip-types --test apps/next-admin/src/shared/api/http.test.mjs
```

Eight new Rust test functions and eleven MJS test functions cover false/true/unknown/invalid
metadata, unchanged status/redaction, strict optional JSON/schema shape, host delegation and
bounded UTF-8/body/deadline/cancellation behavior. Rust compilation, generated OpenAPI output,
mounted/native/browser/remote IO, SQL and concurrency evidence remain maintainer-run work.
No audit cursor/readiness is advanced and no new write replay or idempotency policy is introduced.

## Cart priced-write continuation

The [Cart owner plan](../../crates/modules/rustok-cart/docs/implementation-plan.md#priced-quantity-adoption-2026-10-30)
and [Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#cart-quantity-adoption-2026-10-30)
distinguish source adoption from compiled/runtime readiness. Read-only source entrypoints:

```bash
node scripts/verify/verify-cart-priced-write-context.mjs
node scripts/verify/verify-cart-storefront-native-error-safety.mjs
node scripts/verify/verify-commerce-graphql-cart-owner-context.mjs
node scripts/verify/verify-commerce-storefront-cart-http-error-safety.mjs
```

The following regression commands are **maintainer-run, not executed** by this continuation:

```bash
cargo test -p rustok-cart --test cart_service_test
cargo test -p rustok-cart --lib pricing_context_guard
cargo test -p rustok-cart --lib services::cart::pricing
cargo test -p rustok-commerce --lib storefront_cart_runtime::inventory_tests
node --test scripts/verify/verify-cart-priced-write-context.test.mjs
node --test scripts/verify/lib/rust-public-surface.test.mjs
node --test scripts/verify/verify-pricing-product-identity-context.test.mjs
```

Fourteen new Rust regression functions, nine Cart MJS guard-regression functions and three shared
public-surface MJS regression functions remain unrun.
Source assertions cover original observation retention, exact tier quantity, checked replacement
stock demand, atomic batch refusal, purpose-limited removal/oversized-cart cleanup and all three
adapter paths. They are not Rust compilation, generated schema, database/locking/concurrency,
native/browser/remote execution, durable replay or downstream event/outbox proof. No audit or
readiness label is advanced. The broader FBA registry gate and architecture/dependency checks
must be reported independently, including pre-existing module drift or unavailable Cargo.

## Cart atomic context Pricing continuation

The [Cart owner record](../../crates/modules/rustok-cart/docs/implementation-plan.md#atomic-context-pricing-adoption-2026-10-31)
and [Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31)
distinguish source-only implementation from compilation/SQL/runtime proof.

Read-only source entrypoint (does not import or run regression companions):

```bash
node scripts/verify/verify-cart-context-pricing-atomicity.mjs
```

It checks one neutral guarded owner command, original-versus-target observation ordering, exact
complete adoption, expected-parent/per-line quantity grouping, dedup/chunk/correlation, inherited
batch deadline, no fallback, all actual REST/GQL/native handoffs, null/group semantics and frozen
recovery. Related source gates retain owner error context and existing public envelopes. Lexical
checks and WASM formatting cannot establish Rust types/lifetimes/features or runtime behavior.

Maintainer-run regression commands, **not executed** by this continuation:

```bash
cargo test -p rustok-cart --test cart_service_test
cargo test -p rustok-commerce --lib storefront_checkout_pricing
cargo test -p rustok-commerce --lib diagnostic_tests
cargo test -p rustok-commerce --lib scalar_shipping_clear_does_not_resurrect
cargo test -p rustok-commerce --test context_service_test
cargo test -p rustok-commerce --test checkout_service_test direct_checkout_recovers_locked
node --test scripts/verify/verify-cart-context-pricing-atomicity.test.mjs
```

Twenty new Rust and eight new MJS regression functions are unrun. Focused cases cover committed
context+prices, refusal/rollback after partial transactional work, stale/purpose/locked fencing,
price scope/coverage, source capability/correlation, per-line tiers/dedup/chunk limits, one deadline,
country membership, shipping clears/overlap, fixed-point bounds and private diagnostic redaction.
Cargo/rustc remain unavailable. No generated transport, real SQL rollback/contention, mounted auth/
cookie transition, browser/native/remote provider, receipt/outbox or downstream convergence gate
is promoted. The remediation gate was attempted without bypass flags and rejects quoted Rust-file
inclusion source markers/documentation in the cumulative diff; the architecture guard separately
fails because Cargo is absent. These are blockers, not passes; the [owner source-check record](../../crates/modules/rustok-cart/docs/implementation-plan.md#executed-scoped-context-source-evidence)
retains the exact scope and limits. The canonical documentation gate was also attempted and fails
on wider ADR-registry/topology/missing-link drift outside this context slice; the 336 scoped local
links and 24 new source-heading references resolve. Neither global governance gate is called passed.

## Documentation & ADR Governance Verification

Platform documentation currency, links, metadata, and ADR governance are checked with:

```bash
npm run verify:docs
```

Constituent gates include:

- `npm run verify:adrs` — checks that every dated file in `DECISIONS/` is indexed exactly once, registry statuses use canonical vocabularies, supersession links resolve, and ADRs contain required governance sections.
- `npm run generate:docs-topology -- --check` — verifies that module topology tables in `docs/modules/overview.md` and Mermaid diagrams in `docs/architecture/diagram.md` stay synchronized with `modules.toml`.
- Internal link and anchor validation across platform entrypoints.
- Detection of unescaped literal escape characters (`\n`, `\t`) in documentation text.
- Prevention of forbidden stale terminology and ungrounded claims.
- Front matter schema validation on governed documentation.

## Minimum Verification Path for Platform Modules

For scoped platform modules, the canonical local path is:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

`module validate` checks the module contract and local docs, while `module test` builds a targeted test/check plan for the module itself and its UI packages.

If the composition contract of the entire platform changes, additionally run:

```powershell
cargo xtask validate-manifest
```

## Reference Artifacts Pipeline (DOC-09 / B11)

For phase 1 of DOC-09, use a single cross-platform Node.js exporter of reference artifacts:

```bash
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
```

In CI and Unix environments, `scripts/verify/export-reference-artifacts.sh` remains a thin
wrapper over the same exporter; there is no separate Bash implementation.

What the script does:

- generates rustdoc for `rustok-server` and `rustok-workflow` (unless `SKIP_RUSTDOC=1` is set);
- saves OpenAPI to `openapi/openapi.json` and `openapi/openapi.yaml`;
- saves full GraphQL introspection to `graphql/introspection.json`;
- saves GraphQL SDL from `/api/graphql/schema.graphql` to `graphql/schema.graphql`;
- writes `manifest.json` and legacy `manifest.txt` with timestamp/base_url/git commit.

Environment variables:

- `RUSTOK_BASE_URL` — base URL of the server (default `http://127.0.0.1:5150`);
- `SKIP_RUSTDOC=1` — skip `cargo doc` and export only API artifacts.

Minimum verification set for PR (B11):

```bash
cargo xtask --help
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
rg -n "openapi/|graphql/|manifest.json" artifacts/reference -S
```

## Reference Artifacts Pipeline in CI (DOC-09 / B12)

Phase 2 for DOC-09 is executed via the CI job `reference-artifacts` in
`.github/workflows/ci.yml`.

The job must:

- start the runtime (`rustok-server`) and wait for `/api/openapi.json`;
- run `scripts/verify/export-reference-artifacts.sh artifacts/reference`;
- verify the layout and export completeness via `node scripts/verify/verify-reference-artifacts.mjs artifacts/reference`;
- publish `artifacts/reference/**` via `actions/upload-artifact`;
- be included in the aggregate gate `ci-success`.

Minimum check for B12 in PR:

```bash
rg -n "reference-artifacts|export-reference-artifacts|upload-artifact|ci-success" .github/workflows/ci.yml
```

## Windows Hybrid Path

On the current Windows environment, the mandatory local verification path must not depend on Bash as a hard prerequisite.

Minimum Windows-native set:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm.cmd run verify:storefront:routes
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

Additionally:

- Python-dependent checks are run via the installed Python.
- Bash-only scripts are allowed as legacy/perimeter checks, but not as the sole way to confirm a module contract on this machine.
- Fast source-level runtime invariant checks that do not require full Rust compilation may live in `scripts/verify/*.mjs`; current example is `node scripts/verify/verify-runtime-context-invariants.mjs` for channel context/cache-key, locale-cache metrics and evidence `pages -> page_builder`.
- The CI job `migration-smoke` sets `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` to its isolated PostgreSQL service and runs `./scripts/verify/verify-migration-smoke.sh` in apply-from-zero and incremental modes. Set the same variable to an isolated instance before local runs.

## Runtime/Backend Regression Runbook

Quick diagnostics for persistent backend/runtime guardrails:

| Symptom                                            | Quick Check                                                                                                                                                                                                                                             | What to Look at on Failure                                                                                                                                                                                                                 |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Drift module graph / `pages` dependencies          | `cargo xtask validate-manifest` + `node scripts/verify/verify-runtime-context-invariants.mjs`                                                                                                                                                           | `modules.toml`, `docs/modules/registry.md`, runtime `dependencies()` evidence and registry contract tests must all consistently hold `pages -> [content, page_builder]`.                                                                   |
| Channel resolution without locale/OAuth dimensions | `node scripts/verify/verify-runtime-context-invariants.mjs` + targeted `cargo test -p rustok-server middleware::channel`                                                                                                                                | Source-order middleware chain must execute as `locale -> auth_context -> channel`; `RequestFacts` must take `ResolvedRequestLocale.effective_locale` and `AuthContextExtension.client_id`, and `ChannelCacheKey` must include both fields. |
| Locale DB amplification / cache regression         | `cargo test -p rustok-server middleware::locale` and check `/metrics` for `rustok_tenant_locale_cache_hits_total`, `rustok_tenant_locale_cache_misses_total`, `rustok_tenant_locale_db_queries_total`, `rustok_tenant_locale_cache_invalidations_total` | Repeated tenant-bound requests within TTL should produce cache hits, disabled locale should remain limited by tenant policy, invalidation/TTL should update the snapshot.                                                                  |
| Migration dependency failure                       | `./scripts/verify/verify-migration-smoke.sh` and `RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 ./scripts/verify/verify-migration-smoke.sh`                                                                                                                      | Check `migration_dependencies()` in module crates, aggregation in server migrator, duplicate/cycle/missing dependency tests and FK/cross-module migration order.                                                                           |

For local short iterations, start with fast source-level checks (`node ...` / `rg`), and leave PostgreSQL smoke for migration changes or CI when the build starts taking too long.

## Roles of `xtask` and `scripts/*` (Updated 2026-05)

To avoid duplicating tooling and diverging contracts:

- `xtask` (Rust) — **canonical entrypoint** for platform and module contracts that must run identically on Linux/macOS/Windows.
- `scripts/verify/*.sh` and `scripts/verify/*.mjs` — **perimeter and specialized audit checks** where fast grep/smoke and shell orchestration are more important.
- `scripts/verify/*.ps1` — parity scripts for Windows where a Bash check is mandatory but must have a native fallback.

Practical criteria for choosing an implementation:

1. **Write in `xtask` (Rust)** if:
   - the check is part of the mandatory module acceptance path;
   - cross-platform support without Bash is required;
   - structured parsing is needed (`modules.toml`, manifests, wiring, registry contracts).
2. **Keep in `sh`/`mjs`** if:
   - it is a perimeter/security smoke with many external CLIs;
   - the check is ad-hoc audit in nature and not a module gate;
   - speed of edits in CI orchestration is critical.
3. **Remove/collapse duplicates** if:
   - one script only proxies another without added logic;
   - the command is already covered by `cargo xtask ...` and does not add a separate contract.

## GitHub Actions Failure Logs

Pull the most recent completed failed GitHub Actions run directly to the local
workspace with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/ci/download-failed-logs.ps1
```

The command uses the GitHub Actions logs API, replaces `errors/`, and writes
the flattened job logs plus `metadata.json` into that one local directory.
It creates no auxiliary workflow runs or artifacts. The directory is ignored
by Git. The existing `nextest-logs` artifact remains available directly from
the main CI workflow for the focused Rust test output.

## Page Builder FBA Verification Baseline (Wave 0/Wave 1 Gate)

For the `page_builder -> pages` track, the mandatory minimum gate before advancing between waves:

```bash
node crates/modules/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
```

Baseline gate composition:

1. parity of provider/consumer contract versions;
2. required fallback/toggle profile structure;
3. toggle profile value consistency (`all_on/publish_off/preview_off/builder_off`).

This baseline gate is a mandatory artifact for Sprint/Wave evidence; the remaining Page Builder work is tracked in `docs/modules/page-builder-backlog.md`.

## What Is Considered Mandatory for Module Unification

When changing the module system or a module's local contract, verify not only the code but also the documentation layer:

- presence of `README.md`, `docs/README.md`, `docs/implementation-plan.md`;
- consistency of `modules.toml` and `rustok-module.toml`;
- correctness of admin/storefront manifest wiring;
- accuracy of central docs in `docs/modules/*` and `docs/index.md`.

Support/capability crates may participate in general documentation unification, but scoped `module validate` applies only to slugs from `modules.toml`.

## How to Use the Plan Set

1. Start with the [summary verification plan](./PLATFORM_VERIFICATION_PLAN.md) if a broad run is needed. Its current-cycle cursor is the source of truth for resume order: Core modules first, then `apps/server`, non-module foundation crates, optional/domain modules, public surfaces, and closing gates.
2. Switch to the profile plan if a specific circuit is changing: foundation, API, frontend, RBAC, UI libraries.
3. For targeted module work, first run `cargo xtask module validate <slug>`, not a full workspace-wide run.
4. Record unresolved blockers in the profile plan or in the local docs of the corresponding component, rather than turning `docs/verification/README.md` into a backlog.

## Cyclic Pre-Release Runs

The summary plan is also the durable controller for repeated agent sweeps. During a
cycle, each visited component records the current cycle identifier, status, findings,
fixes, evidence, next action, and resume command under
`## Periodic release verification handoff` in its existing
`docs/implementation-plan.md`.

Only the summary plan owns resettable queue marks and the active cursor. The local
handoff owns component-specific resume evidence. A local completion from an older
cycle never completes the component in a new cycle. After the closing gate, the agent
increments the cycle identifier, resets the summary queue, and starts again from the
Core modules. `rustok-core` remains a separate foundation crate and must not be
described as a Core module.

### Principle for Operational Script Tests

- Tests in `scripts/tests/*` and `scripts/ci/test_*.py` must use isolated fixture directories (`mktemp` / `tempfile`) and not depend on the current state of the repository.
- The repository may temporarily contain drift/legacy data; this must not make script tests flaky during local runs and in CI.

## Update Policy

When architecture, API, UI contracts, module system, observability, or quality gates change:

1. Update local docs of the affected `apps/*` or `crates/modules/*`.
2. Update the profile verification plan in this folder if the verification procedure itself changed.
3. Update related central docs in `docs/modules/*`, `docs/architecture/*` and `docs/index.md`.
4. If a module's acceptance contract changes, synchronously update the [manifest-layer contract](../modules/manifest.md).

## Statuses

- `Not started`
- `In progress`
- `Completed`
- `Blocked`

> Document status: current. For the module system, this README must remain synchronized with `cargo xtask module validate`, `cargo xtask module test` and central docs in `docs/modules/*`.

## Return-restock identity source contract

The [Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence)
and [accepted native amendment](../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#return-restock-identity-amendment-2026-10-31)
separate source evidence from unrun Rust, SQL/locking, mounted and remote behavior.

Read-only checks (no tests/provider/SQL execution):

```bash
npm run verify:commerce:return-restock-identity
node scripts/verify/verify-order-read-port.mjs
node scripts/verify/verify-order-read-port-diagnostic-safety.mjs
node scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs
```

Maintainer-only, unrun here: `npm run test:verify:commerce:return-restock-identity`,
`cargo test -p rustok-order --lib inventory_binding_tests`,
`cargo test -p rustok-inventory --lib return_restock::tests`,
`cargo test -p rustok-commerce --lib return_restock::tests` and
`cargo test -p rustok-commerce --test order_inventory_reservation_test`.
The read-only guard and companion maintainer command are included in the existing ecommerce
source/test command groups respectively, without changing CI workflows. Source evidence does
not certify the separate shipment/cancel/reopen, historical movement or outbox/Index tail.
The guard also preserves the existing inactive-location storage rule: sorted receiving-location
locks and an actionable archived-write refusal precede levels/receipts, while archived exact
replay remains read-only. The added native regression installs the actual storage guard in its
fixture definition; tests and concurrent PostgreSQL/SQLite archival evidence remain unrun.

## Shipment-stock provenance source contract

The [Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence),
[owning amendment](../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01)
and [source record](../../crates/modules/rustok-commerce/contracts/evidence/shipment-stock-provenance-source.json)
separate the implemented native shipment/cancel/reopen boundary from unrun behavior.

Read-only source checks (not tests, SQL/carrier calls or runtime certification):

```bash
npm run verify:commerce:shipment-stock-provenance
node scripts/verify/verify-commerce-graphql-fulfillment-command-owner-port-cutover.mjs
node scripts/verify/verify-commerce-return-restock-identity.mjs
```

58 focused Rust regression definitions are declared, all uncompiled/unrun: Commerce physical 16,
F owner 22, selected recovery 2, selected pagination 5, I scope/budget 7, internal F current-schema/
outcome 4 and HTTP context 2. The evidence explicitly selects the four relevant internal cases.
The companion declares 30 Node source-mutation cases, also unrun and maintainer-only. Commands to run separately, not executed by the agent:

```bash
cargo check -p rustok-inventory
cargo check -p rustok-fulfillment
cargo check -p rustok-commerce
cargo test -p rustok-commerce --test order_inventory_reservation_test shipment_stock_provenance
cargo test -p rustok-fulfillment --test shipment_stock_owner_test
cargo test -p rustok-fulfillment --test provider_operation_journal_test
cargo test -p rustok-fulfillment --lib services::provider_operation_recovery::tests
cargo test -p rustok-commerce --lib services::order_cancellation::selected_fulfillment_tests
cargo test -p rustok-commerce --lib services::fulfillment_reconciliation::tests
cargo test -p rustok-inventory --lib shipment_stock::scope::tests
cargo test -p rustok-commerce --lib controllers::admin::fulfillments::shipment_context_tests
cargo test -p rustok-commerce --test graphql_runtime_parity_test
npm run test:verify:commerce:shipment-stock-provenance
```

Also run affected existing fulfillment service/progress/provider-journal suites. Real migration
installation/down refusal, PostgreSQL/SQLite transaction upgrades and contention, rollback/global
collisions, known/unknown carrier outcomes, mounted human/OAuth context and selected/remote
adapters require behavioral evidence. Neither source markers, syntax parsing, WASM formatting
nor a deadline phase check supplies that evidence or SQL statement timeout guarantees.

No previous adoption one-shot script is a verification command. Do not rerun it. Both new
migrations are unexecuted: I `m20261101_000025_create_shipment_stock_proofs` and F
`m20261101_000121_enforce_provider_effect_admission` after F120. Drain old stock/provider
writers before coordinated installation/enablement. Verify explicit same-key no-effect requeue,
retained-reference CAS, shared malformed-state fences and private release under current schema.
No historical backfill, applied payload rewrite or existing return-item PK rewrite is introduced.
Unpaid-return eligibility, partial-inspection top-ups and full stock event/outbox/Index/Order/
deployment convergence remain open; Product step 8 is not complete.

### Inventory anonymous quantity prerequisite (source only)

- Read-only source guard: `node scripts/verify/verify-inventory-quantity-release.mjs`.
- Maintainer regressions (uncompiled/unrun by the agent):
  `cargo test --locked -p rustok-inventory --lib services::inventory::reservation_release::tests`,
  `cargo test --locked -p rustok-commerce --test inventory_service_test quantity_release`, and
  `node scripts/verify/verify-inventory-quantity-release.test.mjs`.
- [Owner API](../../crates/modules/rustok-inventory/CRATE_API.md#anonymous-quantity-reservations-2026-11-01-source-only)
  and [source record](../../crates/modules/rustok-inventory/contracts/evidence/quantity-release-source.json):
  27 Rust/10 Node definitions only; no test, SQL, compilation, native/UI, locking, relay or
  notification delivery proof. Both suppression fixtures are explicitly SQLite-only.
- Quantity commands remain non-idempotent. Canonical reservation typed invalidations,
  current actor/context, Rust-generated schema digests and real transport/consumer wiring remain
  pending; a source guard cannot certify Search/generic Index convergence or Product step 8.

## Inventory reservation facts and acknowledged delivery

The [owner contract](../../crates/modules/rustok-inventory/CRATE_API.md#reservation-invalidation-facts-2026-11-01-source-only)
and [source packet](../../crates/modules/rustok-inventory/contracts/evidence/reservation-events-source.json)
separate source wiring from actual SQL, relay/Iggy, receive/cursor ACK, restart and Search proof.
Read-only checks (not tests):

```bash
node scripts/verify/verify-inventory-reservation-events.mjs
node scripts/verify/verify-inventory-quantity-release.mjs
node scripts/verify/verify-inventory-stock-events.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-inventory-admin-native-error-safety.mjs
node scripts/verify/verify-ecommerce-fba-registries.mjs
node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs
```

Maintainer-only regression commands, **not executed by this continuation**:

```bash
cargo test --locked -p rustok-events --lib inventory_reservation
cargo test --locked -p rustok-core --lib events::contract_delivery::tests
cargo test --locked -p rustok-commerce --test inventory_service_test reservation_events
cargo test --locked -p rustok-search --lib reservation_contract_tests
cargo test --locked -p rustok-modules --lib event_delivery::contract_projection_tests
cargo test --locked -p rustok-server --lib services::event_transport_factory::contract_delivery_tests
node scripts/verify/verify-inventory-reservation-events.test.mjs
```

Canonical generation is separate from tests and must use only:

```bash
cargo run --locked -p rustok-events --example event_contract_digests -- --write
```

The generated artifact is unchanged here (Cargo/rustc absent); existing pending Checkout-family
admission remains. All 34 Rust/10 Node definitions are uncompiled/unrun. Transport doubles and
SQLite fault sources do not prove PostgreSQL/MySQL locks, actual Iggy or cursor/ACK behavior.
Product-scope Search serialization needs real concurrency/reordered/rebuild/delete evidence;
generic Index's Inventory-owned source/revision/consumer and Product step 8 remain incomplete.

## Inventory catalog deletion prerequisite

The [Inventory owner plan](../../crates/modules/rustok-inventory/docs/implementation-plan.md#catalog-deletion-safety-2026-11-01-source-only),
[Product collaboration contract](../../crates/modules/rustok-product/docs/product-lifecycle-collaboration.md#catalog-deletion-retention-2026-11-01-source-only)
and [source packet](../../crates/modules/rustok-inventory/contracts/evidence/catalog-deletion-source.json)
separate source checks from compilation, backend behavior, migration rollout and runtime proof.
Read-only source entrypoints (do not import/run regression companions):

```bash
node scripts/verify/verify-inventory-catalog-deletion.mjs
node scripts/verify/verify-product-lifecycle-collaboration.mjs
node scripts/verify/verify-product-catalog-schema.mjs
node scripts/verify/verify-commerce-admin-product-route-error-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
node scripts/verify/verify-ecommerce-fba-registries.mjs
node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs
```

Maintainer-only regression commands, **not executed by this continuation**:

```bash
cargo test --locked -p rustok-inventory --lib services::bootstrap
cargo test --locked -p rustok-product --lib inventory_deletion
cargo test --locked -p rustok-commerce --lib deletion
cargo test --locked -p rustok-commerce --test catalog_service_test inventory_catalog_deletion
cargo test --locked -p rustok-commerce --test product_event_index_integration_test
node --test scripts/verify/verify-inventory-catalog-deletion.test.mjs
```

There are 31 new Rust/10 Node definitions, all uncompiled/unrun. Real PostgreSQL READ COMMITTED
contention/isolation refusal, MySQL current reads, SQLite conflicts/rollback, receipt replay,
actual HTTP/GraphQL/mounted clients and I026 up/down/query-plan evidence remain gates. Normal
I026 index creation may block writers; use a controlled rollout window and retained backups.
Existing migrations/receipt identities are unchanged. Native deletion is not arbitrary-SQL or
historic-downgrade safety, history purge, stock disposal or a general retirement workflow.
Leptos remaining Product CRUD still uses generic GraphQL gateway/redaction and non-durable
retained keys, not general native CRUD; full actionable presentation/recovery parity remains
incomplete. No destructive SKU UI is enabled. Inventory Index revisions/baseline/tombstones/
all-writer publication/source/selected bridge/worker and Product step 8 are still open.

## Inventory quantity writer prerequisite (2026-11-01; source only)

The [owner plan](../../crates/modules/rustok-inventory/docs/implementation-plan.md#quantity-writer-prerequisite-2026-11-01-source-only)
and [source packet](../../crates/modules/rustok-inventory/contracts/evidence/quantity-writes-source.json)
cover the real native set/adjust and Product bootstrap quantity writer, not a generic Index source.

Direct read-only source check (does not import test callbacks):

```sh
node scripts/verify/verify-inventory-quantity-writes.mjs
```

Maintainer-only commands below are recorded, **not executed by the agent**:

```sh
cargo check -p rustok-inventory -p rustok-product -p rustok-commerce --all-targets
cargo test -p rustok-inventory services::quantity::tests
cargo test -p rustok-product quantity_errors
cargo test -p rustok-commerce --test catalog_service_test inventory_quantity_writes
cargo test -p rustok-commerce --test inventory_service_test
cargo test -p rustok-commerce --lib quantity_errors
node --test scripts/verify/verify-inventory-quantity-writes.test.mjs
node --test scripts/verify/verify-inventory-admin-boundary.test.mjs
```

33 Rust/10 Node new quantity definitions plus the existing migrated regressions remain unrun.
All SQL, isolation/locking/phantom/ignored-effect/rollback and receipt cases are source definitions,
not backend evidence. No historical migration or event digest changes. Native quantity writes are
not idempotent, existing Inventory generic client redaction remains, and no client recovery/parity
or readiness status is promoted. Explicit installation baseline/revisions/tombstones/all-writer
publication/selected Distribution bridge/Index readiness and Product step 8 remain incomplete.

This slice changes internal Rust errors and finite existing error envelopes, not a GraphQL field,
HTTP route, external DTO shape or event schema; reference schema artifacts are not regenerated.
The canonical root-event digest is byte-preserved. This is not canonical digest generation or
runtime envelope/transport certification.

Executed source checks: 19/23 direct guards passed; the four nonzero results are byte-identical
to the reconstructed dirty task-entry baseline (`inventory-availability-quantity-context` 2,
`inventory-port-diagnostic-safety` 2, `ecommerce-public-port-error-safety-v2` 9,
`product-catalog-receipts` 1). Rust syntax/format (32 files), Node syntax (four, no companion
imports), JSON parse (four), Prettier (eight) and scoped links/whitespace passed. The remediation
gate passed diff invariants but architecture metadata and clippy remain Cargo-blocked. See the
packet for exact command/output identities; none of these checks executes the 33/10 regressions.

## Inventory source-history owner continuation (2026-11-01; source only)

Read-only: `node scripts/verify/verify-inventory-source-history.mjs` or
`npm run verify:inventory:source-history`. The separate companion command
`npm run test:verify:inventory:source-history` executes eight Node definitions and is maintainer-run.
The [source packet](../../crates/modules/rustok-inventory/contracts/evidence/source-history-source.json)
records 18 new Rust definitions (six owner pure, one isolated physical-plan storage definition and eleven Commerce persistence/caller definitions),
all uncompiled/unrun. Maintainer commands: `cargo test -p rustok-inventory source_history::`,
`cargo test -p rustok-commerce --test catalog_service_test inventory_source_history::`.

I027 creates four empty owner tables and refuses downgrade while any has retained state. It
initializes no installation and touches no historical migration payload. Apply it before matching
binaries because actual writes now check owner tracking metadata even before explicit opt-in.
Controlled PostgreSQL READ COMMITTED/SQLite rollout, backups, advisory/snapshot contention,
page interruption/replay, ignored exact effects, commit uncertainty, cleanup and downgrade evidence
remain maintainer-run. Native baseline commands enforce the current context budget using the
existing core timeout helper; rollback is awaited, and commit timeout is an unknown outcome.
Baseline identities are creator-actor bound; automatic reassignment/reset/abort is not provided.

The quantity packet's old migration-registry hash remains historical evidence, not a ban on
adding governed migrations to executable `mod.rs`. Its immutable payload/shared-helper/event
checks remain pinned; the current source-history guard checks I027 and the real fixture registry.
No wire/GraphQL/HTTP schema changed, so no such generated artifacts were edited; the unchanged
canonical event digest is not a generation/runtime certification. Native actionable error/client
parity, canonical versioned publication/relay and selected generic Index integration remain open.

## Inventory canonical source delivery (2026-11-02; source only)

`node scripts/verify/verify-inventory-source-delivery.mjs` is read-only source evidence, not a test
or Rust/SQL/runtime proof. The separate `.test.mjs` companion has ten opt-in maintainer-run
definitions, not executed here. [Evidence](../../crates/modules/rustok-inventory/contracts/evidence/source-delivery-source.json)
lists 26 new Rust definitions for sealed schema, real owner callers, canonical storage/relay and
host memory/fan-out; existing physical/source-error definitions are strengthened too.

Current actor and retained UUID/version share the owner transaction. Fresh final activation
shares receipt/checkpoint/active transition; staged seeds emit no ready facts. Required Inventory
handling refuses unmatched broker-only success; Product remote-only routing stays supported.
Transport doubles do not prove Iggy cursor behavior. No tests, compilation, SQL, migrations or
runtime run. Architecture guard cannot obtain Cargo metadata: cargo is absent. Formatting,
syntax, arity and markers do not prove typing/digest generation/backend/concurrency/durable replay/
current-authority Distribution/Index/persist-before-ACK. Historical migration/root/reservation
payloads and digest are pinned. Earlier active baselines need explicit operator reconciliation,
not synthetic status/journal facts. No live baseline before real handlers plus operator recovery;
no Index or Product step-8 promotion.

## Selected Inventory Index consumer and targeted review (2026-11-02; source only)

The [selected Distribution contract](../../crates/modules/rustok-distribution/docs/inventory-index.md)
and [source/review packet](../../crates/modules/rustok-inventory/contracts/evidence/source-index-consumer-source.json)
cover native Inventory history, Auth-owned current service contexts, canonical Secrets fresh
resolution, generic durable refresh and original-identity replay. The targeted review fixes
a stale-cache-fill race, orphan-checkpoint/unproven completion, PostgreSQL inbox schema decoding
and stale regression table/column names. Actor mapping is checked against and delegates to
canonical `AuthContext::port_actor()`; its intentional service `client_id` identity is retained. It does not implement
backoff or original failed/cancelled-job operator recovery.

Direct read-only source checks, not tests or typing/backend proof:

```sh
node scripts/verify/verify-inventory-index-consumer.mjs
node scripts/verify/verify-index-source-refresh-event.mjs
node scripts/verify/verify-index-mutation-storage.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-replay-runtime-composition.mjs
```

Maintainer-only commands below are recorded, **not executed by the agent**:

```sh
cargo check -p rustok-secrets -p rustok-auth -p rustok-index -p rustok-distribution -p rustok-server --all-targets
cargo test -p rustok-secrets --lib
cargo test -p rustok-server worker_service_context::tests
cargo test -p rustok-distribution inventory_index::tests
cargo test -p rustok-index source_replay_job_tests
cargo test -p rustok-index source_replay_scheduler
cargo test -p rustok-index source_refresh_event_tests
node --test scripts/verify/verify-inventory-index-consumer.test.mjs
```

Two ignored PostgreSQL definitions require an explicitly isolated database supplied through
`RUSTOK_INVENTORY_INDEX_DATABASE_URL`, with schema-creation permission. They cover empty baseline
activation and changed/duplicate/tombstone delivery through the selected bridge and actual Index
storage, but use a test-only authority port: they do not certify real OAuth or deployment grants.
With that database deliberately configured, the maintainer can run:

```sh
cargo test -p rustok-distribution inventory_index::tests::postgres_ -- --ignored
```

Executed read-only review checks: Rust syntax/Scalar format for nine files, Node syntax for two,
JSON parsing, Prettier for thirteen files, scoped whitespace/diff and new local link targets.
Twenty-eight of thirty-eight source guards pass; the other ten match the immutable review entry
(nine byte-identical, one requiring only snapshot-root normalization in a Product Node stack).
The remediation gate passes diff invariants but cannot obtain Cargo metadata because `cargo`
is absent; clippy is not reached. These results do not establish typing or behavior.

The review adds seven Rust/five Node definitions without execution. Secrets regression sources
retain tenant policy, other consumers' cache behavior and an overlapping older cache fill.
Index definitions retain original identities/cursors and refuse both missing and incomplete
completion proofs. Compilation, real credentials/rotation/revocation, PostgreSQL locking,
concurrency, restart, broker ACK, canonical digest admission and full operator-client recovery
remain gates. No live source/Index/Product step-8 readiness is inferred from parsing or markers.
