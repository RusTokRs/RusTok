# Implementation plan for `rustok-region`

## Scope of work

- preserve region-owned country, currency, and tax-provider policy;
- maintain module-owned admin and storefront UI boundaries;
- collect runtime evidence before FBA status promotion.

## Current state

`rustok-region` owns regions, countries, currencies, and the baseline regional
tax-provider policy. `region.tax_provider_id` is the canonical provider field;
an explicit channel override map is supported only for channel-aware cart
runtime. This module does not own tenant locale policy or the full tax domain.

Both admin and storefront packages are module-owned. Native adapters use
`HostRuntimeContext`; storefront preserves its selected GraphQL path. Core owns
route/query, presentation, validation, and typed error evidence, while UI
adapters bind and render prepared state.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `RegionReadPort` / `region.read_projection.v1` in
  `crates/modules/rustok-region/contracts/region-fba-registry.json`.
- Owner read-port error safety: `source_closed_unvalidated`; public validation
  messages and owner diagnostics are bounded by
  `crates/modules/rustok-region/docs/region-owner-port-error-safety.md` and
  `scripts/verify/verify-region-owner-port-error-safety.mjs` without promoting
  FFA/FBA status.
- Static and runtime-order evidence:
  `crates/modules/rustok-region/contracts/evidence/region-contract-test-static-matrix.json`
  and `crates/modules/rustok-region/contracts/evidence/region-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-region-admin-boundary.mjs`,
  `scripts/verify/verify-region-storefront-boundary.mjs`, and
  `npm run verify:region:fba` lock the owner UI boundaries and read-provider
  order.
- The admin surface is an accepted single-adapter owner fragment: it is an
  authenticated operator workflow with no public/headless region-admin
  contract, so its native `#[server]` adapter is intentional and no
  package-local GraphQL fallback is added.

## Open results

1. **Collect live RegionReadPort and storefront transport evidence.** Exercise
   shared-context reads plus native success/failure, GraphQL success, and
   double-failure error envelopes before FBA promotion.
   **Depends on:** composed region provider, storefront host, and test tenants.
   **Done when:** tenant scope, locale fallback, typed errors, DOM evidence, and
   selected-path parity are reproducible in runtime tests.

2. **Evolve regional policy through the owner service.** Add country/currency
   rules and tax-provider behavior only as module-owned policy, preserving the
   boundary between baseline region data, explicit channel overrides, and the
   tax calculation domain.
   **Depends on:** region requirements, tax provider contracts, and commerce
   store-context consumers.
   **Done when:** policy selection is deterministic, tested for edge cases, and
   cart/order consumers retain the selected provider snapshot.

3. **Keep module and commerce documentation synchronized.** Update local,
   admin, storefront, manifest, and umbrella commerce docs with a region or
   transport contract change.
   **Depends on:** the change-owning regional or commerce contract.
   **Done when:** route ownership, fallback behavior, and tax-provider policy
   describe the same module boundary everywhere.

4. **Validate the Region owner-port error-safety source contract.** Execute the
   focused verifier, compile the Region and Commerce consumers, and retain
   mounted failure-envelope evidence without exposing raw owner payloads.
   **Depends on:** repository checkout and composed Region consumers.
   **Done when:** source guard, compile, and mounted runtime evidence are retained
   without changing the current FFA/FBA status prematurely.

## Scaffold reference candidate

