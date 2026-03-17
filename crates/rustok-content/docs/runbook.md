# rustok-content Runbook

Operational procedures for `rustok-content` in production and staging environments.

---

## Migration rollback

### Orchestration tables (Phase 1)

Tables introduced: `content_orchestration_operations`, `content_orchestration_audit_logs`.

**Rollback procedure:**

```sql
-- Run migration down (handled automatically by SeaORM migration runner)
-- Equivalent manual steps:
DROP TABLE IF EXISTS content_orchestration_audit_logs;
DROP TABLE IF EXISTS content_orchestration_operations;
```

The `down()` function in `m20260311_000001_create_content_orchestration_tables.rs` drops both
tables safely. Rollback is safe only when no in-flight orchestration operations exist.

**Pre-rollback checklist:**
1. Verify no active promote/demote/split/merge operations are running.
2. Export audit log data if needed for compliance: `SELECT * FROM content_orchestration_audit_logs;`
3. Run `sea-orm-cli migrate down` or the equivalent migration runner command.

---

## Reindex procedure

### Full reindex (all tenants)

Used after schema changes, indexer bug fixes, or data corrections.

```bash
# Trigger full reindex via admin API (when available)
curl -X POST /admin/content/reindex \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

Until the admin API endpoint is implemented, reindex is triggered by replaying
`NodeCreated` / `NodeUpdated` events for all nodes. See `rustok-index` crate for
`ContentIndexer::index_all`.

### Per-locale failure handling

`ContentIndexer::index_one` isolates per-locale failures:
- A single locale failure logs a `WARN` and continues.
- The operation only fails if **all** locales fail for a given node.

**Monitoring:** Watch for `warn!` log entries with fields `node_id`, `locale`, `error`
from the `rustok_index::content::indexer` target.

**Recovery:** Re-trigger indexing for a specific node by publishing a synthetic
`NodeUpdated` event or running a targeted reindex command.

---

## RBAC enforcement

### Permission scopes

| Role | Nodes (own) | Nodes (any) | Moderate |
|------|-------------|-------------|----------|
| Admin | Update/Delete | Update/Delete | Yes |
| Manager | Update/Delete | Update/Delete | Yes |
| Customer | Read | None | No |
| Guest | None | None | No |

`PermissionScope::None` on any write action returns `ContentError::Forbidden`.

### Orchestration permissions

All four orchestration operations (`promote_topic_to_post`, `demote_post_to_topic`,
`split_topic`, `merge_topics`) require `Action::Moderate` + `Action::Create`.

---

## Status transition rules

Valid transitions enforced by `validate_status_transition` in `node_service.rs`:

| From | To | Method |
|------|----|--------|
| Draft | Published | `publish_node` |
| Published | Draft | `unpublish_node` |
| Published | Archived | `archive_node` |
| Archived | Draft | `restore_node` |

All other transitions return `ContentError::Validation("Invalid status transition: ...")`.

---

## Metadata depth guard

`metadata` JSON fields are limited to 5 levels of nesting (`METADATA_MAX_DEPTH = 5`).
Payloads exceeding this return `ContentError::Validation("metadata exceeds maximum nesting depth")`.

---

## Sanitizer coverage

All orchestration command payloads are sanitized via:
- `ensure_safe_text` / `ensure_safe_optional_text` — rejects unsafe HTML/script content.
- `ensure_idempotency_key` — validates idempotency key format.
- `json_object_depth` — enforces metadata nesting limit.

---

## Alerts and thresholds

| Metric | Alert threshold |
|--------|----------------|
| `content.node.create` error rate | > 1% over 5m |
| `content.node.update` error rate | > 1% over 5m |
| `content.node.publish` error rate | > 0.5% over 5m |
| Reindex partial failures (warn) | Any occurrence |

Metrics are emitted via `rustok-telemetry::metrics::record_span_duration` and
`record_span_error` with labels from `ContentError::kind()`.
