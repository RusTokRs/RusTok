# M4 single-current schema supersession

Status: `source_complete_execution_pending`.

## Purpose

Index schema contracts are immutable at one exact `SchemaRef`: reusing the same numeric routing key
with a different fingerprint/JSON contract fails closed. That rule prevents silent schema drift, but a
consumer eventually needs a safe way to replace one current contract with another without keeping two
runtime compatibility branches active.

`PostgresSchemaRegistrationStore::register_current` is the explicit generic supersession primitive for
that case.

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

## Why retirement is enough for the registration boundary

`index_entities` and `index_links` retain the numeric schema key and schema fingerprint in their storage
identity/foreign-key chain. Supersession does **not** rewrite or delete those historical rows.

That is intentional:

- historical rows keep valid foreign keys to their immutable schema row;
- new-current mutations use a different routing key and therefore cannot collide with old entity,
  link, inbox, checkpoint, or replay identities;
- tenant schema readiness requires the exact runtime-selected current `SchemaRef`, fingerprint, JSON,
  and `status = active`;
- PostgreSQL query execution performs the same exact persisted-schema status/fingerprint/contract
  check before returning rows;
- an old runtime/schema reference therefore fails closed once its persisted contract is retired.

Retirement is an authority transition, not a data purge.

## Rebuild/cutover boundary

`register_current` does **not** claim that the new current contract has been materialized. A caller that
uses supersession must still:

1. publish only the one new current schema in the runtime source catalog;
2. register/supersede that contract for the tenant;
3. rebuild or replay all required entities/links under the new routing key;
4. require exact persisted readiness for the new key;
5. retain parity/freshness/restart evidence;
6. only then cut an authoritative consumer over.

Historical rows may be purged later through a separately admitted maintenance policy, but purge is not
required for correctness and is intentionally not coupled to source-module migrations.

## Product Storefront relevance

The current Product Index contract cannot be expanded under its existing persisted routing key because
that would change the fingerprint. The Storefront parity gate also rejects introducing parallel Product
v4/v5 compatibility branches.

This generic supersession primitive provides the missing persistence mechanism for a future
**single-current** Product replacement:

- one monotonically higher internal routing key;
- only that contract published by Product runtime code;
- all lower persisted Product keys retired atomically per tenant;
- new-key replay/rebuild before Storefront cutover;
- no old Product source/query compatibility branch selected in parallel.

This slice does not change the Product routing key or Product schema yet.

## Deliberate limits

This primitive does not:

- choose a Product replacement key;
- alter Product fields;
- register schemas automatically for every tenant;
- run replay/rebuild jobs;
- delete old Index materialization;
- change event contracts;
- authorize Storefront cutover.

Those are separate owner/execution/evidence decisions.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-index schema_registration --lib -- --nocapture
node scripts/verify/verify-index-schema-supersession.mjs
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
