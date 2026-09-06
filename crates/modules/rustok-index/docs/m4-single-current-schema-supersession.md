# M4 single-current schema supersession

Status: `source_complete_execution_pending`.

## Purpose

Index schema contracts are immutable at one exact `SchemaRef`: reusing the same numeric routing key
with a different fingerprint/JSON contract fails closed. That rule prevents silent schema drift, but a
consumer eventually needs a safe way to replace one current contract with another without keeping two
runtime compatibility branches active.

`PostgresSchemaRegistrationStore::register_current` is the explicit generic authority-transition
primitive for that case.

It is deliberately separate from ordinary `register`.

## Ordinary registration is unchanged

`register` continues to:

- serialize one tenant/module/entity identity;
- preserve exact-key idempotency;
- reject same-key contract reuse;
- reject insertion of an unregistered non-increasing routing key;
- reject reactivation of a retired exact contract;
- leave any previously active lower routing keys unchanged.

No existing module gains automatic retirement behavior merely because this API exists.

That unchanged behavior is useful for a staged rebuild: a monotonically higher replacement contract may
be registered and populated while the lower persisted contract is still active.

## Explicit current supersession

`register_current(tenant_id, schema)` runs in one storage transaction and under the same PostgreSQL
identity advisory lock used by ordinary registration.

It requires the incoming routing key to be at least the latest persisted key for that exact
`tenant/module/entity` identity.

The operation then:

1. resolves or inserts the exact incoming immutable contract;
2. requires an existing exact contract to be active and fingerprint/JSON identical;
3. atomically changes every lower `active` key for the same identity to `retired`;
4. updates the retirement timestamp;
5. returns the ordinary registration outcome plus the number of lower contracts retired.

Calling `register_current` again for the same latest active contract is idempotent and retires zero
additional rows.

Trying to declare an older historical key current after a later key exists fails with
`NonMonotonicVersion`, even when that older exact contract is still present in storage.

## Historical storage and authority boundary

`index_entities` and `index_links` retain the numeric schema key and schema fingerprint in their storage
identity/foreign-key chain. Replay/rebuild checkpoints also include `schema_version` in their primary
identity. Supersession does **not** rewrite or delete those historical rows.

That is intentional:

- historical entity/link rows keep valid foreign keys to their immutable schema row;
- replacement entity/link/checkpoint state uses a different routing key and cannot collide with lower
  schema keys;
- tenant schema readiness requires the exact runtime-selected `SchemaRef`, fingerprint, JSON, and
  `status = active`;
- PostgreSQL query execution performs the same exact persisted-schema status/fingerprint/contract
  check before returning rows;
- an old runtime/schema reference therefore fails closed once its persisted contract is retired.

Retirement is an authority transition, not a data purge.

## Inbox delivery identity is a separate boundary

`index_inbox` intentionally stores `schema_version`, but its deduplication primary key is
`(tenant_id, source_name, delivery_id)`. Therefore a replacement source cannot rely on the new numeric
schema key alone to separate historical and replacement deliveries.

The legacy `derive_index_source_event_id` remains stable for existing sources and is **not** changed by
supersession work. It hashes the owner-selected domain, tenant, entity, locale, and source version, but
not the `SchemaRef`.

A source that replays the same owner mutation/source-version pair under a replacement schema routing key
must instead use `derive_index_schema_source_event_id`. The schema-scoped helper additionally hashes the
exact schema module, entity, and numeric routing key. This gives:

- stable delivery UUIDs for retries of the same exact replacement schema;
- a different delivery UUID for the same owner mutation under a different schema key;
- no collision with a historical inbox row merely because source name, owner domain, entity, locale,
  and source version are unchanged.

The schema-scoped helper is an internal storage/replay identity mechanism. It does not introduce a
versioned event family or compatibility route.

## Recommended staged rebuild sequence

For a replacement that should avoid unnecessarily retiring the old persisted authority before the new
materialization is ready:

1. source code defines **one** replacement current schema; no old compatibility branch is added;
2. use ordinary `register` to stage the monotonically higher immutable routing key while the lower
   persisted key remains active;
3. ensure the replacement source derives deterministic replay delivery IDs with
   `derive_index_schema_source_event_id` before rebuilding the new key;
4. replay/rebuild the replacement key completely;
5. verify exact persisted readiness, parity, freshness, inbox isolation, and restart evidence for the
   replacement key;
6. call `register_current` with that already-staged exact contract;
7. in that final transaction every lower active key becomes `retired`;
8. cut authoritative consumers to the replacement runtime only after that authority transition.

A rolling deployment can temporarily have old and new process generations, and staging can temporarily
leave two persisted keys `active`, but source code still contains only one replacement implementation.
There is no compatibility branch or dual query path in the new runtime.

If fail-closed downtime is acceptable, callers may invoke `register_current` before rebuild; the old
contract then becomes non-authoritative immediately. The staged sequence above is preferred for
production replacement.

Historical rows may be purged later through a separately admitted maintenance policy, but purge is not
required for correctness and is intentionally not coupled to source-module migrations.

## Current Product key-4 application

Product has already crossed the **source-code** replacement boundary described above. Current runtime code
publishes exactly one 15-field Product contract on routing key `4`; lower Product keys are historical storage
identities only. The selected Product source uses `derive_index_schema_source_event_id` and explicitly rejects
reintroduction of the old key `3` compatibility path.

This does **not** mean every tenant has completed persisted promotion. For a tenant that still has a lower
Product contract active, the current key-4 rollout must follow the staged sequence:

1. ordinary-register the exact current Product key `4` contract while any lower persisted Product key remains
   active;
2. rebuild key `4` with schema-scoped delivery UUIDs so historical inbox rows cannot suppress replacement
   deliveries;
3. retain and execute current-key readiness/freshness/parity/restart evidence;
4. call `register_current` with the same already-staged Product key `4` contract;
5. require all lower active Product schema rows for that tenant to become `retired` atomically;
6. require old-key readiness/query execution to fail closed as inactive;
7. only then admit an authoritative Product Index consumer for that tenant.

The runtime must not stage or select a Product key `3` implementation merely because persisted key `3` rows may
still exist. Historical rows remain valid storage history; they are not a compatibility surface.

The focused M7 Product promotion contract is retained in
`m7-product-current-schema-promotion.md` and guarded by
`verify-index-product-current-schema-promotion.mjs`.

## Deliberate limits

This primitive does not:

- automatically stage Product key `4` for every tenant;
- run Product replay/rebuild jobs;
- execute or admit Product current-key PostgreSQL evidence;
- delete old Index materialization;
- change public event contracts;
- authorize Storefront cutover.

Those are separate owner/execution/evidence decisions.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-index source_event_id --lib -- --nocapture
cargo test -p rustok-index schema_registration --lib -- --nocapture
node scripts/verify/verify-index-schema-scoped-source-event-id.mjs
node scripts/verify/verify-index-schema-supersession.mjs
node scripts/verify/verify-index-product-current-schema-promotion.mjs
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
