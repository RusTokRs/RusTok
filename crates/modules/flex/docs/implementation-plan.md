# Implementation plan for `flex`

## Current state

`flex` is a capability-only custom-fields module, not a donor-persistence owner or a separate
business domain. Attached mode extends explicit donor contracts; standalone mode owns schemas and
entries. Registered attached donors remain `user`, `product`, `order` and `topic`; `taxonomy.category`
is now the reference generic attached donor and uses the shared Flex definition/value/localization
boundary rather than a Category-specific custom-field engine.

`forum.topic` remains an intentional attached donor. Optional tenant-defined fields may extend a
topic, but Forum-critical state such as lifecycle/status, category binding, content, route identity,
moderation, counters, accepted-solution semantics and access policy stays normalized and Forum-owned.

Taxonomy owns `taxonomy.category` identity, hierarchy, built-in localized presentation and aggregate
lifecycle. Flex owns only administrator-defined extension fields plus the Translation projection of
those extension fields. The attached Translation owner foundation provides exact source/target
snapshots, revision-safe/idempotent apply, aggregate progress, durable PostgreSQL per-resource state,
an ordered bounded ChangeCursor, schema-change fan-out and same-transaction hard-delete tombstones.
Snapshot `resource_revision` is the Flex-owned durable `attached:N` token. Taxonomy aggregate
revision remains a serialization/CAS mechanism for owner mutation; it is not a second Translation
resource revision.

`flex/schema_copy` is already a registered Translation target. The neutral
`flex/attached_localized_value` provider is now also registered for the `taxonomy.category` donor
when `mod-flex + mod-taxonomy` are composed. It exposes list/read/validate/apply, aggregate progress
and the bounded Flex ChangeCursor over the same owner contracts and durable `attached:N` revision.
Attached dynamic fields now have a separate Flex-owned classification / AI-export policy plane keyed
by `(tenant_id, entity_type, field_key)`. Translation enriches live field descriptors through that
policy without changing content revisions or ChangeCursor semantics. Missing policy remains
`tenant_private + ai_export_allowed=false`, and secret/immutable-transaction fields cannot be made
AI-exportable. The owner-owned GraphQL surface exposes effective policy plus explicit/default
provenance and RBAC-guarded set/reset mutations through the same Flex policy API. Production/pilot
promotion remains an evidence gate, not another provider or revision implementation step.

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

## Attached Translation revision and change contract

The attached Translation projection has one resource-revision source of truth:

- `flex_attached_translation_resource_state` owns the durable per-resource monotonic revision;
- active snapshots and active ChangeCursor rows expose the same `attached:N` token;
- exact localized-value writes and Translation-relevant schema mutations feed the same owner state;
- one database transaction advances one resource at most once, even when several rows or OLD/NEW
  schema fan-out paths are involved;
- field-definition UPDATE/DELETE captures OLD affected resources before FK cascades and NEW affected
  resources after cascades;
- migration backfill establishes current state without fabricating historical change events;
- Category hard delete emits the final same-transaction `deleted` tombstone and removes active state;
- list/read snapshot composition reads schema, donor existence, localized values and durable
  `attached:N` evidence from one PostgreSQL repeatable-read snapshot;
- aggregate progress reads those same durable resource revisions in one PostgreSQL repeatable-read
  snapshot and fails closed outside PostgreSQL rather than synthesizing another revision source;
- apply keeps the schema-generation barrier plus Taxonomy owner lock, performs the owner mutation,
  then observes the Flex-owned revision inside the same write transaction;
- the registered neutral provider delegates all read/apply/progress/change behavior to those owner
  contracts and does not reconstruct state in the host;
- no hash derived from Taxonomy revision/schema, host-side counter, polling reconstruction or dual
  revision/provider path is allowed.

The durable journal migration is `ExpandContract / PreActivation`. Revision cutover and provider
activation are complete at source level; the remaining gate is retained PostgreSQL/concurrency/
recovery evidence on the registered provider.

## Attached field governance policy

Classification and AI-export admission are a separate governance plane from donor definitions and
translated content:

- `flex_attached_field_policies` is keyed by tenant + namespaced entity type + stable field key;
- there is deliberately no FK to `flex_attached_field_definitions`, so legacy donors with their own
  definition tables can converge on the same policy contract;
