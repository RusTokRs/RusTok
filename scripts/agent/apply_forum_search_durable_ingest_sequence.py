from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    write(path, content.replace(old, new, 1))


# Migration registry.
path = "crates/rustok-search/src/migrations/mod.rs"
replace_once(
    path,
    "mod m20260730_000009_create_search_projection_inbox;",
    "mod m20260730_000009_create_search_projection_inbox;\nmod m20260731_000010_add_forum_projection_ingest_sequence;",
)
replace_once(
    path,
    "        Box::new(m20260730_000009_create_search_projection_inbox::Migration),",
    "        Box::new(m20260730_000009_create_search_projection_inbox::Migration),\n        Box::new(m20260731_000010_add_forum_projection_ingest_sequence::Migration),",
)

# Durable inbox ordering and sequence watermarks.
path = "crates/rustok-search/src/forum_inbox.rs"
replace_once(path, "use std::cmp::Ordering;\n\n", "")
replace_once(
    path,
    "                    SELECT event_id, scope_key, revision_at, envelope_json,\n                           status, attempt_count, next_attempt_at",
    "                    SELECT event_id, scope_key, revision_at, ingest_sequence, envelope_json,\n                           status, attempt_count, next_attempt_at",
)
replace_once(
    path,
    "                    ORDER BY revision_at ASC, event_id ASC",
    "                    ORDER BY ingest_sequence ASC",
)
replace_once(
    path,
    "            let revision_at: DateTime<Utc> =\n                row.try_get(\"\", \"revision_at\").map_err(Error::Database)?;\n            let envelope_json:",
    "            let revision_at: DateTime<Utc> =\n                row.try_get(\"\", \"revision_at\").map_err(Error::Database)?;\n            let ingest_sequence: i64 = row\n                .try_get(\"\", \"ingest_sequence\")\n                .map_err(Error::Database)?;\n            if ingest_sequence <= 0 {\n                return Err(Error::External(\n                    \"Forum projection inbox returned a non-positive ingest sequence\".to_string(),\n                ));\n            }\n            let envelope_json:",
)
replace_once(
    path,
    "            if let Some((watermark_at, watermark_event_id)) =\n                load_effective_watermark(&transaction, tenant_id, &scope_key).await?\n                && !is_newer_revision(\n                    &revision_at,\n                    event_id,\n                    &watermark_at,\n                    watermark_event_id,\n                )",
    "            if let Some(watermark_sequence) =\n                load_effective_watermark(&transaction, tenant_id, &scope_key).await?\n                && ingest_sequence <= watermark_sequence",
)
replace_once(
    path,
    "                revision_at,\n                envelope,",
    "                revision_at,\n                ingest_sequence,\n                envelope,",
)
replace_once(
    path,
    "    revision_at: DateTime<Utc>,\n    envelope: EventEnvelope,",
    "    revision_at: DateTime<Utc>,\n    ingest_sequence: i64,\n    envelope: EventEnvelope,",
)
replace_once(
    path,
    "                INSERT INTO search_projection_watermarks (\n                    tenant_id, source_module, scope_key, revision_at, event_id, updated_at\n                ) VALUES ($1, 'forum', $2, $3, $4, CURRENT_TIMESTAMP)\n                ON CONFLICT (tenant_id, source_module, scope_key)\n                DO UPDATE SET\n                    revision_at = EXCLUDED.revision_at,\n                    event_id = EXCLUDED.event_id,\n                    updated_at = CURRENT_TIMESTAMP\n                WHERE search_projection_watermarks.revision_at < EXCLUDED.revision_at\n                   OR (\n                        search_projection_watermarks.revision_at = EXCLUDED.revision_at\n                        AND search_projection_watermarks.event_id < EXCLUDED.event_id\n                   )",
    "                INSERT INTO search_projection_watermarks (\n                    tenant_id, source_module, scope_key, ingest_sequence, revision_at, event_id, updated_at\n                ) VALUES ($1, 'forum', $2, $3, $4, $5, CURRENT_TIMESTAMP)\n                ON CONFLICT (tenant_id, source_module, scope_key)\n                DO UPDATE SET\n                    ingest_sequence = EXCLUDED.ingest_sequence,\n                    revision_at = EXCLUDED.revision_at,\n                    event_id = EXCLUDED.event_id,\n                    updated_at = CURRENT_TIMESTAMP\n                WHERE search_projection_watermarks.ingest_sequence < EXCLUDED.ingest_sequence",
)
replace_once(
    path,
    "                    self.scope_key.into(),\n                    self.revision_at.into(),\n                    self.event_id.into(),",
    "                    self.scope_key.into(),\n                    self.ingest_sequence.into(),\n                    self.revision_at.into(),\n                    self.event_id.into(),",
)
replace_once(
    path,
    ") -> Result<Option<(DateTime<Utc>, Uuid)>> {",
    ") -> Result<Option<i64>> {",
)
replace_once(
    path,
    ") -> Result<Option<(DateTime<Utc>, Uuid)>> {\n    let row = transaction",
    ") -> Result<Option<i64>> {\n    let row = transaction",
)
replace_once(
    path,
    "            SELECT revision_at, event_id\n            FROM search_projection_watermarks",
    "            SELECT ingest_sequence\n            FROM search_projection_watermarks",
)
replace_once(
    path,
    "    row.map(|row| {\n        Ok((\n            row.try_get(\"\", \"revision_at\").map_err(Error::Database)?,\n            row.try_get(\"\", \"event_id\").map_err(Error::Database)?,\n        ))\n    })\n    .transpose()",
    "    row.map(|row| {\n        row.try_get(\"\", \"ingest_sequence\")\n            .map_err(Error::Database)\n    })\n    .transpose()",
)
start = read(path).index("fn max_watermark(")
end = read(path).index("fn retry_delay_seconds", start)
content = read(path)
replacement = '''fn max_watermark(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

'''
write(path, content[:start] + replacement + content[end:])
replace_once(
    path,
    "    fn revision_order_uses_timestamp_then_event_identity() {\n        let timestamp = Utc::now();\n        let low = Uuid::from_u128(1);\n        let high = Uuid::from_u128(2);\n        assert!(is_newer_revision(&timestamp, high, &timestamp, low));\n        assert!(!is_newer_revision(&timestamp, low, &timestamp, high));\n        assert!(is_newer_revision(\n            &(timestamp.to_owned() + Duration::microseconds(1)),\n            low,\n            &timestamp,\n            high\n        ));\n    }\n\n    #[test]\n    fn effective_watermark_prefers_newest_revision() {\n        let timestamp = Utc::now();\n        let low = (timestamp.to_owned(), Uuid::from_u128(1));\n        let high = (\n            timestamp.to_owned() + Duration::microseconds(1),\n            Uuid::from_u128(2),\n        );\n        assert_eq!(max_watermark(Some(low), Some(high.to_owned())), Some(high));\n    }",
    "    fn ingest_sequence_order_is_numeric() {\n        assert_eq!(max_watermark(Some(3), Some(8)), Some(8));\n        assert_eq!(max_watermark(Some(8), Some(3)), Some(8));\n        assert_eq!(max_watermark(Some(8), None), Some(8));\n        assert_eq!(max_watermark(None, None), None);\n    }",
)

