# `rustok-index` Current Implementation Plan — 2026-08-12

Status: `m5_product_refresh_typed_delivery_boundary`

This document supersedes `implementation-plan-current-2026-08-09.md` as the active execution cursor for `rustok-index`.

## 1. Rechecked baseline

The baseline for this update is `main@ac36c04c732e9fdf23f2de3d917faf79e0552f3f` on 2026-08-12.

The previously completed Index foundation remains present and is not reopened:

- M1–M4 generic schema/query/storage boundaries remain the current architecture.
- PostgreSQL source factories still own Product and ProductVariant authoritative replay loading.
- durable inbox/source-version monotonicity remains the write-side idempotency boundary.
- the generic `IndexSourceRefreshEventWorker` still performs exact source load, durable mutation application and acknowledgement in that order.
- the canonical Product refresh owner ledger, writer, durable relay and typed event family remain the only Product refresh wire contract.
- the Product refresh event family remains exactly:
  - `product.index.locale_refresh_requested`
  - `product.index.variant_refresh_requested`
- Product currently routes as `rustok-product::product@4`.
- ProductVariant currently routes as `rustok-product::product_variant@2`.
- no legacy/v2/fallback event family is admitted.

## 2. Focused CI gate recheck

PR #3442 installed the focused `Index Contract CI` workflow.

The former hosted-runner blocker is closed: workflow run `31574291051` completed successfully on 2026-08-12. This satisfies the previous plan's admission condition for continuing M5 delivery work.

The current baseline run `31593587184` exposed a separate canonical digest artifact drift:

- source-contract verification passed;
- `cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets` passed;
- 13 of 14 canonical event contract tests passed;
- only `published_event_contract_matches_committed_release_artifact` failed;
- the canonical digest regeneration job produced the exact replacement digest artifact committed by this revision.

Therefore the digest update in this revision is a prerequisite repair, not a new compatibility contract.

The first distribution-focused admission run for this revision also surfaced a pre-existing compile defect in an always-on `rustok-distribution` dependency: `rustok-auth` migration `m20260721_000009_move_oauth_app_copy_to_translations.rs` used `#[derive(DeriveIden)]` with the `#[iden = ...]` helper, while the supported migration pattern in the repository is `#[derive(Iden)]`. The revision repairs that one derive line before evaluating the Product bridge. No migration SQL, table identity or data transformation is changed by that prerequisite repair.

## 3. M5 boundary implemented by this revision

### 3.1 Exact Product refresh routes

The selected Product distribution now registers two immutable Index mutation-event routes:

| Owner event domain | Exact replay source | Exact Index schema |
| --- | --- | --- |
| `product.index.locale_refresh_requested` | `product-postgres-primary` | `rustok-product::product@4` |
| `product.index.variant_refresh_requested` | `product-variant-postgres-primary` | `rustok-product::product_variant@2` |

Materialization remains fail-closed through `IndexMutationEventCatalog`: owner, source name and exact schema must agree with the registered source contract.

### 3.2 Canonical typed delivery projection

`rustok-distribution` now owns the Product-specific projection into the generic Index refresh delivery.

Locale refresh projects to:

- `tenant_id` from the owner delivery;
- schema `rustok-product::product@4`;
- `entity_id = product_id`;
- locale parsed and canonicalized through `LocaleKey`;
- `minimum_source_version = source_version`;
- the broker acknowledgement token remains opaque.

Variant refresh projects to:

- `tenant_id` from the owner delivery;
- schema `rustok-product::product_variant@2`;
- `entity_id = variant_id`;
- `locale = None`;
- `minimum_source_version = source_version`;
- `product_id` is still validated as a non-nil owner identity;
- the broker acknowledgement token remains opaque.

### 3.3 Commit-before-ack remains generic

The Product-specific bridge delegates to `IndexSourceRefreshEventWorker`; it does not duplicate persistence or acknowledgement logic.

The generic invariant remains:

1. resolve exact event route;
2. resolve exact authoritative source;
3. load exactly one target key;
4. reject missing, ambiguous or behind source state;
5. rebind the replay mutation to the owner event UUID;
6. durably apply the mutation through the replay mutation sink;
7. acknowledge only after durable application succeeds.

Redelivery after acknowledgement failure remains safe through the existing inbox and source-version rules.

## 4. Focused admission added with this boundary

`Index Contract CI` now also watches the Product distribution bridge and runs:

```text
node scripts/verify/verify-index-product-refresh-delivery.mjs
cargo check --locked -p rustok-distribution --features mod-product --lib
cargo test --locked -p rustok-index source_refresh_event --lib
cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib
```

The verifier pins the two canonical event domains, exact Product/ProductVariant schema identities, exact source routes, canonical locale conversion and the generic durable-apply-before-ack ordering.

## 5. Next M5 execution boundary

After this typed-delivery boundary is admitted, the next implementation slice is the concrete host transport/runtime consumer.

It must:

1. consume the canonical `ContractEventEnvelope` Product refresh family from the selected broker transport;
2. map only the two canonical variants into `ProductIndexRefreshDelivery`;
3. retain the broker-owned acknowledgement token without deriving it from the logical event UUID;
4. call `ProductIndexRefreshDeliveryWorker` with the materialized schema/source/event registries;
5. acknowledge only through the generic worker after durable Index persistence;
6. leave failed, missing, ambiguous, stale-source and transient persistence deliveries unacknowledged for broker retry;
7. define bounded startup/shutdown and retry observability at the host boundary;
8. add no second Product refresh wire format and no compatibility fallback.

Do not start partition-wide replay or a second mutation ingestion path as part of this slice.

## 6. M6/M7 gates remain unchanged

The following work remains gated and is not implicitly opened by the Product delivery bridge:

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
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff --exit-code -- crates/rustok-events/contracts/event-contract-digests.json
cargo check --locked -p rustok-events -p rustok-product -p rustok-index --all-targets
cargo check --locked -p rustok-distribution --features mod-product --lib
cargo test --locked -p rustok-events --test canonical_contracts
cargo test --locked -p rustok-index source_refresh_event --lib
cargo test --locked -p rustok-distribution --features mod-product product_index::refresh_event::tests --lib
```

The active cursor after this revision is the concrete broker/host consumption boundary described in section 5.
