# M7 Product current-schema promotion

Status: `postgres_packet_source_complete_execution_pending`.

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

## Retained PostgreSQL promotion packet — source complete

`crates/rustok-distribution/tests/product_current_schema_promotion_postgres.rs` retains an isolated PostgreSQL
scenario using production Product and Index migrations plus the selected distribution runtime.

The packet obtains the exact current Product schema from `SharedIndexSchemaRegistry` and asserts key `4`; it
does not copy the Product field/link contract into the test. It then creates a **storage-only lower-key
fixture** by cloning that immutable current contract and changing only the routing key to `3` before ordinary
registration. This fixture exists solely to exercise lower-active-schema staging, retirement and typed inactive
checks. It does not reconstruct or select the historical key3 Product implementation and it never registers a
key3 Product source/factory.

The retained scenario then:

1. ordinary-registers the lower storage fixture and every current runtime schema, proving key3/key4 persisted
   coexistence during staging;
2. materializes the real `product-postgres-primary` source and current query runtime;
3. loads one real key4 Product mutation from Product owner tables;
4. proves its event ID equals `derive_index_schema_source_event_id` for key4 and differs from the same owner
   coordinates under key3;
5. applies that mutation through `PostgresMutationStore` and queries the key4 materialization;
6. calls `register_current` with the already-staged exact current Product key4 schema and requires exactly one
   lower active Product schema to retire;
7. repeats `register_current` and requires zero further retirement;
8. builds a test-only immutable probe registry containing the lower storage contract plus current schemas, then
   requires `PostgresIndexSchemaReadinessStore` and `PostgresIndexQueryPort` to return typed `Inactive` for key3;
9. requires the existing key4 runtime to remain queryable after promotion;
10. rebuilds distribution/query composition on a separate PostgreSQL connection and requires exactly one
    runtime Product schema, key4, to read the retained key4 materialization after restart.

The packet uses production `register`, `register_current`, schema readiness, query verification, source
materialization and mutation storage. The only synthetic element is the lower-key storage/probe contract needed
to exercise supersession without reviving a deleted compatibility source.

## Storefront gate

Mounted Storefront remains owner-native. Promotion source completeness does not establish Storefront parity,
collation admission, timeout/latency admission or tenant readiness. Channel-less and deep-page Storefront
request shapes remain owner-native under the current key-4 contract.

## Maintainer execution still required

The new PostgreSQL packet is retained source, not an executed result. Maintainer-owned execution must still
confirm the packet compiles and passes against PostgreSQL, then review it together with current-key
readiness/freshness/Storefront/collation evidence before any tenant promotion or traffic switch.

In particular, this source state does not claim:

- a real tenant has been staged/promoted;
- the old production key3 fingerprint/JSON was reconstructed;
- key4 replay/restart evidence has passed;
- collation or Storefront parity has been admitted;
- latency/cancellation evidence has been admitted.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
