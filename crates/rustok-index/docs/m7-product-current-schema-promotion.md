# M7 Product current-schema promotion

Status: `source_contract_complete_execution_pending`.

## Current source identity

Current Product runtime code publishes exactly one Product Index schema on routing key `4` through
`PRODUCT_SCHEMA_ROUTING_KEY`. The schema contains 15 fields and two links. Lower Product routing keys are
historical persisted identities only; they are not selected as runtime compatibility implementations.

`product-postgres-primary` derives replay delivery UUIDs with `derive_index_schema_source_event_id` using the
exact Product `SchemaRef`. The same owner mutation/source-version pair therefore receives a distinct delivery
identity when replayed under a different Product schema key, while retries of the same key remain stable.

The Product source and its current absence/query-admission consumers reject a hard-coded key `3` path.

## Tenant promotion sequence

Source completeness does not imply that every tenant has already promoted persisted Product authority to key
`4`. For a tenant with a lower active Product schema, promotion is explicitly staged:

1. ordinary-register the exact Product key `4` immutable contract;
2. leave lower persisted Product keys untouched while key `4` is rebuilt;
3. rebuild key `4` using schema-scoped replay delivery IDs;
4. execute/admit exact key-4 readiness, freshness, parity, inbox-isolation and restart evidence;
5. call `PostgresSchemaRegistrationStore::register_current` with the already-staged key `4` contract;
6. require every lower active Product schema for that tenant to become `retired` atomically;
7. require lower-key readiness/query execution to fail closed because the persisted schema is inactive;
8. only after promotion may an authoritative Product Index consumer be admitted for that tenant.

Ordinary registration must not retire lower contracts. `register_current` is the authority transition, not the
staging primitive.

## Historical state

Promotion does not rewrite or delete historical `index_entities`, `index_links`, inbox deliveries or replay
checkpoints. Their schema key/fingerprint identities remain valid history. Old materialization may be purged
later only through a separately admitted maintenance policy.

A rolling deployment may temporarily observe more than one persisted active Product schema during staging, but
current source code still contains one Product implementation. Persisted coexistence is not runtime dual-read
compatibility.

## Storefront gate

Mounted Storefront remains owner-native. Promotion source completeness does not establish Storefront parity,
collation admission, timeout/latency admission or tenant readiness. Channel-less and deep-page Storefront
request shapes remain owner-native under the current key-4 contract.

## Execution still required

Maintainer-owned execution must still prove at least:

- Product key `4` can be staged while the lower persisted key remains active;
- schema-scoped key-4 replay does not collide with historical inbox delivery identities;
- exact key-4 readiness/freshness/parity/restart packets pass;
- `register_current` retires lower active Product keys only after staging/rebuild;
- an old Product schema reference fails readiness/query admission after retirement;
- restart after promotion resolves only the current runtime-selected key `4` contract.

This document and its source guard do not claim those runtime results.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
