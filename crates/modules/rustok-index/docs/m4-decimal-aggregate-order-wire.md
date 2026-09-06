# M4 Decimal aggregate order wire

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice enables Decimal as a terminal type for explicit many-link `min_*` / `max_*`
ordering while preserving an exact tagged `IndexValue` JSON contract.

The machine-readable contract is
`crates/rustok-index/contracts/m4-decimal-aggregate-order-wire.json`.

## Domain wire

`IndexValue::Decimal` uses the ordinary Serde representation owned by `rust_decimal`:

```json
{
  "type": "decimal",
  "value": "123.4500"
}
```

The JSON value is a string, not a JSON number. A source test serializes a scaled Decimal,
asserts the exact tagged payload, deserializes it, and reserializes it byte-semantically to the
same JSON value. No float conversion or alternate arbitrary-precision JSON feature is introduced.

## PostgreSQL wire

Stored Decimal fields are already read from the tagged JSON string and cast to PostgreSQL
`numeric` for filtering and ordering. Many-link aggregation continues to evaluate typed
`MIN(numeric)` or `MAX(numeric)` in the correlated scalar subquery.

The hidden `__order_N` handoff deliberately uses a different representation from the ordering
expression:

- `ORDER BY` uses the typed `numeric` aggregate;
- the tagged JSON value uses `to_jsonb((aggregate_scalar)::text)`;
- the resulting object remains `{ "type": "decimal", "value": "..." }`;
- the strict decoder therefore receives the same Decimal wire shape as an ordinary stored
  `IndexValue`.

This separation prevents conversion through a JSON number or `f64` while retaining numeric,
not lexical, ordering.

## Query boundary

Decimal is accepted under the same bounded aggregate policy as integer, string, and timestamp:

- the field path crosses at least one `many` link;
- the terminal field is scalar and sortable;
- the caller explicitly selects `min_asc`, `min_desc`, `max_asc`, or `max_desc`;
- pagination is bounded offset pagination.

Plain `asc` / `desc` through `many`, UUID aggregate ordering, singular aggregate paths, and
aggregate cursor continuation remain rejected.

An empty or all-null relation set still produces SQL `NULL`; ascending uses `NULLS LAST`,
descending uses `NULLS FIRST`, and root entity ID remains the deterministic final tie-break.

## Compatibility

The `OrderExpr`, `PlannedOrder`, compiled-column, and result DTO shapes are unchanged. Existing
ordinary v4 plan/SQL snapshots do not contain aggregate Decimal ordering and remain outside this
slice. Root/one-link Decimal filtering and ordering are unchanged.

No new dependency or feature flag is added. The contract relies on the existing `rust_decimal`
Serde string representation and existing PostgreSQL `numeric` casts.

## Non-claims

This source slice does not:

- enable aggregate cursor continuation;
- add Decimal scenarios to the independent PostgreSQL/reference fixture;
- run PostgreSQL or prove driver decoding;
- update or admit a retained equivalence bundle;
- change runtime composition, authorization, or consumer cutover.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index decimal_tagged_json_uses_exact_string_wire -- --nocapture
cargo test -p rustok-index decimal_aggregate_uses_numeric_order_and_exact_string_wire -- --nocapture
cargo test -p rustok-index aggregate_ordering -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-decimal-aggregate-wire.mjs
node scripts/verify/verify-index-many-link-aggregate-ordering.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
```

A later owner-operated PostgreSQL/reference scenario must persist Decimals with representative
scale and precision, execute `MIN` / `MAX` offset pages through `PostgresIndexQueryPort`, and
compare the decoded page before this path is considered execution proven.
