const INBOX: &str = include_str!("../src/forum_inbox.rs");
const INGESTION: &str = include_str!("../src/ingestion.rs");
const MIGRATION: &str =
    include_str!("../src/migrations/m20260730_000009_create_search_projection_inbox.rs");
const MIGRATION_REGISTRY: &str = include_str!("../src/migrations/mod.rs");
const MANIFEST: &str = include_str!("../Cargo.toml");
const LIB: &str = include_str!("../src/lib.rs");

fn require(source: &str, marker: &str) {
    assert!(source.contains(marker), "missing source marker: {marker}");
}

fn reject(source: &str, marker: &str) {
    assert!(
        !source.contains(marker),
        "forbidden source marker: {marker}"
    );
}

#[test]
fn durable_inbox_stores_replayable_envelopes_and_terminal_state() {
    for marker in [
        "CREATE TABLE IF NOT EXISTS search_projection_inbox",
        "event_id UUID PRIMARY KEY",
        "envelope_json JSONB NOT NULL",
        "'retryable_error'",
        "'dead_letter'",
        "CREATE TABLE IF NOT EXISTS search_projection_watermarks",
        "PRIMARY KEY (tenant_id, source_module, scope_key)",
        "idx_search_projection_inbox_due",
    ] {
        require(MIGRATION, marker);
    }
    reject(MIGRATION, "search_projection_rollup_forum_watermark");
    require(
        MIGRATION_REGISTRY,
        "m20260730_000009_create_search_projection_inbox",
    );
}

#[test]
fn forum_events_are_strictly_ordered_by_envelope_revision() {
    for marker in [
        "status IN ('pending', 'retryable_error')",
        "ORDER BY revision_at ASC, event_id ASC",
        "FOR UPDATE",
        "due_at > Utc::now()",
        "return Ok(None)",
        "pg_try_advisory_xact_lock(hashtextextended($1, 0))",
        "AS acquired",
        "search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}",
        "load_effective_watermark",
        "load_watermark(transaction, tenant_id, FULL_SCOPE_KEY)",
        "incoming_event_id.as_bytes() > watermark_event_id.as_bytes()",
        "Some(\"stale_revision\")",
        "ON CONFLICT (tenant_id, source_module, scope_key)",
        "MAX_ATTEMPTS: u32 = 12",
        "retry_exhausted",
        "SqlValue::Json(Some(Box::new(envelope_json)))",
    ] {
        require(INBOX, marker);
    }
    reject(INBOX, "pg_advisory_xact_lock(");
    reject(INBOX, "OnceLock");
    reject(INBOX, "OwnedMutexGuard");
    reject(INBOX, "SystemTime::now");
}

#[test]
fn handler_has_no_direct_forum_projection_bypass() {
    for marker in [
        "ForumProjectionScope::for_event(&envelope.event)",
        "inbox.enqueue(envelope, &scope).await?",
        "reconcile_forum_inbox",
        "apply_forum_inbox_event",
        "claim.complete().await?",
        "claim.retry(&error).await?",
        "FORUM_INBOX_OPPORTUNISTIC_BATCH",
        "Forum projection inbox opportunistic reconciliation failed",
    ] {
        require(INGESTION, marker);
    }
    require(
        INGESTION,
        "projector.rebuild_tenant(envelope.tenant_id).await",
    );
    require(INGESTION, "projector.delete_tenant(tenant_id).await");
    reject(
        INGESTION,
        ".refresh_entity(envelope.tenant_id, \"forum_topic\"",
    );
}

#[test]
fn module_registers_inbox_without_new_runtime_dependency() {
    require(LIB, "mod forum_inbox;");
    require(LIB, "SearchIngestionHandler::with_forum_source");
    require(MANIFEST, "rustok-content.workspace = true");
    require(MANIFEST, "[dev-dependencies]");
    reject(LIB, "rustok_forum::");
}
