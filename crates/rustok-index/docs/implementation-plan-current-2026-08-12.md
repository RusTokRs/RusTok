# `rustok-index` Current Implementation Plan — 2026-08-12

Status: `m5_product_refresh_host_consumer_boundary`

This document supersedes `implementation-plan-current-2026-08-09.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The baseline for this update is `main@a1a57270094d62eb9ce28dacdbb40c0cfcf6bd87` on 2026-08-12, the squash merge of PR #3450.

The previously completed Index foundation remains present and is not reopened:

- M1–M4 generic schema/query/storage boundaries remain the current architecture.
- PostgreSQL source factories still own Product and ProductVariant authoritative replay loading.
- durable inbox/source-version monotonicity remains the write-side idempotency boundary.
- the generic `IndexSourceRefreshEventWorker` still performs exact source load, durable mutation application and acknowledgement in that order.
- the canonical Product refresh owner ledger, writer, durable relay and typed event family remain the only Product refresh wire contract.
- the Product refresh event family remains exactly:
  - `product.index.locale_refresh_requested`
  - `product.index.variant_refresh_requested`
- Product routes as `rustok-product::product@4`.
- ProductVariant routes as `rustok-product::product_variant@2`.
- no legacy/v2/fallback event family is admitted.

PR #3450 also closed the stale canonical digest artifact and the one-line `rustok-auth` `Iden` derive prerequisite that previously prevented selected `rustok-distribution` compilation. Those repairs are now part of `main` and are not reopened by this revision.

## 2. M5 typed delivery baseline recheck

The Product-specific delivery bridge remains in `rustok-distribution` and delegates to the generic source-refresh worker.

Locale refresh projects to Product `rustok-product::product@4`, canonicalizes the locale through `LocaleKey`, and fences replay with the owner `source_version`.

Variant refresh projects to ProductVariant `rustok-product::product_variant@2`, targets `variant_id`, retains `locale = None`, validates the owner `product_id`, and uses the same source-version fence.

The generic invariant remains:

1. resolve exact event route;
2. resolve exact authoritative source;
3. load exactly one target key;
4. reject missing, ambiguous or behind source state;
5. rebind the replay mutation to the owner event UUID;
6. durably apply the mutation through the replay mutation sink;
7. acknowledge only after durable application succeeds.

## 3. M5 host consumption boundary implemented by this revision

### 3.1 Dedicated persistent broker cursor

The server now owns a default-off Product Index refresh worker with:

- runtime flag `RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED`;
- topic `domain`, matching the existing sealed domain-event transport;
- dedicated consumer group `rustok-product-index-refresh`;
- bounded idle polling and bounded retry backoff sourced from the existing event relay retry policy;
- `StopHandle`-driven shutdown using `tokio::select!`;
- runtime-consumer telemetry for startup, receive, decode, retry and termination stages.

The worker is admitted only for `outbox_iggy` delivery and starts only after `bootstrap_app_runtime` has published the materialized module runtime extensions.

### 3.2 Canonical envelope projection only

The host reads a validated `ContractEventEnvelope` from `PersistentContractConsumerGroup` and maps only:

- `ProductIndexRefreshEvent::LocaleRefreshRequested` -> `ProductIndexRefreshDelivery::locale`;
- `ProductIndexRefreshEvent::VariantRefreshRequested` -> `ProductIndexRefreshDelivery::variant`.

Other sealed domain-event families are unrelated to this dedicated Product cursor and are acknowledged without entering Index mutation work.

Undecodable broker bytes are not acknowledged by this slice. No Product-specific raw-poison/DLQ protocol is introduced here.

### 3.3 Opaque acknowledgement identity retained

The host acknowledgement adapter uses `ConsumedContractEvent` itself as the generic `IndexMutationEventAcknowledger::Token`.

This is deliberate: `ConsumedContractEvent` retains the connector-owned cursor metadata and opaque acknowledgement token. The host does not copy, stringify, derive or reconstruct that token from the logical event UUID.

`ProductIndexRefreshDeliveryWorker` delegates to `IndexSourceRefreshEventWorker`, so the broker offset is committed only after durable Index mutation application succeeds.

If durable apply succeeds but broker acknowledgement fails, bounded in-process retry re-enters the same generic path. Existing inbox deduplication and source-version monotonicity make that redelivery safe.

### 3.4 Fail-closed retry boundary

For Product refresh deliveries, this revision adds no second terminal path.

After bounded retries are exhausted, the worker terminates and leaves the source offset uncommitted. This applies to source load failures, missing/ambiguous/behind source state, persistence failures and acknowledgement failures.

The next broker delivery attempt therefore remains the recovery mechanism; no compatibility event, direct mutation fallback or Product-specific DLQ mutation path is created.

## 4. Focused admission expanded with the host boundary

`Index Contract CI` now also watches the Product host consumer and server bootstrap.

The revision adds:

```text
node scripts/verify/verify-index-product-refresh-host-consumer.mjs
cargo check --locked -p rustok-server --no-default-features --features mod-product --lib
cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib
```

The host verifier pins:

- the public distribution delivery/worker boundary;
- the dedicated persistent consumer group and shared `domain` topic;
- exact mapping of the two canonical Product refresh variants;
- use of `ConsumedContractEvent` as the opaque acknowledgement token;
- materialized schema/source/event registries plus `PostgresMutationStore`;
- bounded retry and `StopHandle` shutdown;
- absence of a second Product refresh DLQ/fallback route;
- continued generic durable-apply-before-ack ordering.

## 5. Next M5 execution boundary

After this host consumer is admitted, the next slice is host-backed redelivery evidence against the real broker/database adapters.

It must prove, with bounded integration evidence:

1. a canonical Product locale refresh published through the selected Iggy transport is consumed by `rustok-product-index-refresh`;
2. the authoritative Product source is loaded and the resulting mutation reaches PostgreSQL before source offset commit;
3. a canonical ProductVariant refresh follows the same exact route and non-localized key contract;
4. an injected post-persistence acknowledgement failure leaves the source offset recoverable and a later redelivery resolves through inbox deduplication without duplicate logical mutation;
5. a behind/missing authoritative source result remains unacknowledged;
6. restart resumes from the persistent broker cursor without a second ingestion path;
7. evidence remains bounded and does not turn the default-off deployment flag on automatically.

Do not start partition-wide replay, storefront cutover, or a Product-specific DLQ protocol as part of that evidence slice.

## 6. M6/M7 gates remain unchanged

The following work remains gated and is not implicitly opened by the Product host consumer:

- partition-scoped replay remains blocked until the owner source applies the partition predicate before keyset pagination rather than filtering after page selection;
- drift/reconciliation repair keeps the existing bounded source and recovery contracts;
- Product graph/storefront cutover keeps the existing readiness, relation-admission and parity gates;
- historical schema identities remain storage history only and must not become runtime fallback implementations.

## 7. Merge admission for this revision

Before merge, require all of the following on the revision head:

```text
node scripts/verify/verify-index-contract-ci.mjs
node scripts/verify/verify-event-contract-digest-admission.mjs
node scripts/verify/verify-index-product-refresh-event-family.mjs
node scripts/verify/verify-index-product-refresh-delivery.mjs
node scripts/verify/verify-index-product-refresh-host-consumer.mjs
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json
cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets
cargo check --locked -p rustok-distribution --features mod-product --lib
cargo check --locked -p rustok-server --no-default-features --features mod-product --lib
cargo test --locked -p rustok-events --test canonical_contracts
cargo test --locked -p rustok-index source_refresh_event --lib
cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib
cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib
```

The active cursor after this revision is the real Iggy/PostgreSQL redelivery evidence boundary described in section 5.
