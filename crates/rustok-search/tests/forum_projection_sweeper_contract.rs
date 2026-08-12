const RECONCILER: &str = include_str!("../src/forum_reconciliation.rs");
const INBOX: &str = include_str!("../src/forum_inbox.rs");
const INGESTION: &str = include_str!("../src/ingestion.rs");
const LIB: &str = include_str!("../src/lib.rs");
const SERVER_WORKER: &str =
    include_str!("../../../apps/server/src/services/forum_search_inbox_worker.rs");
const SERVER_SERVICES: &str = include_str!("../../../apps/server/src/services/mod.rs");
const SERVER_BOOTSTRAP: &str =
    include_str!("../../../apps/server/src/services/server_bootstrap.rs");

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
fn due_tenant_discovery_preserves_oldest_event_retry_barrier() {
    for marker in [
        "SELECT DISTINCT ON (tenant_id)",
        "status IN ('pending', 'retryable_error')",
        "ORDER BY tenant_id, ingest_sequence ASC",
        "status = 'pending'",
        "next_attempt_at <= CURRENT_TIMESTAMP",
        "ORDER BY ingest_sequence ASC",
        "DEFAULT_FORUM_SWEEP_TENANT_LIMIT: usize = 32",
        "DEFAULT_FORUM_SWEEP_EVENT_LIMIT: usize = 64",
        "MAX_FORUM_SWEEP_TENANT_LIMIT: usize = 256",
        "MAX_FORUM_SWEEP_EVENT_LIMIT: usize = 256",
    ] {
        require(RECONCILER, marker);
    }
    reject(
        RECONCILER,
        "SELECT DISTINCT tenant_id FROM search_projection_inbox",
    );
}

#[test]
fn sweeper_reuses_search_owned_claim_projection_and_retry_owners() {
    for marker in [
        "self.inbox.claim_next(tenant_id).await?",
        "claim.complete().await?",
        "claim.retry(&error).await?",
        "self.forum_projector.rebuild_tenant",
        "self.forum_projector.refresh_entity",
        "self.forum_projector.delete_tenant",
        "self.projector.rebuild_tenant",
        "self.blog_projector.rebuild_tenant",
        "ForumProjectionReconciler",
    ] {
        require(RECONCILER, marker);
    }
    require(INBOX, "pg_try_advisory_xact_lock");
    require(LIB, "pub use forum_reconciliation");
    reject(RECONCILER, "UPDATE search_projection_inbox");
    reject(RECONCILER, "INSERT INTO search_projection_watermarks");
}

#[test]
fn sweeper_replay_scope_matches_event_ingestion_scope() {
    for marker in [
        "DomainEvent::ForumTopicCreated",
        "DomainEvent::ForumTopicReplied",
        "DomainEvent::ForumTopicStatusChanged",
        "DomainEvent::ForumTopicPinned",
        "DomainEvent::ForumReplyStatusChanged",
        "DomainEvent::TenantModuleToggled",
        "DomainEvent::LocaleEnabled",
        "DomainEvent::LocaleDisabled",
        "DomainEvent::TenantCreated",
        "DomainEvent::TenantUpdated",
        "DomainEvent::ReindexRequested",
        "(\"search\", _)",
        "(\"forum\", _) | (\"forum_topic\", Some(_))",
        "(\"forum_category\", Some(category_id))",
    ] {
        require(RECONCILER, marker);
        require(INGESTION, marker);
    }
}

#[test]
fn host_worker_runs_startup_periodic_and_shutdown_aware_sweeps() {
    for marker in [
        "runs_background_workers()",
        "ForumSearchInboxWorkerHandle",
        "search_projection_source_registry_from_extensions",
        "supports_background_reconciliation()",
        "tokio::spawn(forum_search_inbox_worker_loop",
        "sweep_due(",
        "Duration::from_secs(5)",
        "StopHandle",
        "stop_rx.changed()",
        "Forum Search inbox sweep failed",
    ] {
        require(SERVER_WORKER, marker);
    }
    require(SERVER_SERVICES, "pub mod forum_search_inbox_worker;");
    require(SERVER_BOOTSTRAP, "start_forum_search_inbox_worker_if_ready");
    reject(SERVER_WORKER, "search_projection_inbox");
    reject(SERVER_WORKER, "search_projection_watermarks");
    reject(SERVER_WORKER, "ReindexRequested");
}
