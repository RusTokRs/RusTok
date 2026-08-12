# `rustok-index` Current Implementation Plan — 2026-08-12

Status: `m5_product_refresh_redelivery_manual_runner_source_ready`

This document supersedes `implementation-plan-current-2026-08-09.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The current execution baseline for this revision is `main@934069fb4a65d207aeba84bdde1e89f8444aaba8` on 2026-08-12. PR #3457 previously merged the source-ready Product refresh PostgreSQL/Iggy redelivery evidence packet. The intervening mainline delta is unrelated to this runner slice except that the `rustok-media` WebP compile break carried by the superseded draft is already fixed in current `main` by PR #3463, so this revision intentionally carries no Media change. PR #3465 then advanced `main` with Forum/Page Builder evidence only; those files do not overlap this Index runner slice.

The completed Index foundation remains present and is not reopened:

- M1–M4 generic schema/query/storage boundaries remain the current architecture.
- PostgreSQL source factories own Product and ProductVariant authoritative replay loading.
- durable inbox/source-version monotonicity remains the write-side idempotency boundary.
- `IndexSourceRefreshEventWorker` resolves the exact source, applies the durable mutation and acknowledges in that order.
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

## 3. M5 redelivery evidence source

The bounded cross-adapter executable proof source remains:

```text
apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs
```

It uses public production boundaries for the behavior under evidence:

- `IggyTransport` and `PersistentContractConsumerGroup` for broker delivery and offset acknowledgement;
- canonical `ContractEventEnvelope` / `ProductIndexRefreshEvent` publication;
- materialized Product/ProductVariant PostgreSQL Index sources;
- `ProductIndexRefreshDeliveryWorker` and the generic source-refresh worker;
- production `PostgresMutationStore` and durable `index_inbox` deduplication.

The harness requires explicit evidence-scoped PostgreSQL and external Iggy settings. It has no generic `DATABASE_URL`, localhost, credential or broker fallback. Missing settings on a direct developer invocation may produce a source-friendly skip; a skip is not runtime evidence.

### 3.1 Locale post-persistence ACK failure and restart

The first scenario publishes canonical locale and variant refresh envelopes through Iggy. For the locale delivery, the evidence acknowledger mutates only a clone of the broker acknowledgement token. The real consumer-group ACK fails after the generic worker has already committed the Product mutation.

The scenario requires:

- Product `index_entities` state and one applied `index_inbox` identity after the injected ACK failure;
- consumer/transport restart returning the same uncommitted broker offset and exact raw envelope bytes;
- redelivery resolving as `IndexReplayMutationOutcome::Duplicate` through the durable inbox;
- exactly one applied inbox identity remaining;
- successful acknowledgement advancing the group to the queued ProductVariant event;
- ProductVariant materializing as schema version 2 with the non-localized key and one applied inbox identity.

### 3.2 Behind-source restart

The second isolated scenario publishes a canonical Product locale refresh whose minimum owner `source_version` is one revision above the visible authoritative Product source.

The generic worker must return `SourceVersionBehind` before persistence or acknowledgement. The harness requires no matching `index_inbox` row, restarts the Iggy transport/group and requires the same uncommitted offset and exact raw payload again.

Missing-source behavior remains pinned by the existing generic source-refresh tests.

## 4. Evidence contract, source admission and manual execution runner

The machine-readable source contract is:

```text
crates/rustok-index/contracts/evidence/product-refresh-postgres-iggy-source.json
```

The operator guide is:

```text
crates/rustok-index/docs/m5-product-refresh-postgres-iggy-redelivery-evidence.md
```

The source verifier is:

```text
scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
```

`Index Contract CI` keeps source-only admission:

```text
node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs
cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy
```

This revision adds a dedicated maintainer-owned manual runner:

```text
.github/workflows/index-product-refresh-redelivery-evidence.yml
```

The runner is intentionally stricter than a direct developer invocation:

1. `workflow_dispatch` is its only trigger;
2. the operator must explicitly select `execute`;
3. repository permissions are read-only and checkout credentials are not persisted;
4. `RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL` and `RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS` must be configured as GitHub secrets;
5. optional Iggy username/password must either both be configured or both omitted;
6. missing confirmation or required secrets fails before the harness starts instead of producing a successful skip;
7. the workflow never sets or enables `RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED`;
8. it runs the exact source verifier and the exact external `cargo test` command from the evidence contract.

`Index Contract CI` watches the manual workflow source, so runner drift is rejected by the normal source/compile gate. The manual workflow itself is never executed automatically by push or pull request.

## 5. Runtime evidence status and next M5 execution boundary

The evidence status remains **runtime execution pending**. Adding a safe manual runner does not promote the claim.

The next retained M5 boundary is now operational and concrete:

1. merge the reviewed manual runner source;
2. configure operator-approved evidence-scoped PostgreSQL/Iggy GitHub secrets;
3. dispatch `Index Product Refresh Redelivery Evidence` with confirmation `execute` against the reviewed source commit;
4. require the workflow to complete successfully without a skip;
5. retain the exact run id, source SHA and result as reviewed runtime evidence;
6. only then update the machine contract/plan from `runtime_execution_pending` to the appropriate runtime-proven state.

A promotable execution must demonstrate all of the following on one reviewed source commit:

1. locale refresh reaches PostgreSQL before the injected broker ACK failure;
2. restart returns the same uncommitted Iggy offset and raw payload;
3. locale redelivery resolves through durable inbox dedup without a second logical mutation;
4. successful ACK advances the same production consumer group to ProductVariant;
5. ProductVariant materializes through its authoritative source and non-localized key;
6. a behind-source delivery remains out of the inbox and redelivers at the same broker offset after restart;
7. the default-off deployment flag is not automatically enabled by evidence tooling.

Do not claim runtime completion from source verification, compilation, creation of the manual runner or an environment skip. Do not start partition-wide replay, storefront cutover or a Product-specific DLQ protocol as part of this boundary.

## 6. M6/M7 gates remain unchanged

The following work remains gated:

- partition-scoped replay remains blocked until the owner source applies the partition predicate before keyset pagination rather than filtering after page selection;
- drift/reconciliation repair keeps the existing bounded source and recovery contracts;
- Product graph/storefront cutover keeps the existing readiness, relation-admission and parity gates;
- historical schema identities remain storage history only and must not become runtime fallback implementations.

## 7. Merge admission for this revision

Before merge, require all focused source/compile checks on the revision head and PR merge-ref:

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

The external runtime execution is deliberately separate from source-only admission. After this revision merges, the active cursor remains the operator-approved `workflow_dispatch` execution and retained run evidence described in section 5.
