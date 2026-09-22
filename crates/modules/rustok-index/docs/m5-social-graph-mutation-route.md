# M5 Social Graph production mutation route

Status: `source_complete_runtime_execution_pending`.

## Purpose

The generic M5 mutation-event contract already guarantees commit-before-ack ordering, exact schema
routing, durable inbox deduplication, and monotonic source versions. Social Graph already owns a
production Iggy consumer with bounded retry/backoff, DLQ handling, raw-poison receipts, graceful
shutdown, and runtime metrics. The missing boundary was a canonical replay source and an immutable
`IndexMutationEventCatalog` route tying that live consumer to the same source/schema identity.

This slice closes that source-composition gap without changing the concrete Iggy worker or bypassing
the separate concrete-repair PostgreSQL evidence gate.

## Registered identities

The Social Graph owner registers:

```text
owner module:  social_graph
schema:        rustok-social-graph/relation/v1
source:        social_graph.relation.state_changed.v1
event domain:  social_graph.relation.state_changed.v1
factory:       social-graph-relation-index-source
```

The source name deliberately matches `SOCIAL_GRAPH_INDEX_SOURCE` in the existing live consumer.
Incremental deliveries therefore continue to enter `index_inbox` under the same source identity that
bounded replay uses.

Live broker deliveries retain their sealed owner event UUID. Replay mutations use a separate
versioned deterministic derivation domain:

```text
rustok-social-graph.relation-replay-v1
```

The identities need not be equal. Inbox deduplication protects exact redelivery, while the monotonic
relation revision protects convergence when replay and live ingestion observe the same logical
version through different delivery UUIDs.

## Bounded replay source

`SocialGraphRelationPostgresIndexSource` reads the authoritative `social_graph_relations` table.

`scan`:

- requires the exact non-localized relation schema;
- accepts the generic Index page bound of at most 1,000 rows;
- uses one source-owned cursor containing only a non-nil relation UUID;
- filters one exact tenant;
- orders by relation UUID;
- fetches `limit + 1` and emits an advancing cursor only when another page exists.

`load`:

- accepts at most 256 exact keys through `IndexSourceLoadRequest`;
- rejects locale-bearing keys;
- filters one exact tenant and the requested relation UUID set;
- returns only existing authoritative rows in deterministic UUID order.

Every row is revalidated before conversion. Tenant, relation, source user, and target user UUIDs must
be non-nil; source and target users must differ; relation revision must be positive; and relation kind
must pass the sealed Social Graph event contract. Active rows become upserts and inactive rows become
revisioned tombstones through the existing owner conversion function.

The source adapter rejects non-PostgreSQL execution. Factory registration itself performs no SQL and
starts no task.

## Atomic source and route materialization

`materialize_postgres_index_sources` now stages both boundaries together:

1. clone the current runtime extensions;
2. invoke every selected PostgreSQL source factory into the staged source catalog;
3. materialize the mutation event registry against that exact staged catalog;
4. verify every route owner, source name, and schema;
5. publish the source catalog and immutable event registry only if the complete batch is valid.

A factory failure, unknown source, owner mismatch, or schema mismatch leaves the live extensions
unchanged. A partially registered production route cannot escape startup composition.

## Existing concrete consumer

The already existing Social Graph Iggy worker remains the concrete transport owner. It provides:

- one persistent contract consumer cursor;
- sealed envelope decoding and exact-byte poison retention;
- durable Index mutation commit before source-offset acknowledgement;
- duplicate-safe redelivery after acknowledgement loss;
- bounded retry and backoff;
- durable DLQ/poison receipts;
- graceful shutdown with an uncommitted in-flight offset;
- consumer lag, outcome, failure-stage, and termination metrics.

This slice does not duplicate that worker. It supplies the canonical registry/source identities that
were missing from module and host composition.

## Deliberate limits

This slice does not:

- run the Social Graph worker or broker;
- retain new PostgreSQL/Iggy runtime output;
- register Product or ProductVariant incremental routes;
- add batch mutation transactions beyond the existing per-delivery durable boundary;
- change retry, DLQ, lag-metric, or poison-receipt policy;
- expose drift repair through a public command surface;
- add automatic repair iteration or time-derived repair leases.

The current concrete-repair cursor remains `M6 - execute and admit concrete repair evidence`.

## Maintainer validation

Suggested owner-run commands:

```bash
cargo test -p rustok-social-graph --features index-consumer index_source -- --nocapture
cargo test -p rustok-index source_factory -- --nocapture
node scripts/verify/verify-index-social-graph-mutation-route.mjs
cargo check -p rustok-social-graph --features index-consumer --all-targets
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, PostgreSQL/Iggy scenarios, workflows, or CI were
run by the implementation agent.
