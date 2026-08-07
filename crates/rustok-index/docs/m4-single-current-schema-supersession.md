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

## Why retirement is enough for the authority boundary

`index_entities` and `index_links` retain the numeric schema key and schema fingerprint in their storage
identity/foreign-key chain. Supersession does **not** rewrite or delete those historical rows.

That is intentional:

- historical rows keep valid foreign keys to their immutable schema row;
- replacement mutations use a different routing key and therefore cannot collide with old entity,
  link, inbox, checkpoint, or replay identities;
- tenant schema readiness requires the exact runtime-selected `SchemaRef`, fingerprint, JSON, and
  `status = active`;
- PostgreSQL query execution performs the same exact persisted-schema status/fingerprint/contract
  check before returning rows;
- an old runtime/schema reference therefore fails closed once its persisted contract is retired.

Retirement is an authority transition, not a data purge.

## Recommended staged rebuild sequence

For a replacement that should avoid unnecessarily retiring the old persisted authority before the new
materialization is ready:

1. source code defines **one** replacement current schema; no old compatibility branch is added;
2. use ordinary `register` to stage the monotonically higher immutable routing key while the lower
   persisted key remains active;
3. replay/rebuild the replacement key completely;
4. verify exact persisted readiness, parity, freshness, and restart evidence for the replacement key;
5. call `register_current` with that already-staged exact contract;
6. in that final transaction every lower active key becomes `retired`;
7. cut authoritative consumers to the replacement runtime only after that authority transition.

A rolling deployment can temporarily have old and new process generations, and staging can temporarily
leave two persisted keys `active`, but source code still contains only one replacement implementation.
There is no Product v4/v5 compatibility branch or dual query path in the new runtime.

If fail-closed downtime is acceptable, callers may invoke `register_current` before rebuild; the old
contract then becomes non-authoritative immediately. The staged sequence above is preferred for
production replacement.

Historical rows may be purged later through a separately admitted maintenance policy, but purge is not
required for correctness and is intentionally not coupled to source-module migrations.

## Product Storefront relevance

The current Product Index contract cannot be expanded under its existing persisted routing key because
that would change the fingerprint. The Storefront parity gate also rejects introducing parallel Product
v4/v5 compatibility branches.

This generic supersession primitive provides the missing persistence mechanism for a future
**single-current** Product replacement:

- one monotonically higher internal routing key;
- only that replacement contract published by new Product runtime code;
- staged new-key replay/rebuild before authority transition;
- all lower persisted Product keys retired atomically per tenant at final supersession;
- no old Product source/query compatibility branch selected in the replacement runtime.

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