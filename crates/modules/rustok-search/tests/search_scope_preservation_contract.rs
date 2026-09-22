const SEARCH_LIB: &str = include_str!("../src/lib.rs");
const ACTIVE_PROJECTOR: &str = include_str!("../src/projector.rs");
const LEGACY_PROJECTOR: &str = include_str!("../src/projector_legacy.rs");
const INGESTION: &str = include_str!("../src/ingestion.rs");
const BLOG_PROJECTOR: &str = include_str!("../src/blog_projector.rs");
const FORUM_PROJECTOR: &str = include_str!("../src/forum_projector.rs");

#[test]
fn active_tenant_rebuild_never_calls_the_destructive_legacy_tenant_rebuild() {
    assert!(SEARCH_LIB.contains("mod projector_legacy;"));
    assert!(!SEARCH_LIB.contains("pub mod projector_legacy;"));
    assert!(ACTIVE_PROJECTOR.contains("self.legacy.rebuild_content_scope(tenant_id).await?"));
    assert!(ACTIVE_PROJECTOR.contains("self.legacy.rebuild_product_scope(tenant_id).await"));
    assert!(!ACTIVE_PROJECTOR.contains("self.legacy.rebuild_tenant"));
    assert!(!ACTIVE_PROJECTOR.contains("DELETE FROM search_documents WHERE tenant_id = $1\""));
    assert!(LEGACY_PROJECTOR.contains("DELETE FROM search_documents WHERE tenant_id = $1\""));
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
    let blog = rebuild
        .find("self.blog_projector.rebuild_tenant")
        .expect("Blog scope should rebuild after core scopes");
    let forum = rebuild
        .rfind("projector.rebuild_tenant")
        .expect("Forum scope should rebuild after Blog");
    assert!(core < blog && blog < forum);

    for marker in [
        "let tx = self.begin_transaction().await?",
        "self.delete_tenant_documents_in(&tx, tenant_id).await?",
        "self.commit_transaction(tx).await",
    ] {
        assert!(
            BLOG_PROJECTOR.contains(marker),
            "missing Blog atomic marker {marker}"
        );
    }
    for marker in [
        "let tx = self.db.begin().await.map_err(Error::Database)?",
        "self.create_stage(&tx).await?",
        "delete_forum_scope(&tx, tenant_id).await?",
        "tx.commit().await.map_err(Error::Database)",
    ] {
        assert!(
            FORUM_PROJECTOR.contains(marker),
            "missing Forum atomic marker {marker}"
        );
    }
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
