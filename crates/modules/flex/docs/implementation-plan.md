# Implementation plan for `flex`

## Current state

`flex` is a capability-only custom-fields module, not a donor-persistence owner or a separate
business domain. Attached mode extends explicit donor contracts; standalone mode owns schemas and
entries. Current attached donors are `user`, `product`, `order` and `topic`; donors retain their
business tables and write paths.

`forum.topic` remains an intentional attached donor. Optional tenant-defined fields may extend a
topic, but Forum-critical state such as lifecycle/status, category binding, content, route identity,
moderation, counters, accepted-solution semantics and access policy stays normalized and Forum-owned.

The accepted next shared consumer is `taxonomy.category`, after `rustok-taxonomy` gains canonical
Category identity/hierarchy/localized presentation ownership. Flex will provide only administrator-
defined extension fields for categories; Taxonomy built-ins stay normalized owner data.

Owner-owned contracts live in `flex::graphql`, `flex::registry`, `flex::rest` and
`flex::standalone`. The server composes `FlexGraphqlRuntime`, SeaORM, registry/cache adapters and
Axum REST handlers only. Localized attached and standalone values use parallel storage; inline
localized JSON is not a canonical runtime fallback.

Localized authoring accepts only a valid normalized locale and starts from its exact row for both
attached and standalone updates. Presentation fallback is confined to explicit read resolution and
never becomes input to an authoring write.

## Platform donor rule

Flex support is explicit product opt-in. A metadata/JSON column is not an extension contract by
itself. A new module should need only a bounded registration/storage adapter, permissions and its
owner write/read integration. It must not rebuild field definitions, type validation, localized
attached values, cache invalidation, generic transport or schema-builder behavior.

The target onboarding contract is intentionally small:

```text
entity registration
  + tenant/entity identity
  + owner payload/storage adapter
  + permissions
  = Flex definitions + values + validation + localization + generic admin rendering
```

If onboarding a donor requires a second module-specific custom-field engine, Flex has failed its
platform responsibility and the common capability must be improved instead.

Critical domain invariants remain normalized even on Flex-enabled donors. Flex must never become the
source of truth for price, SKU, payment/inventory/ledger state, route identity, moderation lifecycle
or other owner-critical fields.

## Cache convergence

The field-definition cache is byte-weighted and keeps the local EventBus consumer as a low-latency
exact-invalidation path. Durable convergence is source-complete:

- `flex_field_definition_cache_generation` is a singleton database generation;
- transaction-local database triggers advance it for every INSERT/UPDATE/DELETE on
  `user_field_definitions`, `product_field_definitions`, `order_field_definitions` and
  `topic_field_definitions`, including reorder and soft-delete updates;
- Flex owns `m20260716_000000_create_field_definition_cache_generation`; every owner trigger
  migration explicitly depends on it, so the shared generation exists before owner triggers and
  reverse rollback removes triggers before the singleton table/function;
- every serving runtime reads the durable generation, clears the complete cache before marking the
  generation applied, polls every five seconds and repeats the clear on advancement;
- database read failure or generation regression clears the process cache, leaves readiness failed,
  terminates the worker iteration and relies on the supervisor to retry without lowering the applied
  generation;
- task liveness and durable recovery readiness are separate: repeated initialization preserves a
  live degraded supervisor while the critical runtime guardrail checks `is_ready()`;
- the process-local consumer remains restartable/abort-on-drop and full-clears on local lag.

Source evidence includes the four-definition-table cache matrix. Adding `taxonomy.category` must
extend this mechanism through the common donor contract rather than introducing Taxonomy-specific
cache invalidation for Flex definitions.

This cache evidence is source-complete but is not compiled or database verified until the permanent
cache workflow passes its compiled and PostgreSQL jobs on one revision.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- Capability runtime is manifest-composed through `FlexModule` and `[provides.graphql]`; it has no
  donor-specific module-owned UI or FBA provider port.
- `node scripts/verify/verify-flex-multilingual-contract.mjs` locks the multilingual storage and
  owner-boundary contract.

## Open results

1. **Make donor onboarding a minimal reusable capability.** Reduce donor-specific plumbing so a new
   entity can opt in through one bounded registration/storage contract instead of copying field-
   definition services, adapters, event/cache plumbing and admin rendering. Existing Topic support is
   retained and should converge onto that smaller adapter rather than being removed.
   **Depends on:** the existing registry/GraphQL/runtime contracts and the next real consumer.
   **Done when:** `taxonomy.category` can opt in without implementing a parallel custom-field stack,
   existing donors remain compatible, and a guardrail rejects module-local replacement engines.

