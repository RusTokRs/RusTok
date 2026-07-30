# Social Graph relation-event replay CLI

Date: 2026-07-30

Status: `source_complete_execution_pending`

The owner-local command is:

```text
social_graph relation-event-replay
```

It exposes the existing `SocialGraphRelationEventMaintenancePort` through the canonical CLI registry. The adapter does not read `social_graph_relations`, construct SQL, or publish outbox rows directly. `SocialGraphRelationEventMaintenanceService::with_outbox` composes the owner service with the canonical transactional outbox inside `rustok-social-graph`.

## Required and optional options

- `--tenant-id <uuid>` is mandatory and selects exactly one tenant.
- `--after-relation-id <uuid>` is optional. Omit it for the first page; use only the returned `next_after_relation_id` to continue.
- `--limit <1..1000>` is optional and defaults to `100`.
- `--dry-run` validates and selects the page without publishing any events.

The command uses a system actor, a 30-second deadline, and a stable idempotency key derived from tenant, cursor, limit, and dry-run state. User actors are rejected by the owner port. The owner query remains tenant-scoped, ordered by relation UUID, and bounded to one page.

## Output

The result includes:

- `tenant_id`;
- `after_relation_id`;
- `dry_run`;
- `limit`;
- `selected_relations`;
- `published_events`;
- `next_after_relation_id`.

A non-null `next_after_relation_id` identifies the last selected row. Operators may invoke the next page explicitly with that cursor. The command never loops over all pages automatically and never collects all relation IDs first.

## Transaction and replay semantics

For a non-dry-run page, every authoritative persisted relation revision is converted to the existing replayable Social Graph state fact and published through the transactional outbox in the same database transaction. Any publication failure rolls back the whole page. Repeating a page may redeliver the same revision fact; downstream Index mutation storage remains responsible for inbox deduplication and monotonic source-version handling.

This command provides an operational repair entrypoint. It does not prove projection freshness, automatically reconcile drift, authorize Index privacy reads, or replace retained parity and outage-recovery evidence.

## Suggested validation

```bash
cargo test -p rustok-social-graph-cli -- --nocapture
cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture
node scripts/verify/verify-social-graph-relation-event-replay.mjs
cargo xtask module validate social_graph
```
