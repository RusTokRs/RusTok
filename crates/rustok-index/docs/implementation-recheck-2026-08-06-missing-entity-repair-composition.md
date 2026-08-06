# Index implementation recheck — concrete missing-entity repair

Date: 2026-08-06.

Audited baseline: `main@3f9be66d3b3d3ed594ffa1f325b02db728212797`.

## Reviewed source scope

- `crates/rustok-index/src/infrastructure/postgres/drift_missing_entity_repair.rs`
- PostgreSQL and crate exports
- targeted-repair and concrete-composition documentation
- live implementation plan
- focused and aggregate static guards

## Source conclusions

The concrete composition supports only `MissingEntity` and rejects `OrphanLink` before delegating to
the generic durable reservation store. Unsupported targets cannot create `prepared` state through
this service.

The evidence reader:

- uses one exact `IndexSourceLoadRequest`;
- requires retained absence when ordinary load is empty;
- reads only exact `index_entities.source_version` and `is_deleted`;
- brackets the materialized read with two authoritative owner reads;
- rejects owner change as retryable;
- derives evidence digest only from typed identity, versions, observed owner/materialized shape, and
  admitted evidence state.

A live row is repairable only when the exact absence version is strictly newer than the exact indexed
version. Equal-version deletion does not bypass `PostgresMutationStore` monotonicity.

The concrete owner creates one `IndexMutation::Delete`, uses the durable repair command UUID as the
mutation event and inbox delivery identity, and applies it through `PostgresMutationStore` with the
existing `SchemaRegistry`. It contains no direct Index write SQL.

The retry phase was reviewed explicitly. An exact tombstone at the admitted absence version is:

- `Repairable` during `capture_before`, allowing the same command UUID to reach the inbox duplicate
  path after a crash between mutation commit and repair receipt;
- `Converged` only during `capture_after`.

This avoids terminally misclassifying a committed mutation retry as
`NotRepaired(before_not_repairable)` while preserving the generic before-owner-after order.

The owner receipt digest binds command UUID, finding UUID, exact entity identity, committed versions,
and typed mutation outcome. `Applied`, `Duplicate`, and `StaleIgnored` never prove success by
themselves; the generic service still requires exact converged after evidence.

## Fail-closed boundaries retained

- no default or allow-all authorizer;
- no orphan-link mutation owner;
- no caller payload, SQL, arbitrary digest, or mutation JSON;
- no `ModuleRuntimeExtensions` insertion;
- no GraphQL, HTTP, CLI, MCP, native-admin, scheduler, worker, or automatic finding iteration;
- no automatic finding lifecycle transition;
- no direct update of finding, lifecycle, repair-receipt, entity, or link rows outside established
  stores.

## Known limits

Owner source reads and the PostgreSQL Index mutation do not share one transaction. The double owner
read and exact after evidence detect source movement, but they cannot prevent an owner change in the
small interval after before evidence and before mutation commit. A newer already-materialized Index
version fences the delete as stale; otherwise the after read fails closed and does not record
`Repaired`. No atomic source/Index claim is made.

A durable `prepared` command still has no lease, active-owner heartbeat, expiry, abandonment,
takeover, or operator recovery decision. Exact command retry is supported, but ambiguous attempts are
not silently expired or discarded.

A physically missing Index row is not admitted as convergence. This slice requires the exact retained
tombstone version so the repair receipt is tied to the canonical monotonic mutation result.

## Validation disclosure

This was a source-only review. No tests, Node verifiers, formatting, Cargo checks, PostgreSQL/SQLite
scenarios, migrations, workflows, or CI were run. Compile, migration, concurrency, runtime, and
production behavior are not claimed.
