use std::error::Error;

use rustok_core::{MigrationSource, events::EventHandler};
use rustok_events::{ContractEventEnvelope, DomainEvent, EventEnvelope};
use rustok_search::{SearchIngestionHandler, SearchModule};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog search projection lifecycle test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            create_blog_projection_source_tables(&db).await
        }
        .await;

        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
struct SearchDocumentSnapshot {
    status: String,
    is_public: bool,
    title: String,
    slug: Option<String>,
    locale: String,
    payload: JsonValue,
}

#[tokio::test]
async fn blog_events_upsert_publish_archive_and_delete_search_document() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_lifecycle").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "draft",
        "release-notes",
        "Release notes",
    )
    .await?;

    let handler = SearchIngestionHandler::new(test_db.db.clone());
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostCreated {
                post_id,
                author_id: Some(author_id),
                locale: "en".to_string(),
            },
        )?)
        .await?;

    let draft = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("created Blog post should be projected");
    assert_eq!(draft.status, "draft");
    assert!(!draft.is_public);
    assert_eq!(draft.title, "Release notes");
    assert_eq!(draft.slug.as_deref(), Some("release-notes"));
    assert_eq!(draft.locale, "en");
    assert_eq!(draft.payload["slug"], "release-notes");
    assert_eq!(draft.payload["author_name"], "Search Author");
    assert_eq!(draft.payload["tags"], serde_json::json!(["cms", "rust"]));
    assert_eq!(draft.payload["channel_slugs"], serde_json::json!(["web"]));

    test_db
        .db
        .execute_unprepared(&format!(
            r#"
            UPDATE blog_posts
            SET status = 'published', published_at = NOW(), updated_at = NOW(), version = 2
            WHERE id = '{post_id}' AND tenant_id = '{tenant_id}';
            UPDATE blog_post_translations
            SET title = 'Release notes v2', body = 'Published body', updated_at = NOW()
            WHERE post_id = '{post_id}' AND locale = 'en';
            "#
        ))
        .await?;
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostPublished {
                post_id,
                author_id: Some(author_id),
            },
        )?)
        .await?;

    let published = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("published Blog post should remain projected");
    assert_eq!(published.status, "published");
    assert!(published.is_public);
    assert_eq!(published.title, "Release notes v2");
    assert_eq!(published.payload["version"], 2);
    assert!(!published.payload["published_at"].is_null());

    test_db
        .db
        .execute_unprepared(&format!(
            r#"
            UPDATE blog_posts
            SET status = 'archived', archived_at = NOW(), updated_at = NOW(), version = 3
            WHERE id = '{post_id}' AND tenant_id = '{tenant_id}'
            "#
        ))
        .await?;
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostArchived {
                post_id,
                reason: Some("superseded".to_string()),
            },
        )?)
        .await?;

    let archived = load_blog_document(&test_db.db, tenant_id, post_id)
        .await?
        .expect("archived Blog post should remain projected for non-public search surfaces");
    assert_eq!(archived.status, "archived");
    assert!(!archived.is_public);
    assert_eq!(archived.payload["version"], 3);
    assert!(!archived.payload["archived_at"].is_null());

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::BlogPostDeleted { post_id },
        )?)
        .await?;
    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_none()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn full_blog_reindex_replaces_only_current_tenant_blog_documents() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_reindex").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let first_post_id = Uuid::new_v4();
    let second_post_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        first_post_id,
        author_id,
        "published",
        "first-post",
        "First post",
    )
    .await?;
    insert_blog_post(
        &test_db.db,
        tenant_id,
        second_post_id,
        author_id,
        "draft",
        "second-post",
        "Second post",
    )
    .await?;

    let stale_id = Uuid::new_v4();
    let other_tenant_document_id = Uuid::new_v4();
    insert_search_document(
        &test_db.db,
        tenant_id,
        stale_id,
        "blog",
        "blog_post",
        "stale",
    )
    .await?;
    insert_search_document(
        &test_db.db,
        other_tenant_id,
        other_tenant_document_id,
        "blog",
        "blog_post",
        "other-tenant",
    )
    .await?;

    let handler = SearchIngestionHandler::new(test_db.db.clone());
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;

    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 2);
    assert!(
        load_blog_document(&test_db.db, tenant_id, stale_id)
            .await?
            .is_none()
    );
    assert!(
        load_blog_document(&test_db.db, tenant_id, first_post_id)
            .await?
            .is_some()
    );
    assert!(
        load_blog_document(&test_db.db, tenant_id, second_post_id)
            .await?
            .is_some()
    );
    assert!(
        load_blog_document(&test_db.db, other_tenant_id, other_tenant_document_id)
            .await?
            .is_some()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn blog_module_disable_cleans_scope_and_enable_rebuilds_it() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_module_toggle").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let other_tenant_document_id = Uuid::new_v4();
    insert_blog_post(
        &test_db.db,
        tenant_id,
        post_id,
        author_id,
        "published",
        "module-toggle",
        "Module toggle",
    )
    .await?;
    insert_search_document(
        &test_db.db,
        other_tenant_id,
        other_tenant_document_id,
        "blog",
        "blog_post",
        "other-tenant",
    )
    .await?;

    let handler = SearchIngestionHandler::new(test_db.db.clone());
    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: None,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 1);

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: false,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 0);
    assert!(
        load_blog_document(&test_db.db, other_tenant_id, other_tenant_document_id)
            .await?
            .is_some()
    );

    handler
        .handle(&envelope(
            tenant_id,
            Some(author_id),
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug: "blog".to_string(),
                enabled: true,
            },
        )?)
        .await?;
    assert_eq!(count_blog_documents(&test_db.db, tenant_id).await?, 1);
    assert!(
        load_blog_document(&test_db.db, tenant_id, post_id)
            .await?
            .is_some()
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn targeted_reindex_removes_stale_document_when_source_post_is_missing() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("blog_missing_target").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let missing_post_id = Uuid::new_v4();
    insert_search_document(
        &test_db.db,
        tenant_id,
        missing_post_id,
        "blog",
        "blog_post",
        "missing-source",
    )
    .await?;

    let handler = SearchIngestionHandler::new(test_db.db.clone());
    handler
        .handle(&envelope(
            tenant_id,
            None,
            DomainEvent::ReindexRequested {
                target_type: "blog".to_string(),
                target_id: Some(missing_post_id),
            },
        )?)
        .await?;

    assert!(
        load_blog_document(&test_db.db, tenant_id, missing_post_id)
            .await?
            .is_none()
    );

    test_db.cleanup().await
}

