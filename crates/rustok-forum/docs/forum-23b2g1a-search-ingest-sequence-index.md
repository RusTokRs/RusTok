# FORUM-23B2G1A Forum Search ingest-sequence lookup index

## Status

`source_complete_execution_pending`

This hardening slice adds an upgrade-safe PostgreSQL access path for the durable
Forum Search inbox ordering delivered in `FORUM-23B2G1`.

The original `000010` migration is already part of the migration history and is
left byte-for-byte unchanged. A new additive `000011` migration ensures that
existing installations receive the lookup index instead of relying on a modified
historical migration that would never rerun.

## Query ownership

Two existing owner queries require the same ordering prefix:

- `ForumProjectionInbox::claim_next` filters one tenant and the Forum source,
  keeps only pending/retryable rows and claims the lowest `ingest_sequence`;
- `ForumProjectionReconciler::due_tenants` finds the lowest non-terminal sequence
  per tenant, then orders the bounded tenant set by that oldest sequence.

The migration creates:

```sql
CREATE INDEX idx_search_projection_inbox_due_ingest_sequence
ON search_projection_inbox (source_module, tenant_id, ingest_sequence)
WHERE status IN ('pending', 'retryable_error');
```

The partial predicate excludes completed, skipped and dead-letter rows. The key
order supports the exact Forum source, per-tenant claim order and the
`DISTINCT ON (tenant_id) ... ORDER BY tenant_id, ingest_sequence` due-tenant
selection without changing either query or its semantics.

## Constraint repair

`000011` also rechecks the positive inbox-sequence and non-negative watermark
constraints. Unlike the historical migration, each existence check includes the
owning relation through `pg_constraint.conrelid`. A same-named constraint on an
unrelated table therefore cannot suppress the required Search constraint.

The down migration removes only the new lookup index. The sequence columns and
constraints belong to `000010` and remain intact until that predecessor migration
is rolled back.

## Compatibility

No event, Forum owner, inbox claim, retry, watermark, projection, public API,
dependency or `Cargo.lock` behavior changes. SQLite remains unchanged and does not
claim background Forum projection reconciliation.

This is query-plan hardening for the Search-issued ingest sequence. It is not the
final Forum-owner-issued monotonic projection revision.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
node scripts/verify/verify-forum-search-ingest-sequence-index.mjs
cargo check -p rustok-search --all-targets
cargo xtask module validate search
cargo xtask module validate forum
```

PostgreSQL evidence should apply migrations through `000011`, retain the index
DDL, and capture `EXPLAIN (ANALYZE, BUFFERS)` for both the per-tenant claim query
and the bounded due-tenant query with enough tenants and terminal rows to show the
partial access path. Runtime planner selection remains pending until that evidence
is recorded.