- `FlexDataClassification` is typed in Flex and maps exhaustively to the neutral Translation
  classification contract;
- every policy resolver returns a value for every requested field; an absent row resolves to
  `tenant_private` with AI export denied;
- explicit `ai_export_allowed=true` is rejected for `secret` and `immutable_transaction`;
- reads validate explicit policy before it can influence a Translation descriptor;
- the database-backed store exposes bounded resolve/upsert/delete APIs so owner/admin adapters do not
  bypass the policy contract with direct SQL;
- effective policy resolution carries explicit/default provenance so admin tooling can distinguish a
  reviewed row from the fail-closed fallback;
- the owner-owned GraphQL query lists policy only for field keys supplied by the registered donor
  definition service, while set/reset mutations require `flex_schemas:update` and validate the target
  field against that same registry before changing policy;
- the registered attached Translation provider is decorated at host composition with the policy
  resolver; list/progress/change and donor apply semantics remain delegated to the existing provider;
- policy changes do not manufacture content revisions or ChangeCursor entries. Consumers must treat
  classification/export admission as live governance metadata and re-read it at export time.

No permissive seed/backfill is created. Existing dynamic fields therefore remain fail-closed until an
owner explicitly writes reviewed policy.

## Cache convergence

The field-definition cache is byte-weighted and keeps the local EventBus consumer as a low-latency
exact-invalidation path. Durable convergence is source-complete:

- `flex_field_definition_cache_generation` is a singleton database generation;
- transaction-local database triggers advance it for every INSERT/UPDATE/DELETE on the registered
  definition stores, including generic attached definitions used by `taxonomy.category`;
- Flex owns `m20260716_000000_create_field_definition_cache_generation`; owner trigger migrations
  depend on the shared generation before installing their triggers;
- every serving runtime reads the durable generation, clears the complete cache before marking the
  generation applied, polls every five seconds and repeats the clear on advancement;
- database read failure or generation regression clears the process cache, leaves readiness failed,
  terminates the worker iteration and relies on the supervisor to retry without lowering the applied
  generation;
- task liveness and durable recovery readiness are separate: repeated initialization preserves a
  live degraded supervisor while the critical runtime guardrail checks `is_ready()`;
- the process-local consumer remains restartable/abort-on-drop and full-clears on local lag.

The cache evidence is source-complete but is not compiled or database verified until the permanent
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

1. **Collect retained evidence for the registered `taxonomy.category` attached Translation provider.**
   Exercise the already-registered neutral provider against PostgreSQL without introducing a test-only
   owner path.
   **Depends on:** the merged durable ChangeCursor/state, `attached:N` snapshot cutover and provider
   composition.
   **Done when:** retained evidence covers migration/backfill, concurrent resource/source/target CAS,
   idempotent replay, aggregate-progress stability, schema OLD/NEW fan-out, hard-delete tombstones and
   bounded change-cursor recovery on one revision. Until then readiness remains `blocked` even though
   provider status is `registered`.

2. **Verify the reusable attached-field classification/export policy before enabling machine translation.**
   Source implementation is present: typed Flex policy, fail-closed resolution with provenance, safe
   persistence mutation API, owner-owned RBAC GraphQL query/set/reset surface, persistence migration,
   and live Translation descriptor decoration are wired without donor-specific hard-coding. No
   permissive policy is seeded.
   **Depends on:** review/compile/database and GraphQL RBAC evidence for the reusable Flex policy rather
   than a donor-specific field-definition change.
   **Done when:** retained evidence proves an explicit safe policy can admit AI export, forbidden
   classifications cannot, missing/reset policy remains fail-closed and visibly non-explicit,
   unauthorized policy mutation is rejected, unknown donor field keys cannot be configured, and
   `taxonomy.category` composition reads the same policy plane without changing content
   revision/ChangeCursor behavior.

3. **Reduce remaining donor onboarding plumbing using `taxonomy.category` as the reference.** Existing
   Topic and older donor adapters should converge on the generic attached definition/value contract
   where behavior is reusable instead of preserving donor-specific service stacks.
   **Depends on:** the active generic Category donor and existing registry/GraphQL/runtime contracts.
   **Done when:** a new demonstrated donor can opt in through one bounded adapter without copying
   field-definition, localization, change/revision or transport machinery.