The Region create/read slice is the candidate with owner storage and no Fly
contribution. Its [file map](README.md#region-command-and-query-file-map)
documents the service split. The SQLite and PostgreSQL create/read tests apply
the owner migration list and verify country resolution, locale fallback, and
tenant isolation. A SQLite fault test inserts a journal entry, injects a
readback failure, and verifies that the entire update rolls back.

Cart tax recalculation now reads `RegionTaxPolicy` through the native
`RegionService::read_tax_policy_in_tx` entrypoint. The private Region mappings
live inside this crate. Cart and Commerce fixtures apply the owner migrations
and create Regions through the service. The
[ownership decision](../../../../DECISIONS/2026-11-04-region-storage-and-cart-tax-policy.md)
records the transaction and deployment profile. Complete the remaining layout
checks and verify the declared admin and storefront clients before enrolling
the complete module.

### Remaining reference gates

The retained `xtask` CLI recheck on 2026-11-05 still rejects the complete
Region module. Continue in this order without promoting the reference status:

1. Move the existing ports and translation entrypoints from the backend source
   root into their canonical responsibility slots. Migrate every caller and
   source-verifier path together, preserving the owner API and transaction rules.
2. Split the 56,186-byte admin core by its existing responsibilities and place
   admin/storefront contract tests in the canonical `src/tests/` slots. Clarify
   the documented single-adapter transport decision against the lint contract.
3. Verify the functional Next.js admin and storefront packages and declare the
   corresponding manifest bindings. Retain route, locale, authorization, and
   interaction parity with Leptos before advertising those bindings.
4. Run the complete module lint and the applicable backend, transport, and client
   evidence gates. Mounted FFA/FBA evidence remains separate from storage tests.

## Verification

- `node scripts/verify/verify-region-owner-port-error-safety.mjs`
- `npm run verify:region:admin-boundary`
- `npm run verify:region:storefront-boundary`
- `npm run verify:region:fba`
- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo test -p rustok-region --lib --test region_service_test --test module`
- `cargo test -p rustok-region --test region_service_test postgres_create_and_resolve_region_by_country -- --ignored`
  runs the same create/read scenario in a fresh database. Set
  `RUSTOK_REGION_TEST_POSTGRES_URL` to a test PostgreSQL instance with database
  creation permission. The fixture removes the database after the run. The
  Region PostgreSQL workflow runs the same crate-scoped scenario.
- Targeted region lookup, country/currency policy, tax-baseline, and storefront
  transport tests.

On 2026-11-04, on local `main` at `111d6dd4bf` with the Region slice changes
applied, Cargo passed five crate and module contract tests and ten SQLite
tests. The Cargo-built integration test binary passed the same create/read
scenario on PostgreSQL 16.13. The Region FBA and owner-port source guards
passed. The Commerce GraphQL error-safety verifier still expects an earlier
adapter shape and needs separate reconciliation with Commerce's current code.

The native tax-policy continuation was rechecked on 2026-11-05 from local
`main` at `f465936127921b0f57b2165afead3456c409a7ff`:

- `cargo test -p rustok-region --locked --offline --lib --test region_service_test --test module`
  passed 19 tests. The PostgreSQL test is opt-in and was ignored in that command.
- The freshly Cargo-built integration binary passed
  `postgres_create_and_resolve_region_by_country --ignored --exact` on
  PostgreSQL 16.13. The test creates and removes an isolated database. The same
  scenario now checks the tax projection, foreign/missing regions, channel
  selection, reads of uncommitted data through the supplied transaction, an
  empty country-rule set, rollback, and propagated storage errors.
- `npm run verify:region:fba`, the Region owner-port error-safety verifier,
  and `python scripts/architecture_dependency_guard.py` passed.
- The retained `target/debug/xtask.exe module lint region` failed on the
  reference gates above. No full-module or client readiness was promoted.
- Commerce's `pricing_service_test --no-run` target is blocked by its import
  of private Pricing entities and three Product constructors missing `brands`.
  Its older fixture tests remain unverified. The
  [Cart record](../../rustok-cart/docs/implementation-plan.md#native-region-policy-verification-2026-11-05)
  retains the successful Cart rollback and optional-schema evidence separately.
- `cargo test -p rustok-commerce --locked --offline --test context_service_test`
  passed five tests against the shared fixture applying Region migrations.
  The explicit-region test now checks the owner port's `region.not_found`
  boundary for both missing and foreign identities. Production error behavior
  is unchanged. Existing unrelated Commerce warnings remain.

The earlier successful remote refresh matched the recorded base. The final
pre-commit refresh failed with an empty server reply and then DNS resolution
failure for GitHub. Local `main` and the last fetched `origin/main` still
matched that base before the scoped local commit. No remote integration or push
was performed; refresh remote history before subsequent integration.

## Update rules

1. Keep regional policy and region read projections in this module.
2. Update local docs, `rustok-module.toml`, and commerce/tax documentation with
   a regional or transport contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