2. **Add `taxonomy.category` as the reference shared donor.** After Taxonomy owns Category
   identity/hierarchy/localized copy/presentation, attach administrator-defined category fields
   through Flex and the generic schema-builder path.
   **Depends on:** the accepted Taxonomy Category migration plan and Taxonomy owner storage.
   **Done when:** category custom fields support shared and localized values, tenant isolation,
   validation and generic admin authoring while built-in category fields remain Taxonomy-owned.

3. **Preserve the Topic extension boundary.** Keep `topic` attached fields as optional extension data
   and prevent Flex schemas from becoming a substitute for Forum normalized state.
   **Depends on:** Forum Topic write/read adapters and the common donor contract.
   **Done when:** Topic custom fields roundtrip through the same generic Flex semantics as other
   donors, critical Forum fields are neither writable nor shadowable through Flex, and no Forum-only
   custom-field engine exists.

4. **Execute durable field-cache recovery evidence.** Run the source-complete SQLite owner matrix,
   PostgreSQL transaction/concurrency/replay test and two-replica server outage/regression recovery
   test on one reconciled `main` revision, then fix every format, compile, test or Clippy failure.
   **Depends on:** the permanent cache workflow or another Rust 1.96 environment with PostgreSQL 17.
   **Done when:** compiled and PostgreSQL jobs pass on the same revision and the result is recorded
   without copying raw logs.

5. **Finish the owner transport extraction with targeted runtime evidence.** Remove remaining
   server Flex artifacts beyond Axum handler extraction, SeaORM/bootstrap adapters and runtime
   composition; run targeted owner-root GraphQL/REST tests when compilation is available.
   **Depends on:** host-composed `FlexGraphqlRuntime` and targeted test fixtures.
   **Done when:** server holds only the allowed adapters and owner-owned roots execute with
   persistence, RBAC, errors, events and cache invalidation.

6. **Close attached and standalone migration and exact-authoring verification.** Verify localized
   value backfill/cleanup, PATCH merges, tenant scoping, schema validation, donor read/write paths
   and standalone schema/entry roundtrips against production persistence.
   **Depends on:** donor migrations, standalone SeaORM adapter and compiled integration fixtures.
   **Done when:** no runtime reads inline localized payload as canonical, a localized update never
   copies another locale into its target, all live donors retain their data and standalone
   integration tests are stable.

7. **Evolve advanced Flex capability only for demonstrated product needs.** Add future types such as
   Media/reference/rich-text only through the common Flex contract and only with explicit ownership,
   governance, permissions, indexing and documentation decisions.
   **Depends on:** a concrete product requirement and capability review.
   **Done when:** new behavior cannot be mistaken for a replacement of a normalized domain module
   or a shared donor-persistence layer.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- `cargo check -p flex --lib`
- `cargo check -p rustok-auth --lib`
- `cargo check -p rustok-product --lib`
- `cargo check -p rustok-commerce --lib`
- `cargo check -p rustok-forum --lib`
- `cargo check -p rustok-server --lib`
- `cargo test -p flex cache_generation --lib`
- `cargo test -p rustok-server field_definition_registry_bootstrap --lib`
- `cargo test -p rustok-server field_definition_cache_generation --lib`
- `cargo test -p rustok-server --test field_definition_cache_generation_guard`
- `RUSTOK_FLEX_TEST_POSTGRES_URL=postgres://... cargo test -p flex --test postgres_cache_generation -- --ignored --nocapture --test-threads=1`

## References

- [Taxonomy Category + Flex platform plan](../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache capability implementation plan](../../rustok-cache/docs/implementation-plan.md)

## Change rules

1. Flex support is explicit product opt-in; never infer it from a metadata column alone.
2. Keep donor business persistence and attachment relations with their owning module unless a
   generic Flex value store is explicitly the accepted attached-value owner.
3. Keep reusable generation/trigger helpers and Flex contracts in this crate; owner migrations
   install triggers on their own definition tables.
4. Do not create a module-local custom-field definition/validation/localization/transport engine.
   Improve Flex when a donor needs reusable behavior.
5. Flex fields may extend a donor but must not replace normalized owner invariants.
6. Keep server work to composition, persistence adapters, reconciliation and HTTP handler
   extraction.
7. Update the canonical Flex README, manifest, donor docs and central module documentation with a
   capability contract change.
