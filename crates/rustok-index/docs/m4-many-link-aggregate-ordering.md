# M4 many-link aggregate ordering

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice replaces the previous blanket rejection of all many-link ordering with one explicit,
bounded policy. Ordinary `asc` and `desc` over a path that crosses a `many` link remain
ambiguous and continue to fail closed. Callers must select one of four serialized modes:

- `min_asc`;
- `min_desc`;
- `max_asc`;
- `max_desc`.

The `OrderExpr` shape is unchanged. Existing `asc` / `desc` payloads, source literals, and
legacy plan/cursor discriminants therefore retain their old representation.

The machine-readable contract is
`crates/rustok-index/contracts/m4-many-link-aggregate-ordering.json`. Decimal's exact hidden-order
handoff is separately fixed by
`crates/rustok-index/contracts/m4-decimal-aggregate-order-wire.json`.

## Validation policy

Aggregate ordering is accepted only when all of the following are true:

1. the field path crosses at least one `LinkCardinality::Many` edge;
2. the terminal field has scalar cardinality, is sortable, and has type `integer`, `decimal`,
   `string`, or `timestamp`;
3. pagination is bounded offset pagination under the existing limit/depth caps.

Aggregate ordering on a root or one-link-only path is rejected. Plain `asc` / `desc` over a
many path is still rejected as `AmbiguousManyLinkSort`. Boolean, UUID, list-valued, and
unsortable fields are rejected. UUID remains outside the bounded PostgreSQL `MIN` / `MAX`
contract.

Decimal uses an exact split wire. PostgreSQL evaluates `MIN` / `MAX` and `ORDER BY` through the
typed `numeric` scalar. The hidden `__order_N` tagged payload converts that scalar through
`numeric::text`, producing a JSON string that matches `IndexValue::Decimal` Serde. It never
passes the Decimal through a JSON number or float conversion.

Aggregate cursor continuation remains open. This slice deliberately does not reinterpret the
existing cursor envelope or silently encode a derived many-link value into a legacy cursor.

## Plan and compiler contract

`SchemaRegistry::plan_query` uses the aggregate-aware validation boundary while preserving the
legacy `QueryPlanError::Validation` and `QueryPlanError::Registry` mapping for old queries.
Only new aggregate-policy failures use `QueryPlanError::AggregateValidation`.

The existing `PlannedOrder` shape remains unchanged. An aggregate order field is marked
nullable in the plan because an empty or all-null relation set produces a derived SQL `NULL`,
even when every present terminal value is non-nullable. Existing non-aggregate v4 plan bytes
are unchanged because the old enum variants remain first and no plan fields were added.

The compiler independently rejects:

- a many-path order without an explicit aggregate;
- an aggregate on a singular path;
- a forged aggregate plan with an unsupported value type;
- a forged aggregate cursor plan;
- aggregate metadata that does not match the registry-derived field contract.

## PostgreSQL semantics

For every aggregate order expression the compiler emits a correlated scalar subquery rooted at
the current root entity. It walks the complete relation path and evaluates `MIN` or `MAX` over
the typed terminal scalar. SQL aggregate null semantics are intentional:

- null terminal values do not participate in `MIN` / `MAX`;
- an empty relation set or a set containing only null values yields SQL `NULL`;
- ascending order uses `NULLS LAST`;
- descending order uses `NULLS FIRST`;
- root `entity_id ASC` remains the deterministic final tie-break.

The aggregate relation does not enter the outer rowset, so root cardinality, exact count, and
bounded offset pagination remain duplicate free. The selected `__order_N` column is encoded as
tagged `IndexValue` JSONB for the existing strict decoder, while `ORDER BY` uses the typed
scalar expression. Decimal is the only supported aggregate type whose tagged value uses a JSON
string derived from typed text; the ordering expression itself remains `numeric`.

## Compatibility boundary

The two test-only reference engines normalize physical direction through
`OrderDirection::base_direction()` so adding explicit modes cannot create an exhaustive-match
compile break. Their aggregate result materialization is not extended in this slice because
aggregate reference equivalence is still owner-execution work.

This slice does not:

- enable aggregate cursor pagination;
- enable UUID aggregate ordering;
- change ordinary root or one-link ordering;
- choose the first related row, link ordinal, or storage order as an implicit aggregate;
- add `array_agg` or caller-defined SQL;
- run PostgreSQL or update the retained PostgreSQL/reference evidence bundle;
- change Social Graph privacy authority, freshness policy, or any consumer cutover.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index decimal_tagged_json_uses_exact_string_wire -- --nocapture
cargo test -p rustok-index decimal_aggregate_uses_numeric_order_and_exact_string_wire -- --nocapture
cargo test -p rustok-index aggregate_ordering -- --nocapture
cargo test -p rustok-index aggregate_ordering_tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-decimal-aggregate-wire.mjs
node scripts/verify/verify-index-many-link-aggregate-ordering.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo xtask module validate index
```

A later slice should extend the independent reference fixture plus retained
PostgreSQL/reference capture with Decimal and other aggregate offset scenarios before aggregate
ordering is treated as execution proven. Cursor support should be designed and admitted
separately.
