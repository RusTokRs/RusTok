# FORUM-33E mention reconciliation actualization — 2026-08-08

Status: `source-ready / maintainer-execution-open / repair-open`

## Rechecked baseline

The FORUM-33 source line is now complete through:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent counter keyset continuation;
- FORUM-33C: accepted-solution eligibility and solution-author stat reconciliation;
- FORUM-33D: bounded persisted topic/category subscription reconciliation.

FORUM-33D explicitly advanced the next source reconciliation cursor to mentions. The canonical implementation-plan ledger was also found to still describe subscriptions as open even though #3351 is already merged; this slice synchronizes the ledger with the actual mainline state.

## Existing mention owner rechecked

FORUM-12 owns immutable relation revisions plus user/audience mention snapshots. `MentionRelationService` resolves profile handles only during the owner write boundary through `ProfilesReader`, stores exact source identity, enforces the 32-target limit and writes semantic mention-added events from the exact persisted diff.

The persisted mention projection is intentionally not a copy of Profile or Notifications state. A later profile rename, privacy change or notification-delivery outcome must not rewrite historical mention snapshot identity.

## Read-only reconciliation owner

`ForumMentionReconciliationService` adds a bounded FORUM-33 diagnostic over Forum-owned persisted relation/mention state only.

It does not re-resolve handles through Profiles and does not query Notifications-owned inbox, fan-out, preference or delivery tables.

The service checks mention-bearing relation revisions for:

- `source_unavailable`: the exact relation source `(kind, id, locale)` no longer resolves through the same Forum source table used by the write guard;
- `child_source_mismatch`: one or more user/audience mention rows disagree with their authoritative relation revision source identity;
- `target_limit_exceeded`: persisted user plus audience targets exceed `FORUM_MAX_MENTION_TARGETS_PER_REVISION = 32`;
- `locale_invalid`: the persisted source locale is not the canonical normalized locale written by the owner;
- `projection_fingerprint_invalid`: the persisted owner fingerprint is neither the historical `legacy` sentinel nor one lowercase 64-character SHA-256 hex digest.

Relation revisions without persisted mention targets are traversed for keyset progress but are not classified by this mention-specific report.

## Bounded traversal

The source table already has one monotonic `BIGSERIAL`/SQLite integer relation revision identity. The report therefore uses strict keyset continuation:

```text
revision_id > relationAfter
```

The owner accepts an internal positive `i64` cursor. GraphQL carries that cursor as a positive decimal string rather than relying on GraphQL's 32-bit `Int` contract.

Every page uses the existing FORUM-33 default 100 / hard 500 limit and one `effective_limit + 1` lookahead row. `clean` is page-local; whole-tenant clean requires exhausting the relation cursor with every page clean.

## GraphQL admission

The current-tenant operator query is:

```text
forumMentionReconciliationReport(
  limit: Int,
  relationAfter: String
)
```

Tenant identity comes only from `TenantContext`. Auth/tenant mismatch is rejected. GraphQL requires both effective permissions:

```text
forum_categories:manage
forum_topics:manage
```

The owner service independently reauthorizes both scopes through the canonical Forum RBAC helper before database work so a later non-GraphQL adapter cannot bypass operator admission.

## Snapshot and observability

Each page executes under one database snapshot:

- PostgreSQL: `REPEATABLE READ READ ONLY`;
- SQLite: one transaction snapshot.

The service reuses platform module entrypoint, span duration, span error and module error telemetry. It adds no duplicate Forum-only metric family.

## Deliberately open

This slice adds no relation repair, mention deletion/rewrite, Profile reconciliation, notification delivery repair, migration, dependency change, lockfile update, CLI adapter or runtime evidence.

Generic write repair remains blocked on explicit repair RBAC, dry-run semantics, durable audit, idempotent job/receipt state, bounded retry/recovery and retained PostgreSQL/SQLite execution evidence.

The next FORUM-33 source reconciliation cursor is **attachments**, followed by permitted shared-owner projections and remaining non-duplicative operational metrics.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check:

```bash
node scripts/verify/verify-forum-mention-reconciliation-source.mjs
```
