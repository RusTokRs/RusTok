# Product to SalesChannel relation admission

Status: `canonical_source_complete_freshness_and_runtime_evidence_pending`

The current Product Index graph already contains the Product-to-SalesChannel link. There is no future
Product compatibility schema waiting to add it.

## Relation owner contract

`rustok-product` owns `product_sales_channel_index_relation_snapshots` and
`ProductSalesChannelIndexRelationStore`.

For each tenant/Product identity the append-only ledger stores a positive contiguous `relation_epoch`
and the complete resolved SalesChannel UUID set. Membership is canonical, bounded, idempotent, and
retained independently from Product and Channel physical rows.

The writer locks the live Product and exact relation identity before append. Product hard delete uses
the same ordering and retains an empty relation epoch when necessary, so a stale non-empty relation
cannot be appended after deletion.

The Product crate does not query Channel tables and does not depend on `rustok-channel` or
`rustok-index`.

## Cross-owner resolution

`rustok-distribution::product_index::channel_relation_resolver` converts Product visibility metadata
into current tenant SalesChannel UUID membership.

Policy:

- missing visibility or an empty canonical allowlist means unrestricted visibility;
- unrestricted visibility resolves to every current tenant Channel identity;
- restricted visibility matches canonical `lower(btrim(slug))` membership;
- malformed/non-canonical Product visibility fails closed;
- Channel `is_active` does not alter relation identity membership;
- resolver pages and membership sets are bounded;
- exact Product resolution uses observe/write/re-observe stabilization with at most three attempts.

This resolver is a convergence primitive, not a continuous consumer or durability proof.

## Complete Product graph clock

Product state and resolved relation membership have independent clocks. The complete Product Index
record therefore uses neither component counter directly.

`product_index_graph_projection_snapshots` retains both Product and relation watermarks and advances a
separate contiguous `projection_epoch` whenever either retained component advances. The canonical
Product Index source uses that projection epoch as its only full-record `source_version`.

The canonical Product source joins the exact relation row referenced by projection state and emits:

- `sales_channel_ids` as a current graph field;
- a many-cardinality `sales_channels` `IndexLink` to SalesChannel identity.

Product locale absence uses the same projection clock.

Detailed projection contract:
`crates/rustok-product/docs/index-graph-projection-ledger.md`.

## Freshness boundary

Monotonic projection ordering does not prove that the relation resolver has already converged the
newest Product visibility or Channel identity change. Durable Product/Channel convergence triggering
or an admitted freshness watermark remains required before authoritative Product graph use.

## Production admission status

1. Durable relation epoch storage: source complete.
2. Bounded resolved membership discovery: source complete.
3. Atomic membership + relation epoch commit: source complete.
4. Retained empty membership on Product delete: source complete.
5. Complete Product graph projection epoch: source complete.
6. Canonical Product `sales_channels` link and projection-aware replay/absence: source complete.
7. Durable Product/Channel convergence trigger or freshness watermark: pending.
8. Product typed event route/consumer after event-contract digest admission: pending.
9. PostgreSQL concurrency/restart/retry/delete-recreate/out-of-order/locale evidence: pending.
10. Storefront/production cutover: pending.

## Maintainer validation

```bash
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
