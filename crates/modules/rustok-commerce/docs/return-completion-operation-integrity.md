# Return completion operation journal integrity

Status: **source-ready / unvalidated**

## Scope

This slice audits the durable journal in `services/return_completion_operation.rs`, including request identity, lease bounds, CAS transitions, tenant binding, and database-level state invariants.

## Confirmed hardening

- Runtime admission already normalizes `request_hash` to exactly 64 lowercase hexadecimal characters.
- The original operation table allowed any non-empty hash at the database layer, unlike the command inbox.
- The original database guards allowed impossible durable combinations such as `pending` at a non-`created` stage or `completed` at a non-`completed` stage.
- The forward-only hardening migration enforces these invariants on PostgreSQL, MySQL, and SQLite.
- MySQL uses `REGEXP_LIKE(..., 'c')` so the lowercase hexadecimal invariant is not weakened by a case-insensitive collation.
- Lease owner and lease duration remain bounded by the existing journal normalizers.
- Runtime claim/checkpoint/finish operations remain tenant-scoped and lease/CAS guarded; no replay or external-effect semantics were changed.

## Migration policy

The new migration is intentionally forward-only with explicit backend branches. It does not rewrite existing rows or silently repair corrupted identities. An upgraded database containing rows that violate the newly enforced invariants will fail migration rather than silently normalize durable operation identity.

## Validation disclosure

Source inspection and static guards were added. No Cargo, migration runner, test suite, runtime, or CI command was executed by the agent.
