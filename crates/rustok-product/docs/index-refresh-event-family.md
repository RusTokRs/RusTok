# Product Index refresh typed event family

Status: `source_ready_digest_regeneration_pending`.

## Admission basis

The previously stale canonical event-contract digest baseline was regenerated, admitted through PR #3390, and
then regenerated again by the maintainer on the admitted commit `7983092f96e14c002c57451709de936e40c01356`
with an empty diff. The later mainline change before this source slice is Page Builder/Telemetry-only and does not
change `rustok-events`, Product or Index wire contracts.

That completes the baseline gate that previously blocked `ProductIndexRefreshEvent`. This source slice adds the
family itself. Its new digest values are intentionally **not** guessed: the same canonical generator must be run
again on this exact branch and the generated artifact must be committed in this reviewed wire-contract PR before
merge.

## Family

The closed `product_index_refresh` typed family has schema version `1` and exactly two event types:

```text
product.index.locale_refresh_requested
product.index.variant_refresh_requested
```

`LocaleRefreshRequested` contains only:

```text
product_id
locale
source_version
```

`VariantRefreshRequested` contains only:

```text
product_id
variant_id
source_version
```

`source_version` is the positive Product owner revision retained by the immutable refresh ledger and must fit the
positive PostgreSQL `int64` range. Locale is non-empty, bounded to 128 bytes, and carries no leading/trailing
whitespace.

## Identity and causation

The payload deliberately excludes tenant, actor, delivery identity and causation. The existing Product canonical
writer remains authoritative:

```text
id = correlation_id = refresh_id
causation_id = root_event_id
tenant_id = ledger tenant_id
actor_id = validated Product root envelope actor
```

The writer re-reads the causal root from `sys_events`, validates its registered Product lifecycle envelope and
requires the same tenant/Product before publishing. A payload cannot substitute a different identity or causal
predecessor.

## Product factory

`CanonicalProductIndexRefreshEventFactory` is the concrete Product-owned implementation of the existing
`ProductIndexRefreshEventFactory` relay seam. It maps immutable locale and variant ledger records directly to the
sealed `rustok-events::ProductIndexRefreshEvent` variants.

`ProductIndexRefreshContract` is implemented for that family inside `rustok-product`; its target projection is the
same fact set the canonical writer already compares against each ledger row. The relay still publishes at most one
row per explicit call and advances its durable cursor in the same transaction as the write-once outbox envelope.

No new loop, scheduler, retry owner, broker consumer, acknowledgement path or Index mutation route is introduced.

## Wire compatibility

This is the first and only repository-owned Product Index refresh typed family. There is no legacy decoder,
parallel v2 family, fallback payload, dual publication path or format compatibility layer. Future pre-release
changes replace the canonical contract in place unless an explicit external compatibility bridge is approved.

The old source-test-only event-domain string `product.index.product-locale-refresh-v1` is not adopted as the new
wire type. The canonical family carries schema version structurally through `EventContract`, consistent with the
rest of `rustok-events`, rather than embedding a format version in the event name.

## Remaining work after digest admission

This slice does **not** register Product/ProductVariant Index mutation routes, start an Outbox/Iggy consumer,
acknowledge source deliveries, add retry/DLQ behavior, switch Storefront traffic, or alter the M6 concrete-repair
gate. Those remain separate boundaries.

The next wire step after this PR is admitted is a concrete Product/ProductVariant typed-delivery adapter into the
existing generic `IndexSourceRefreshEventWorker`, with commit-before-ack behavior and source-route registration.

## Required maintainer generation

Run on the exact branch/head for this PR:

```bash
node scripts/verify/verify-event-contract-digest-admission.mjs
cargo run --locked -p rustok-events --example event_contract_digests -- --write
git diff -- crates/rustok-events/contracts/event-contract-digests.json
```

Then commit the generated `crates/rustok-events/contracts/event-contract-digests.json` in this same PR and run:

```bash
node scripts/verify/verify-index-product-refresh-event-family.mjs
cargo check -p rustok-events --all-targets
cargo check -p rustok-product --all-targets
git diff --check
```

The implementation agent does not claim those commands have run on this branch yet.
