use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use rustok_core::events::EventHandler;
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumModule,
    ForumSearchProjectionSourceFactory, ReplyService, TopicService,
};
use rustok_outbox::{OutboxModule, TransactionalEventBus};
use rustok_search::{
    SearchIngestionHandler, SearchModule, SearchProjectionSourceFactory, SearchResultItem,
    canonical_search_result_url,
};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";

struct TestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema: String,
}

impl TestDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(url) = postgres_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to PostgreSQL; skipping Forum canonical-route reindex evidence"
            );
            return Ok(None);
        };
        let control = connect(&url).await?;
        let schema = format!("rustok_forum_route_reindex_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema}""#))
            .await?;
        let db = connect(&url).await?;
        set_search_path(&db, &schema).await?;

        let setup = async {
            db.execute_unprepared(
                "CREATE TABLE users (id UUID NOT NULL PRIMARY KEY, tenant_id UUID NOT NULL)",
            )
            .await?;
            let manager = SchemaManager::new(&db);
            for migration in OutboxModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in TaxonomyModule.migrations() {
                migration.up(&manager).await?;
            }
            flex::cache_generation::create_field_definition_cache_generation_table(&manager)
                .await?;
            for migration in ForumModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;
        if let Err(error) = setup {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
                .await;
            return Err(error.into());
        }
        Ok(Some(Self {
            control,
            db,
            schema,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema
            ))
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
struct Document {
    id: Uuid,
    kind: String,
    locale: String,
    title: String,
    payload: JsonValue,
}

#[tokio::test]
async fn forum_reindex_atomically_replaces_legacy_routes_with_owner_canonical_routes()
-> TestResult<()> {
    let Some(test) = TestDb::setup().await? else {
        return Ok(());
    };
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    execute(
        &test.db,
        "INSERT INTO users (id, tenant_id) VALUES ($1, $2)",
        vec![admin_id.into(), tenant_id.into()],
    )
    .await?;

    let bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    let admin = SecurityContext::new(UserRole::Admin, Some(admin_id));
    let category_id = CategoryService::new(test.db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "Platform".into(),
                slug: "platform".into(),
                description: Some("Canonical route evidence".into()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id;
    let topic_id = TopicService::new(test.db.clone(), bus.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: "Canonical search".into(),
                slug: Some("canonical-search".into()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Forum Search canonical route evidence",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id;
    let reply_id = ReplyService::new(test.db.clone(), bus)
        .create(
            tenant_id,
            admin,
            topic_id,
            CreateReplyInput {
                locale: "en".into(),
                content: rustok_api::RichTextDocument::single_paragraph(
                    "Approved reply route evidence",
                ),
                parent_reply_id: None,
            },
        )
        .await?
        .id;

    execute(
        &test.db,
        "UPDATE forum_topics SET author_id = NULL, status = 'open' WHERE tenant_id = $1 AND id = $2",
        vec![tenant_id.into(), topic_id.into()],
    )
    .await?;
    execute(
        &test.db,
        "UPDATE forum_replies SET author_id = NULL, status = 'approved' WHERE tenant_id = $1 AND id = $2",
        vec![tenant_id.into(), reply_id.into()],
    )
    .await?;

    let stale_orphan_id = Uuid::new_v4();
    for (id, kind, route) in [
        (
            category_id,
            "forum_category",
            format!("/modules/forum?category={category_id}"),
        ),
        (
            topic_id,
            "forum_topic",
            format!("/modules/forum?topic={topic_id}"),
        ),
        (
            reply_id,
            "forum_reply",
            format!("/modules/forum?topic={topic_id}&reply={reply_id}"),
        ),
        (
            stale_orphan_id,
            "forum_topic",
            format!("/modules/forum?topic={stale_orphan_id}"),
        ),
    ] {
        insert_legacy(&test.db, tenant_id, id, kind, &route).await?;
    }
    let other_id = Uuid::new_v4();
    insert_legacy(
        &test.db,
        other_tenant_id,
        other_id,
        "forum_topic",
        &format!("/modules/forum?topic={other_id}"),
    )
    .await?;

    let source = ForumSearchProjectionSourceFactory.build(test.db.clone());
    let handler = SearchIngestionHandler::with_forum_source(test.db.clone(), Some(source));
    let reindex = envelope(
        tenant_id,
        admin_id,
        DomainEvent::ReindexRequested {
            target_type: "forum".into(),
            target_id: None,
        },
    )?;
    handler.handle(&reindex).await?;
    assert_inbox_completed(&test.db, tenant_id, reindex.id).await?;

    let documents = load_documents(&test.db, tenant_id)
        .await?
        .into_iter()
        .map(|document| (document.kind.clone(), document))
        .collect::<HashMap<_, _>>();
    assert_eq!(documents.len(), 3);

    let topic_identity = topic_id.simple().to_string();
    let topic_short_id = &topic_identity[..12];
    let category_route = "/en/forum/c/platform".to_string();
    let topic_route = format!("/en/forum/t/{topic_short_id}/canonical-search");
    let reply_route = format!("{topic_route}?reply={reply_id}");
    assert_route(
        documents.get("forum_category").expect("category document"),
        category_id,
        &category_route,
    );
    assert_route(
        documents.get("forum_topic").expect("topic document"),
        topic_id,
        &topic_route,
    );
    assert_route(
        documents.get("forum_reply").expect("reply document"),
        reply_id,
        &reply_route,
    );

    assert_eq!(count_legacy_routes(&test.db, tenant_id).await?, 0);
    assert!(
        load_document(&test.db, tenant_id, stale_orphan_id)
            .await?
            .is_none()
    );
    let other = load_document(&test.db, other_tenant_id, other_id)
        .await?
        .expect("other tenant document");
    assert_eq!(
        other.payload["route"],
        format!("/modules/forum?topic={other_id}")
    );

    test.cleanup().await
}

fn assert_route(document: &Document, id: Uuid, route: &str) {
    assert_eq!(document.id, id);
    assert_eq!(document.locale, "en");
    assert_eq!(document.payload["route"], route);
    let result = SearchResultItem {
        id,
        entity_type: document.kind.clone(),
        source_module: "forum".into(),
        title: document.title.clone(),
        snippet: None,
        score: 1.0,
        locale: Some(document.locale.clone()),
        payload: document.payload.clone(),
    };
    assert_eq!(canonical_search_result_url(&result).as_deref(), Some(route));
}

fn envelope(tenant_id: Uuid, actor_id: Uuid, event: DomainEvent) -> TestResult<EventEnvelope> {
    Ok(ContractEventEnvelope::new(tenant_id, Some(actor_id), event)?.into_root_envelope()?)
}

async fn execute(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .await?;
    Ok(())
}

async fn insert_legacy(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
    kind: &str,
    route: &str,
) -> Result<(), sea_orm::DbErr> {
    let payload = serde_json::json!({
        "category_id": id,
        "topic_id": id,
        "reply_id": id,
        "route": route
    });
    execute(
        db,
        r#"
        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, body, keywords_text, facets, payload,
            created_at, updated_at, indexed_at
        ) VALUES ($1, $2, $3, 'forum', $4, 'en', 'public', TRUE,
            'Legacy Forum document', '', '', '{}'::jsonb, $5, NOW(), NOW(), NOW())
        "#,
        vec![
            format!("legacy:{kind}:{id}:en").into(),
            tenant_id.into(),
            id.into(),
            kind.to_string().into(),
            payload.into(),
        ],
    )
    .await
}

async fn load_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<Vec<Document>, sea_orm::DbErr> {
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT document_id, entity_type, locale, title, payload FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum' AND entity_type IN ('forum_category', 'forum_topic', 'forum_reply') ORDER BY entity_type",
            vec![tenant_id.into()],
        ))
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(Document {
                id: row.try_get("", "document_id")?,
                kind: row.try_get("", "entity_type")?,
                locale: row.try_get("", "locale")?,
                title: row.try_get("", "title")?,
                payload: row.try_get("", "payload")?,
            })
        })
        .collect()
}

async fn load_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Document>, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT document_id, entity_type, locale, title, payload FROM search_documents WHERE tenant_id = $1 AND document_id = $2 AND source_module = 'forum'",
            vec![tenant_id.into(), id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(Document {
            id: row.try_get("", "document_id")?,
            kind: row.try_get("", "entity_type")?,
            locale: row.try_get("", "locale")?,
            title: row.try_get("", "title")?,
            payload: row.try_get("", "payload")?,
        })
    })
    .transpose()
}

async fn count_legacy_routes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM search_documents WHERE tenant_id = $1 AND source_module = 'forum' AND payload ->> 'route' LIKE '/modules/forum%'",
            vec![tenant_id.into()],
        ))
        .await?
        .expect("legacy route count");
    row.try_get("", "value")
}

async fn assert_inbox_completed(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT scope_key, status FROM search_projection_inbox WHERE tenant_id = $1 AND event_id = $2",
            vec![tenant_id.into(), event_id.into()],
        ))
        .await?
        .expect("Forum inbox row");
    let scope: String = row.try_get("", "scope_key")?;
    let status: String = row.try_get("", "status")?;
    assert_eq!(scope, "forum");
    assert_eq!(status, "completed");
    Ok(())
}

fn postgres_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema}", public"#))
        .await?;
    Ok(())
}