# Tenant sweeper follows the same oldest durable row.
path = "crates/rustok-search/src/forum_reconciliation.rs"
replace_once(
    path,
    "                        revision_at,\n                        event_id",
    "                        ingest_sequence",
)
replace_once(
    path,
    "                    ORDER BY tenant_id, revision_at ASC, event_id ASC",
    "                    ORDER BY tenant_id, ingest_sequence ASC",
)
replace_once(
    path,
    "                ORDER BY revision_at ASC, event_id ASC",
    "                ORDER BY ingest_sequence ASC",
)

# Canonical Forum plan.
path = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    path,
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters before owner eligibility, visible totals, facets and pagination. Remaining kind, channel/group and attachment-presence filters, owner revision ordering/reconciliation and maintainer runtime evidence remain.",
    "FORUM-23B2F3 locks exact requested/fallback locale and adds inclusive published date-window filters; FORUM-23B2G1 replaces Forum Search inbox wall-clock/UUID execution ordering with a durable PostgreSQL ingest sequence and sequence watermarks. Remaining kind, channel/group and attachment-presence filters, owner-issued revision ordering/reconciliation and maintainer runtime evidence remain.",
)
replace_once(
    path,
    "### Compatibility and degraded mode\n\nNo database migration, manual backfill, Search query shape, dependency, public",
    '''### Delivered in `FORUM-23B2G1`

- a Search-owned PostgreSQL migration adds positive unique immutable
  `ingest_sequence` values to Forum projection inbox rows and non-negative sequence
  watermarks;
- existing rows are backfilled deterministically by database arrival time,
  envelope revision timestamp and event identity before the database sequence is
  advanced beyond the retained maximum;
- claim order, retry blocking, due-tenant order and stale watermark comparison use
  only `ingest_sequence`; producer wall-clock timestamps and UUID ordering no longer
  choose execution order;
- `revision_at` and `event_id` remain mandatory envelope-identity and diagnostic
  fields, and author privacy/account-deletion scopes remain unskippable redaction
  barriers;
- event schemas, Forum owner writes, reindex targets, projection rebuilds, retry,
  dead-letter, public transport and storefront query behavior remain unchanged;
- `forum-search-durable-ingest-sequence.json`,
  `forum-23b2g1-search-durable-ingest-sequence.md`, and
  `verify-forum-search-durable-ingest-sequence.mjs` lock migration, ordering,
  compatibility and non-claim boundaries while runtime evidence remains pending.

### Compatibility and degraded mode

`FORUM-23B2G1` adds one PostgreSQL-only Search migration and deterministic inbox
backfill. No Forum database migration, Search query shape, dependency, public''',
)
replace_once(
    path,
    "DTO or `Cargo.lock` change is required by\n`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3`.",
    "DTO or `Cargo.lock` change is required by\n`FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3/B2G1` apart from the explicit Search inbox ordering migration above.",
)
replace_once(
    path,
    "- complete owner-issued monotonic projection revisions, durable inbox ordering,\n  reconciliation and deletion or ACL-change document cleanup;",
    "- add Forum-owner-issued monotonic projection revisions and reconcile them with\n  the delivered Search ingest sequence; complete deletion/ACL runtime evidence;",
)
content = read(path)
marker = "node scripts/verify/verify-forum-search-locale-date-filter.mjs\ncargo check -p rustok-search"
if content.count(marker) != 2:
    raise SystemExit(f"{path}: expected two Forum verification markers")
