# FORUM-23B2G2A1 Search owner-revision counter hardening

## Status

`source_complete_execution_pending`

`FORUM-23B2G2A` introduced the Forum-owned tenant projection revision counter and
append-only revision-to-outbox ledger. This follow-up closes direct SQL mutation
paths that were still protected only by the owner allocator convention.

The delivered baseline migration
`m20260731_000007_add_forum_projection_revision_ledger` remains unchanged. All
hardening is applied through additive migration
`m20260731_000008_harden_forum_projection_revision_counter`.

## Counter invariants

PostgreSQL row triggers enforce:

- the first row for one tenant must start at revision `1`;
- an update must retain the same tenant key;
- an update must advance the previous revision by exactly `1`;
- row deletion is forbidden.

The trigger remains compatible with the canonical owner allocator:

```sql
INSERT ... VALUES (tenant_id, 1)
ON CONFLICT (tenant_id)
DO UPDATE SET revision = revision + 1
RETURNING revision
```

The baseline positive check remains active. This migration adds no alternate
allocator and does not change owner service code.

## Truncation boundary

The original G2A ledger rejected row updates and deletes, but PostgreSQL
`TRUNCATE` does not execute row-level delete triggers. A statement trigger now
rejects truncation of both:

```text
forum_projection_revision_counters
forum_projection_revision_ledger
```

Without this guard, direct maintenance SQL could reset the tenant clock or erase
the immutable revision-to-event audit trail while satisfying all row-level
triggers.

## Migration compatibility

The hardening migration is PostgreSQL-only, matching the owner ledger and Search
Forum projection runtime. SQLite remains a no-op.

Down migration removes only the new counter and truncate triggers/functions. It
does not drop, rewrite or truncate either table, and it does not modify the
baseline G2A migration.

No owner allocator, outbox API, event schema, Search ingestion, Search inbox,
public API, dependency or `Cargo.lock` change is introduced.

## Deliberate boundary

This slice does not decode or validate `sys_events.payload` from a Forum
migration. Exact outbox envelope identity remains guaranteed by the G2A owner
transaction, unique ledger event ID and canonical outbox writer. Coupling a Forum
schema trigger to outbox JSON serialization would create a cross-module storage
contract and is deferred rather than hidden inside database SQL.

The versioned owner-revision event, persistent Search consumer, dual watermark
commit protocol and lost-delivery reconciliation remain `FORUM-23B2G2B`.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
node scripts/verify/verify-forum-search-owner-revision-counter-hardening.mjs
cargo test -p rustok-forum projection_revision -- --nocapture
cargo check -p rustok-forum --all-targets
cargo xtask module validate forum
```

PostgreSQL evidence should attempt initial revision values other than `1`, skipped
and repeated revisions, tenant-key mutation, row deletion, counter truncation and
ledger truncation. It should also confirm that the canonical concurrent
`INSERT ... ON CONFLICT` allocator continues to commit exact increasing tenant
revisions.
