# `rustok-index` Current Implementation Plan — 2026-08-12

Status: `m5_product_refresh_redelivery_evidence_source_ready`

This document supersedes `implementation-plan-current-2026-08-09.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The baseline for this update is `main@f998be7402527d116a4f45e9fd5f823288cbd681` on 2026-08-12, the squash merge of PR #3455.

The previously completed Index foundation remains present and is not reopened:

- M1–M4 generic schema/query/storage boundaries remain the current architecture.
- PostgreSQL source factories still own Product and ProductVariant authoritative replay loading.
- durable inbox/source-version monotonicity remains the write-side idempotency boundary.
- `IndexSourceRefreshEventWorker` still resolves the exact source, applies the durable mutation and acknowledges in that order.
- the canonical Product refresh owner ledger, writer, relay and typed event family remain the only Product refresh wire contract.
- Product refresh remains exactly `product.index.locale_refresh_requested` and `product.index.variant_refresh_requested`.
- Product routes as `rustok-product::product@4`; ProductVariant routes as `rustok-product::product_variant@2`.
- no legacy/v2/fallback Product refresh family is admitted.

PR #3455 completed the default-off server host consumer on the shared `domain` topic with dedicated group `rustok-product-index-refresh`, `ConsumedContractEvent` as the opaque acknowledgement token, production `PostgresMutationStore`, bounded retry/lifecycle telemetry and no Product-specific DLQ path.

## 2. Host boundary recheck

The host consumer remains admitted only for `outbox_iggy` delivery and only when `RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED` is explicitly enabled. It starts after module runtime materialization.

The host maps only:

- `ProductIndexRefreshEvent::LocaleRefreshRequested` -> `ProductIndexRefreshDelivery::locale`;
- `ProductIndexRefreshEvent::VariantRefreshRequested` -> `ProductIndexRefreshDelivery::variant`.

`ProductIndexRefreshDeliveryWorker` still delegates to the generic source-refresh worker, so the invariant remains:

1. resolve exact event route and schema;
2. resolve exact authoritative source;
3. load exactly one target key;
4. reject missing, ambiguous or behind source state;
5. rebind the source mutation to the owner event UUID;
6. durably apply through the replay mutation sink;
7. acknowledge only after durable persistence succeeds.

## 3. M5 redelivery evidence source added by this revision

This revision adds a bounded cross-adapter executable proof source:

```text
apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs
```

It uses only public production boundaries for the behavior under evidence:

- `IggyTransport` and `PersistentContractConsumerGroup` for broker delivery and offset acknowledgement;
- canonical `ContractEventEnvelope` / `ProductIndexRefreshEvent` publication;
- materialized Product/ProductVariant PostgreSQL Index sources;
- `ProductIndexRefreshDeliveryWorker` and the generic source-refresh worker;
- production `PostgresMutationStore` and durable `index_inbox` deduplication.

The harness is opt-in and requires explicit evidence-scoped PostgreSQL and external Iggy settings. It has no `DATABASE_URL`, localhost, credential or broker fallback. Missing required settings produces a developer skip; a skip is explicitly not runtime evidence.

### 3.1 Locale post-persistence ACK failure and restart

The first scenario publishes canonical locale and variant refresh envelopes through the selected Iggy transport.

For the first locale delivery, the evidence acknowledger alters only a clone of the broker acknowledgement token. The real Iggy consumer group therefore rejects the ACK after the generic worker has already committed the Product mutation.

The scenario requires:

- Product `index_entities` state and one applied `index_inbox` identity to exist after the injected ACK failure;
- consumer/transport restart to return the same uncommitted broker offset and exact raw envelope bytes;
- redelivery to resolve as `IndexReplayMutationOutcome::Duplicate` through the durable inbox;
- exactly one applied inbox identity to remain;
- successful acknowledgement to advance the group to the queued ProductVariant event;
- ProductVariant to materialize as schema version 2 with the non-localized key and one applied inbox identity.

### 3.2 Behind-source restart

The second isolated scenario publishes a canonical Product locale refresh whose minimum owner `source_version` is one revision above the currently visible authoritative Product source.

The generic worker must return `SourceVersionBehind` before mutation persistence or acknowledgement. The harness requires no matching `index_inbox` row, restarts the Iggy transport/group and requires the same uncommitted offset and exact raw payload again.

This combined proof selects the behind-source branch of the fail-closed contract. Missing-source behavior remains pinned by the existing generic source-refresh tests.

## 4. Evidence contract and admission

The machine-readable source contract is:

```text
crates/rustok-index/contracts/evidence/product-refresh-postgres-iggy-source.json
```

The operator guide is:

```text
crates/rustok-index/docs/m5-product-refresh-postgres-iggy-redelivery-evidence.md
```

`Index Contract CI` adds source-only admission:

```text
node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy
```

The focused gate intentionally compiles but does not execute the external-service test without explicit Iggy/PostgreSQL settings. This prevents an environment-driven skip from being mislabeled as runtime proof.

The verifier pins the harness to the production host route, exact canonical event mapping, `ConsumedContractEvent` acknowledgement identity, materialized source/event registries, `PostgresMutationStore`, generic durable-apply-before-ack ordering and the absence of Product-specific fallback/DLQ behavior.

## 5. Runtime evidence status and next M5 execution boundary

This revision is **source-ready only**. `evidence_status` remains `runtime_execution_pending` until the harness is executed against operator-approved PostgreSQL and external Iggy services without a skip.

The next M5 execution boundary is therefore retained external execution of this exact source packet, not another production ingestion implementation.

A promotable execution must demonstrate all of the following on one reviewed source commit:

1. locale refresh reaches PostgreSQL before the injected broker ACK failure;
2. restart returns the same uncommitted Iggy offset and raw payload;
3. locale redelivery resolves through durable inbox dedup without a second logical mutation;
4. successful ACK advances the same production consumer group to ProductVariant;
5. ProductVariant materializes through its authoritative source and non-localized key;
6. a behind-source delivery remains out of the inbox and redelivers at the same broker offset after restart;
7. the default-off deployment flag is not automatically enabled by evidence tooling.

Do not claim runtime completion from source verification, compilation or an environment skip. Do not start partition-wide replay, storefront cutover or a Product-specific DLQ protocol as part of this boundary.

## 6. M6/M7 gates remain unchanged

The following work remains gated:

- partition-scoped replay remains blocked until the owner source applies the partition predicate before keyset pagination rather than filtering after page selection;
- drift/reconciliation repair keeps the existing bounded source and recovery contracts;
- Product graph/storefront cutover keeps the existing readiness, relation-admission and parity gates;
- historical schema identities remain storage history only and must not become runtime fallback implementations.

## 7. Merge admission for this revision

Before merge, require all source/compile checks on the revision head:

```text
node scripts/verify/verify-index-contract-ci.mjs
node scripts/verify/verify-event-contract-digest-admission.mjs
node scripts/verify/verify-index-product-refresh-event-family.mjs
node scripts/verify/verify-index-product-refresh-delivery.mjs
node scripts/verify/verify-index-product-refresh-host-consumer.mjs
node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json
cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets
cargo check --locked -p rustok-distribution --features mod-product --lib
cargo check --locked -p rustok-server --no-default-features --features mod-product --lib
cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy
cargo test --locked -p rustok-events --test canonical_contracts
cargo test --locked -p rustok-index source_refresh_event --lib
cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib
cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib
```

The external execution command is documented in the evidence guide but is not part of source-only admission unless approved PostgreSQL/Iggy endpoints are supplied. The active cursor after this revision is retained execution of the exact evidence packet described in section 5.