content = content.replace(
    marker,
    "node scripts/verify/verify-forum-search-locale-date-filter.mjs\nnode scripts/verify/verify-forum-search-durable-ingest-sequence.mjs\ncargo check -p rustok-search",
)
content = content.replace(
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3` source and contract records do not",
    "The `FORUM-23B2A/B2B/B2C/B2D/B2E1/B2E2/B2F1/B2F2/B2F3/B2G1` source and contract records do not",
    1,
)
write(path, content)

# Canonical Search plan.
path = "crates/rustok-search/docs/implementation-plan.md"
replace_once(
    path,
    "Runtime and reindex evidence remain\npending.\n\nSearch settings have one owner boundary.",
    '''Runtime and reindex evidence remain
pending.

`FORUM-23B2G1` adds a durable PostgreSQL-issued Forum inbox ingest sequence.
Existing rows are deterministically backfilled, new successful inserts receive a
positive unique sequence, and claim, retry blocking, due-tenant order plus scope
watermarks use that value instead of producer timestamps and event UUIDs.
Envelope `revision_at` and `event_id` remain identity/diagnostic fields, author
redaction barriers remain unskippable, and event schemas, Forum writes, rebuilds,
public APIs and storefront query behavior remain unchanged. This is not the final
Forum-owner-issued projection revision; that owner contract and rollout
reconciliation remain pending. Runtime evidence remains pending.

Search settings have one owner boundary.''',
)
replace_once(
    path,
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.",
    "- Exact Forum locale/date contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-locale-date-filter.json` and\n  `scripts/verify/verify-forum-search-locale-date-filter.mjs`.\n- Durable Forum inbox ingest-sequence status:\n  `source_complete_execution_pending` under `FORUM-23B2G1`.\n- Durable ingest-sequence contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-durable-ingest-sequence.json` and\n  `scripts/verify/verify-forum-search-durable-ingest-sequence.mjs`.",
)
replace_once(
    path,
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Durable non-Forum projection replay/recovery remains `blocked`.",
    "- Exact Forum locale and date filtering is `source_complete_execution_pending` under\n  `FORUM-23B2F3`.\n- Durable Forum inbox ingest ordering is `source_complete_execution_pending` under\n  `FORUM-23B2G1`; Forum-owner-issued revisions remain pending.\n- Durable non-Forum projection replay/recovery remains `blocked`.",
)
replace_once(
    path,
    "    fail-closed legacy projection behavior under `FORUM-23B2F3`.\n\n## Next results",
    "    fail-closed legacy projection behavior under `FORUM-23B2F3`.\n23. Added a PostgreSQL-issued immutable Forum inbox ingest sequence, deterministic\n    existing-row backfill, sequence-based claims/due-tenant ordering and completed\n    sequence watermarks under `FORUM-23B2G1`.\n\n## Next results",
)
replace_once(
    path,
    "2. **Generalize durable Search projection recovery.** Use the existing generic",
    "2. **Add owner-issued Forum projection revisions.** Carry a monotonic Forum-owned\n   revision in versioned invalidation events and reconcile it with the delivered\n   Search ingest sequence during rolling deployment. **Done when:** source revision\n   watermarks reject stale owner state independently of delivery order.\n3. **Generalize durable Search projection recovery.** Use the existing generic",
)
content = read(path)
content = content.replace("3. **Execute Forum Search eligibility evidence.**", "4. **Execute Forum Search eligibility evidence.**", 1)
content = content.replace("4. **Execute canonical URL evidence.**", "5. **Execute canonical URL evidence.**", 1)
content = content.replace("5. **Verify click analytics.**", "6. **Verify click analytics.**", 1)
content = content.replace("6. **Execute live Blog projection evidence.**", "7. **Execute live Blog projection evidence.**", 1)
content = content.replace("7. **Execute live provider evidence.**", "8. **Execute live provider evidence.**", 1)
content = content.replace("8. **Add external engines only as adapters.**", "9. **Add external engines only as adapters.**", 1)
write(path, content)