4. **Preserve the Topic extension boundary.** Keep `topic` attached fields as optional extension data
   and prevent Flex schemas from becoming a substitute for Forum normalized state.
   **Depends on:** Forum Topic write/read adapters and the common donor contract.
   **Done when:** Topic custom fields roundtrip through the same generic Flex semantics as other
   donors, critical Forum fields are neither writable nor shadowable through Flex, and no Forum-only
   custom-field engine exists.

5. **Close standalone Translation exact-owner parity.** Standalone Flex localized values must expose
   the same owner-safe exact source/target, revision, progress and bounded-change semantics before a
   standalone localized-value Translation target can be activated.
   **Depends on:** canonical standalone parallel localized storage and owner service.
   **Done when:** standalone authoring never seeds from fallback/default locale, arbitrary JSON is not
   exposed as copy, and the provider uses one durable owner revision/change contract.

6. **Execute durable field-cache recovery evidence.** Run the source-complete SQLite owner matrix,
   PostgreSQL transaction/concurrency/replay test and two-replica server outage/regression recovery
   test on one reconciled `main` revision, then fix every format, compile, test or Clippy failure.
   **Depends on:** the permanent cache workflow or another Rust 1.96 environment with PostgreSQL 17.
   **Done when:** compiled and PostgreSQL jobs pass on the same revision and the result is recorded
   without copying raw logs.

7. **Finish the owner transport extraction with targeted runtime evidence.** Remove remaining
   server Flex artifacts beyond allowed SeaORM/bootstrap/composition adapters and run targeted
   owner-root GraphQL/REST tests when compilation evidence is available.
   **Depends on:** host-composed `FlexGraphqlRuntime` and targeted test fixtures.
   **Done when:** server holds only allowed adapters and owner-owned roots execute with persistence,
   RBAC, errors, events and cache invalidation.

8. **Close attached and standalone migration and exact-authoring verification.** Verify localized
   value backfill/cleanup, PATCH merges, tenant scoping, schema validation, donor read/write paths,
   attached Translation revision/change behavior and standalone schema/entry roundtrips against
   production persistence.
   **Depends on:** donor migrations, standalone SeaORM adapter and compiled integration fixtures.
   **Done when:** no runtime reads inline localized payload as canonical, a localized update never
   copies another locale into its target, all live donors retain their data and integration evidence
   is stable.

9. **Evolve advanced Flex capability only for demonstrated product needs.** Add future types such as
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

The repository owner runs tests for the current attached Translation rollout. A PR that intentionally
skips local test execution must still complete source-level policy, diff, ownership and migration
review before merge and must not claim runtime evidence that was not executed.

## References

- [Translation implementation plan](../../../docs/modules/translation-implementation-plan.md)
- [Taxonomy Category + Flex platform plan](../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- [Taxonomy/Flex ownership ADR](../../../DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md)
- [Host cache contract inventory](../../rustok-cache/docs/host-cache-inventory.md)
- [Cache capability implementation plan](../../rustok-cache/docs/implementation-plan.md)

## Change rules

1. Flex support is explicit product opt-in; never infer it from a metadata column alone.
2. Keep donor business persistence and attachment relations with their owning module unless a
   generic Flex value store is explicitly the accepted attached-value owner.
3. Keep reusable generation/trigger/revision/change/provider helpers and Flex contracts in this crate;
   owner modules contribute only canonical identity/lifecycle and bounded adapters.
4. Do not create a module-local custom-field definition/validation/localization/transport engine.
   Improve Flex when a donor needs reusable behavior.
5. Flex fields may extend a donor but must not replace normalized owner invariants.
6. Keep server work to composition and concrete persistence/runtime adapters; do not reconstruct Flex
   revision/change semantics in the host.
7. Dynamic attached-field AI export stays denied unless an explicit valid Flex policy allows it;
   absence and invalid policy are never permissive fallbacks.
8. Update the canonical Flex README, implementation plan, donor docs and relevant central ownership
   documentation with a capability contract change.
9. Before completion, remove superseded names/paths and verify that no deprecated or parallel
   internal contract remains in the touched scope.