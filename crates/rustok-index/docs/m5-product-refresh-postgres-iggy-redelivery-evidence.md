# M5 Product refresh PostgreSQL/Iggy redelivery evidence

Status: `source_ready_maintainer_execution_pending`

## Purpose

`apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs` is the bounded host-backed evidence harness for the Product Index refresh consumer boundary. It composes the real external Iggy persistent consumer-group cursor with the production Product/ProductVariant PostgreSQL Index sources and the production `PostgresMutationStore`.

The source contract is:

```text
crates/rustok-index/contracts/evidence/product-refresh-postgres-iggy-source.json
```

The source verifier is:

```text
scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
```

A successful source review or compile is not runtime evidence. The runtime claim remains pending until the executable test is run against operator-approved PostgreSQL and external Iggy services without a skip.

## Required services

Provide explicit, evidence-scoped settings:

```text
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL=postgresql://...
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS=host:8090
```

Optional Iggy credentials must be supplied together:

```text
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME=...
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD=...
```

There is intentionally no `DATABASE_URL`, localhost, default credential or default broker fallback. Missing PostgreSQL or Iggy settings makes a direct developer invocation report a skip and return successfully; such a skip must never be promoted as retained runtime evidence.

The first evidence boundary is external TCP/non-TLS only. TLS, authentication failure, failover and multi-replica rebalance behavior remain separate work.

## Isolation

Each scenario creates:

- one unique PostgreSQL schema;
- dedicated one-connection SeaORM pools with a session-local `search_path`;
- only the Channel, Product and Index migrations required by the production Product source path;
- one unique Iggy stream with one `domain` partition;
- the exact production consumer group `rustok-product-index-refresh`.

PostgreSQL cleanup drops only the unique schema. The harness does not call an unreviewed Iggy stream deletion API, so the broker must be disposable or have an operator-approved cleanup process.

## Scenario 1: PostgreSQL commit before failed broker acknowledgement

The harness loads current Product and ProductVariant mutations through the materialized production source registry, then publishes two canonical typed contracts through `IggyTransport::publish_contract`: locale refresh first and variant refresh second.

For the locale delivery it uses `ConsumedContractEvent` itself as the opaque acknowledgement token, exactly like the server worker. The evidence acknowledger changes only a clone of the returned broker acknowledgement token on its first call. `PersistentContractConsumerGroup::acknowledge` rejects the mismatched token before offset commit.

The expected order is therefore:

1. receive canonical Product locale refresh from the exact production consumer group;
2. load the exact authoritative Product source key;
3. apply the rebound event UUID through `PostgresMutationStore`;
4. observe one `index_entities` row at the current source version and one `index_inbox` row in `applied` state;
5. attempt the deliberately mismatched acknowledgement and receive an acknowledgement failure;
6. drop the first consumer and transport without a committed source offset;
7. reopen the same stream and production consumer group;
8. require the same broker offset, same typed envelope UUID and exact raw payload bytes;
9. process the redelivery through the same production source and mutation path;
10. require `IndexReplayMutationOutcome::Duplicate` and still exactly one applied inbox identity;
11. acknowledge successfully and receive the queued ProductVariant refresh at a greater broker offset;
12. apply ProductVariant as `rustok-product::product_variant@2` with `locale_key = ''` and one applied inbox identity.

This proves the intended cross-adapter recovery boundary without a second ingestion implementation.

## Scenario 2: authoritative source behind the owner event

A separate isolated scenario publishes a canonical locale refresh whose `source_version` is exactly one greater than the current authoritative Product source revision.

The generic source-refresh worker must return `SourceVersionBehind` before mutation persistence or broker acknowledgement. The harness requires no matching `index_inbox` row, then restarts the Iggy transport and production consumer group and requires the same broker offset and exact raw payload bytes again.

This is the real-adapter evidence for the fail-closed source-version fence. Missing-source behavior remains separately covered by the generic source-refresh contract tests; this combined harness chooses the behind-source branch because it also proves the owner revision fence against a present authoritative source.

## Production parity

The static verifier pins the harness to the server worker and generic Index worker. It requires:

- exact topic `domain` and group `rustok-product-index-refresh`;
- exact `ProductIndexRefreshEvent` locale/variant mapping;
- `ConsumedContractEvent` as the acknowledgement token;
- `ProductIndexRefreshDeliveryWorker` plus materialized schema/source/event registries;
- production `PostgresMutationStore`;
- generic apply-before-ack ordering;
- no Product refresh DLQ/fallback path;
- no use or enabling of `RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED` by the evidence harness.

The evidence harness is test-only and does not replace the server loop, retry policy, telemetry or deployment flag.

## Maintainer execution

```bash
node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs

RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL='postgresql://...' \
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS='host:8090' \
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME='...' \
RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD='...' \
  cargo test --locked -p rustok-server \
  --no-default-features --features mod-product \
  --test product_index_refresh_redelivery_postgres_iggy \
  -- --nocapture --test-threads=1
```

The username/password pair may both be omitted when the disposable broker permits anonymous access.

## Non-claims

Even after a successful external execution, this packet does not claim a distributed PostgreSQL/Iggy transaction, physical exactly-once broker delivery, Product-specific DLQ behavior, partition-wide replay, storefront cutover, multi-replica rebalance guarantees, or TLS/auth/failover coverage.