fn envelope(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    event: DomainEvent,
) -> TestResult<EventEnvelope> {
    Ok(ContractEventEnvelope::new(tenant_id, actor_id, event)?.into_root_envelope()?)
}

async fn create_blog_projection_source_tables(
    db: &DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id UUID PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE blog_posts (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            author_id UUID NOT NULL,
            category_id UUID NULL,
            status TEXT NOT NULL,
            slug TEXT NOT NULL,
            metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
            featured_image_url TEXT NULL,
            published_at TIMESTAMPTZ NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            archived_at TIMESTAMPTZ NULL,
            comment_count BIGINT NOT NULL DEFAULT 0,
            view_count BIGINT NOT NULL DEFAULT 0,
            version BIGINT NOT NULL DEFAULT 1
        );

        CREATE TABLE blog_post_translations (
            id UUID PRIMARY KEY,
            post_id UUID NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            excerpt TEXT NULL,
            seo_title TEXT NULL,
            seo_description TEXT NULL,
            body TEXT NOT NULL,
            body_format TEXT NOT NULL DEFAULT 'markdown',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );

        CREATE TABLE blog_post_channel_visibility (
            tenant_id UUID NOT NULL,
            post_id UUID NOT NULL,
            channel_slug TEXT NOT NULL
        );

        CREATE TABLE blog_category_translations (
            tenant_id UUID NOT NULL,
            category_id UUID NOT NULL,
            locale TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL
        );
        "#,
    )
    .await?;
    Ok(())
}

async fn insert_blog_post(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
    status: &str,
    slug: &str,
    title: &str,
) -> Result<(), sea_orm::DbErr> {
    let translation_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO users (id, name)
        VALUES ('{author_id}', 'Search Author')
        ON CONFLICT (id) DO NOTHING;

        INSERT INTO blog_posts (
            id, tenant_id, author_id, status, slug, metadata, published_at,
            created_at, updated_at, comment_count, view_count, version
        ) VALUES (
            '{post_id}', '{tenant_id}', '{author_id}', '{status}', '{slug}',
            '{{"tags":["rust","cms"]}}'::jsonb,
            CASE WHEN '{status}' = 'published' THEN NOW() ELSE NULL END,
            NOW(), NOW(), 4, 12, 1
        );

        INSERT INTO blog_post_translations (
            id, post_id, locale, title, excerpt, seo_title, seo_description,
            body, body_format, created_at, updated_at
        ) VALUES (
            '{translation_id}', '{post_id}', 'en', '{title}', 'Excerpt',
            'SEO title', 'SEO description', 'Draft body', 'markdown', NOW(), NOW()
        );

        INSERT INTO blog_post_channel_visibility (tenant_id, post_id, channel_slug)
        VALUES ('{tenant_id}', '{post_id}', 'web');
        "#
    ))
    .await?;
    Ok(())
}

async fn insert_search_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    document_id: Uuid,
    source_module: &str,
    entity_type: &str,
    slug: &str,
) -> Result<(), sea_orm::DbErr> {
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO search_documents (
            document_key, tenant_id, document_id, source_module, entity_type,
            locale, status, is_public, title, slug, body, keywords_text,
            facets, payload, created_at, updated_at, indexed_at
        ) VALUES (
            '{source_module}:{document_id}:en', '{tenant_id}', '{document_id}',
            '{source_module}', '{entity_type}', 'en', 'published', TRUE,
            'Stale document', '{slug}', '', '', '{{}}'::jsonb,
            '{{"slug":"{slug}"}}'::jsonb, NOW(), NOW(), NOW()
        )
        "#
    ))
    .await?;
    Ok(())
}

async fn load_blog_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
) -> Result<Option<SearchDocumentSnapshot>, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT status, is_public, title, slug, locale, payload
            FROM search_documents
            WHERE tenant_id = $1
              AND document_id = $2
              AND source_module = 'blog'
              AND entity_type = 'blog_post'
            "#,
            vec![tenant_id.into(), post_id.into()],
        ))
        .await?;

    row.map(|row| {
        Ok(SearchDocumentSnapshot {
            status: row.try_get("", "status")?,
            is_public: row.try_get("", "is_public")?,
            title: row.try_get("", "title")?,
            slug: row.try_get("", "slug")?,
            locale: row.try_get("", "locale")?,
            payload: row.try_get("", "payload")?,
        })
    })
    .transpose()
}

async fn count_blog_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COUNT(*)::bigint AS count
            FROM search_documents
            WHERE tenant_id = $1
              AND source_module = 'blog'
              AND entity_type = 'blog_post'
            "#,
            vec![tenant_id.into()],
        ))
        .await?
        .expect("count query should return one row");
    Ok(row.try_get("", "count")?)
}

fn postgres_database_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
        .await?;
    Ok(())
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}
