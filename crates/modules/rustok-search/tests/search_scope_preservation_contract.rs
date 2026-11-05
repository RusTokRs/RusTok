const SEARCH_LIB: &str = include_str!("../src/lib.rs");
const ACTIVE_PROJECTOR: &str = include_str!("../src/projector.rs");
const CORE_PROJECTOR: &str = include_str!("../src/projector_core.rs");
const INGESTION: &str = include_str!("../src/ingestion.rs");
const PROJECTION_DRIVER: &str = include_str!("../src/projection_driver.rs");

#[test]
fn active_tenant_rebuild_preserves_external_scopes_and_eliminates_destructive_deletes() {
    assert!(SEARCH_LIB.contains("mod projector_core;"));
    assert!(!SEARCH_LIB.contains("pub mod projector_core;"));
    assert!(ACTIVE_PROJECTOR.contains("self.core.rebuild_tenant(tenant_id).await"));
    assert!(CORE_PROJECTOR.contains("self.rebuild_content_scope(tenant_id).await?"));
    assert!(CORE_PROJECTOR.contains("self.rebuild_product_scope(tenant_id).await"));
    assert!(!ACTIVE_PROJECTOR.contains("DELETE FROM search_documents WHERE tenant_id = $1\""));
    assert!(!CORE_PROJECTOR.contains("DELETE FROM search_documents WHERE tenant_id = $1\""));
}

#[test]
fn full_ingestion_rebuild_keeps_source_order_and_atomic_external_replacements() {
    let rebuild = INGESTION
        .split("async fn rebuild_tenant")
        .nth(1)
        .expect("Search ingestion tenant rebuild should remain explicit")
        .split("async fn handle_reindex_request")
        .next()
        .expect("Search ingestion tenant rebuild should have a bounded body");
    let core = rebuild
        .find("self.projector.rebuild_tenant")
        .expect("core Search scopes should rebuild first");
    let owners = rebuild
        .find("self.owner_drivers()")
        .expect("owner sources should rebuild after the core scopes");
    assert!(core < owners);
    // The Blog scope is an owner source now, so the rebuild loop above is the only
    // place that reaches it; a module-named writer in the ingestion path would put
    // the old private projector back.
    assert!(!INGESTION.contains("blog_projector"));
    assert!(!INGESTION.contains("BlogSearchProjector"));

    for marker in [
        "let tx = self.db.begin().await.map_err(Error::Database)?",
        "self.create_stage(&tx).await?",
        "self.delete_scope(&tx, tenant_id).await?",
        "tx.commit().await.map_err(Error::Database)",
    ] {
        assert!(
            PROJECTION_DRIVER.contains(marker),
            "missing projection-driver atomic marker {marker}"
        );
    }
    // A scope delete must be bounded by the source module, never by a module name.
    assert!(
        PROJECTION_DRIVER
            .contains("DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2")
    );
}

#[test]
fn bootstrap_presence_check_ignores_external_only_documents() {
    assert!(ACTIVE_PROJECTOR.contains("entity_type IN ('node', 'product')"));
    for external_entity in ["blog_post", "forum_category", "forum_topic"] {
        assert!(
            !ACTIVE_PROJECTOR
                .split("const CORE_SCOPE_COUNT_SQL")
                .nth(1)
                .expect("core count SQL should exist")
                .split("\"#;")
                .next()
                .expect("core count SQL should terminate")
                .contains(external_entity),
            "bootstrap count must ignore {external_entity}"
        );
    }
}
