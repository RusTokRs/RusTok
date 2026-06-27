use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use serde_json::Value;
use uuid::Uuid;

const REPRESENTATIVE_ROWS: i32 = 100_000;

#[tokio::test]
#[ignore = "requires a live PostgreSQL DATABASE_URL with migrated search_documents"]
async fn representative_search_queries_use_expected_indexes() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let db = Database::connect(database_url)
        .await
        .expect("connect to PostgreSQL");
    assert_eq!(db.get_database_backend(), DbBackend::Postgres);

    let table = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('public.search_documents')::text AS table_name".to_string(),
        ))
        .await
        .expect("inspect search_documents")
        .and_then(|row| row.try_get::<Option<String>>("", "table_name").ok())
        .flatten();
    assert!(
        table.is_some(),
        "search_documents is missing; apply server migrations first"
    );

    let manager = SchemaManager::new(&db);
    let typo_index_migration = rustok_search::migrations::migrations()
        .into_iter()
        .find(|migration| {
            migration
                .name()
                .contains("000006_add_search_typo_tolerance_indexes")
        })
        .expect("search typo-tolerance migration");
    typo_index_migration
        .up(&manager)
        .await
        .expect("apply idempotent search typo-tolerance migration");

    let tenant_id = Uuid::new_v4();
    let seed_sql = format!(
        r#"
        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, subtitle, slug, handle, body,
            keywords_text, facets, payload, created_at, updated_at, indexed_at
        )
        SELECT
            '{tenant_id}:' || item::text,
            '{tenant_id}'::uuid,
            md5('{tenant_id}:' || item::text)::uuid,
            'product',
            'product',
            'en-US',
            'published',
            true,
            CASE WHEN item % 10000 = 0
                THEN 'rare quantum telescope ' || item::text
                ELSE 'catalog item ' || item::text
            END,
            NULL,
            'catalog-item-' || item::text,
            'catalog-item-' || item::text,
            CASE WHEN item % 10000 = 0
                THEN 'rare quantum telescope observatory'
                ELSE 'ordinary catalog description'
            END,
            CASE WHEN item % 10000 = 0 THEN 'astronomy quantum' ELSE 'catalog' END,
            '{{}}'::jsonb,
            '{{}}'::jsonb,
            now(),
            now(),
            now()
        FROM generate_series(1, {REPRESENTATIVE_ROWS}) AS item
        "#
    );
    db.execute(Statement::from_string(DbBackend::Postgres, seed_sql))
        .await
        .expect("seed representative search documents");
    db.execute(Statement::from_string(
        DbBackend::Postgres,
        "ANALYZE search_documents".to_string(),
    ))
    .await
    .expect("analyze search_documents");

    let fts_sql = format!(
        r#"
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        SELECT document_id, title
        FROM search_documents
        WHERE tenant_id = '{tenant_id}'::uuid
          AND locale = 'en-US'
          AND search_vector @@ websearch_to_tsquery('simple', 'rare quantum telescope')
        ORDER BY updated_at DESC
        LIMIT 25
        "#
    );
    let typo_sql = format!(
        r#"
        EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
        WITH candidate_keys AS (
            SELECT document_key FROM search_documents
            WHERE tenant_id = '{tenant_id}'::uuid AND locale = 'en-US'
              AND lower(title) % 'quantm telescop'
            UNION
            SELECT document_key FROM search_documents
            WHERE tenant_id = '{tenant_id}'::uuid AND locale = 'en-US'
              AND lower(COALESCE(slug, '')) % 'quantm telescop'
            UNION
            SELECT document_key FROM search_documents
            WHERE tenant_id = '{tenant_id}'::uuid AND locale = 'en-US'
              AND lower(COALESCE(handle, '')) % 'quantm telescop'
            UNION
            SELECT document_key FROM search_documents
            WHERE tenant_id = '{tenant_id}'::uuid AND locale = 'en-US'
              AND lower(COALESCE(keywords_text, '')) % 'quantm telescop'
        )
        SELECT sd.document_id, sd.title
        FROM search_documents sd
        INNER JOIN candidate_keys candidates USING (document_key)
        WHERE sd.tenant_id = '{tenant_id}'::uuid AND sd.locale = 'en-US'
        ORDER BY updated_at DESC
        LIMIT 25
        "#
    );

    let fts_plan = explain_json(&db, fts_sql).await;
    let typo_plan = explain_json(&db, typo_sql).await;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        format!("DELETE FROM search_documents WHERE tenant_id = '{tenant_id}'::uuid"),
    ))
    .await
    .expect("clean representative search documents");

    let fts_text = fts_plan.to_string();
    let typo_text = typo_plan.to_string();
    let fts_ms = execution_time_ms(&fts_plan);
    let typo_ms = execution_time_ms(&typo_plan);

    println!(
        "search_query_plan_baseline rows={REPRESENTATIVE_ROWS} fts_ms={fts_ms:.3} typo_ms={typo_ms:.3}"
    );
    assert!(
        fts_text.contains("idx_search_documents_fts"),
        "FTS plan did not use idx_search_documents_fts: {fts_text}"
    );
    assert!(
        typo_text.contains("idx_search_documents_title_trgm")
            || typo_text.contains("idx_search_documents_slug_trgm")
            || typo_text.contains("idx_search_documents_handle_trgm")
            || typo_text.contains("idx_search_documents_keywords_trgm"),
        "typo plan did not use a search trigram index: {typo_text}"
    );
    assert!(fts_ms < 500.0, "FTS baseline exceeded 500 ms: {fts_ms}");
    assert!(typo_ms < 500.0, "typo baseline exceeded 500 ms: {typo_ms}");
}

async fn explain_json(db: &sea_orm::DatabaseConnection, sql: String) -> Value {
    db.query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("execute EXPLAIN")
        .expect("EXPLAIN row")
        .try_get("", "QUERY PLAN")
        .expect("decode EXPLAIN JSON")
}

fn execution_time_ms(plan: &Value) -> f64 {
    plan.as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("Execution Time"))
        .and_then(Value::as_f64)
        .expect("EXPLAIN must contain Execution Time")
}
