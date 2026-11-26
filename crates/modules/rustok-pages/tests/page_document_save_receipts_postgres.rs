//! Opt-in PostgreSQL writer-serialization coverage across *independent* pools.
//! Does not run without RUSTOK_PAGES_TEST_DATABASE_URL or DATABASE_URL.
//! These scenarios are authored, not executed by the source-only contributor.

use std::{env, error::Error};

use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::OutboxModule;
use rustok_pages::entities::{page, page_body, page_body_revision, page_translation};
use rustok_pages::{
    CreatePageInput, PageBodyInput, PageService, PagesModule, PageTranslationInput,
    RestorePageBodyRevisionInput, SavePageDocumentInput,
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct ScopedPg {
    control: DatabaseConnection,
    url: String,
    schema: String,
}

impl ScopedPg {
    async fn new() -> TestResult<Option<Self>> {
        let Some(url) = env::var("RUSTOK_PAGES_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .ok()
            .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
        else {
            tracing::info!("No PostgreSQL test URL: skipping page document save receipts test");
            return Ok(None);
        };
        let control = connect(&url).await?;
        let schema = format!("pages_save_receipts_{}", Uuid::new_v4().simple());
        control.execute_unprepared(&format!(r#"CREATE SCHEMA "{schema}""#)).await?;
        let db = scoped_connection(&url, &schema).await?;
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule
            .migrations()
            .into_iter()
            .chain(PagesModule.migrations())
        {
            migration.up(&manager).await?;
        }
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS tenant_modules (
                id UUID PRIMARY KEY,
                tenant_id UUID NOT NULL,
                module_slug TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                settings JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            )",
        ).await?;
        Ok(Some(Self { control, url, schema }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.url, &self.schema).await
    }

    async fn close(self) -> TestResult<()> {
        self.control.execute_unprepared(&format!(
            r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
            self.schema
        )).await?;
        Ok(())
    }
}

async fn connect(url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(url.to_string());
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(url: &str, schema: &str) -> TestResult<DatabaseConnection> {
    let db = connect(url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#)).await?;
    Ok(db)
}

async fn seed_page(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
) -> TestResult<(Uuid, String)> {
    let page_id = Uuid::new_v4();
    let now: sea_orm::prelude::DateTimeWithTimeZone =
        chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
            .expect("current timestamp")
            .into();
    page::ActiveModel {
        id: Set(page_id),
        tenant_id: Set(tenant_id),
        author_id: Set(None),
        status: Set("published".to_string()),
        template: Set("default".to_string()),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        archived_at: Set(None),
        version: Set(1),
    }.insert(db).await?;
    page_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        page_id: Set(page_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        title: Set("Concurrent save".to_string()),
        slug: Set(slug.to_string()),
        meta_title: Set(None),
        meta_description: Set(None),
        revision: Set(1),
    }.insert(db).await?;
    page_body::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set("en".to_string()),
        content: Set(builder_document("initial").to_string()),
        format: Set(rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT.to_string()),
        updated_at: Set(now),
    }.insert(db).await?;
    Ok((page_id, now.to_string()))
}

fn builder_document(label: &str) -> serde_json::Value {
    json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Concurrent save",
                "description": "Receipt concurrency regression",
                "slug": "home"
            },
            "component": {
                "id": "root", "type": "wrapper",
                "components": [{
                    "id": "heading", "type": "heading",
                    "tagName": "h1", "content": label
                }]
            }
        }]
    })
}

fn command(expected_revision: &str, label: &str) -> SavePageDocumentInput {
    SavePageDocumentInput {
        expected_revision: expected_revision.to_string(),
        body: PageBodyInput {
            locale: "en".to_string(),
            document: builder_document(label),
        },
    }
}

async fn revision_count(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<usize> {
    Ok(page_body_revision::Entity::find()
        .filter(page_body_revision::Column::TenantId.eq(tenant_id))
        .filter(page_body_revision::Column::PageId.eq(page_id))
        .all(db).await?.len())
}

#[tokio::test]
async fn postgres_duplicate_and_distinct_saves_are_serialized_across_connections()
-> TestResult<()> {
    let Some(db_scope) = ScopedPg::new().await? else { return Ok(()); };
    let primary = db_scope.connection().await?;
    let secondary = db_scope.connection().await?;
    let tenant_id = Uuid::new_v4();
    let user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let a = PageService::new(primary.clone(), mock_transactional_event_bus());
    let b = PageService::new(secondary.clone(), mock_transactional_event_bus());

    let (duplicate_id, base) = seed_page(&primary, tenant_id, "duplicate-saves").await?;
    let duplicate = command(&base, "same-body");
    let (left, right) = tokio::join!(
        a.save_document_with_receipt(
            tenant_id, user.clone(), duplicate_id,
            duplicate.clone(), Some("postgres:duplicate".into()),
        ),
        b.save_document_with_receipt(
            tenant_id, user.clone(), duplicate_id,
            duplicate.clone(), Some("postgres:duplicate".into()),
        ),
    );
    let first = left?;
    let second = right?;
    assert_eq!(
        serde_json::to_value(first)?,
        serde_json::to_value(second)?
    );
    assert_eq!(revision_count(&primary, tenant_id, duplicate_id).await?, 1);

    let (competing_id, base) = seed_page(&primary, tenant_id, "competing-saves").await?;
    let (left, right) = tokio::join!(
        a.save_document_with_receipt(
            tenant_id, user.clone(), competing_id,
            command(&base, "left"), Some("postgres:left".into()),
        ),
        b.save_document_with_receipt(
            tenant_id, user.clone(), competing_id,
            command(&base, "right"), Some("postgres:right".into()),
        ),
    );
    assert_ne!(left.is_ok(), right.is_ok(), "exactly one distinct writer must win CAS");
    let loser = if left.is_err() { left.err() } else { right.err() }
        .expect("the other writer must be rejected");
    assert!(
        loser.to_string().contains("PAGE_DOCUMENT_REVISION_CONFLICT"),
        "stale writer must receive typed CAS conflict: {loser}"
    );
    assert_eq!(revision_count(&secondary, tenant_id, competing_id).await?, 1);
    let persisted_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(competing_id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(&secondary)
        .await?
        .expect("the winning writer's body remains persisted");
    let persisted_document: serde_json::Value =
        serde_json::from_str(&persisted_body.content)?;
    let persisted_label = persisted_document["pages"][0]["component"]["components"][0]
        ["content"]
        .as_str();
    assert!(
        matches!(persisted_label, Some("left") | Some("right")),
        "independent connections must persist exactly one complete winner, got {persisted_label:?}"
    );

    // The original request can still be queried across a second process/
    // connection, not only from the service instance that performed the save.
    let recovered = b.document_save_receipt(
        tenant_id, user, duplicate_id, "postgres:duplicate",
    ).await?.expect("durable original acknowledgement");
    assert_eq!(recovered.id, duplicate_id);
    db_scope.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_concurrent_page_create_replays_across_independent_connections()
-> TestResult<()> {
    let Some(db_scope) = ScopedPg::new().await? else {
        return Ok(());
    };
    let primary = db_scope.connection().await?;
    let secondary = db_scope.connection().await?;
    let tenant_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let service = PageService::new(primary.clone(), mock_transactional_event_bus());
    let peer_service = PageService::new(secondary.clone(), mock_transactional_event_bus());
    let input = CreatePageInput {
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "Concurrent create".to_string(),
            slug: Some("concurrent-create".to_string()),
            meta_title: None,
            meta_description: None,
        }],
        template: Some("default".to_string()),
        body: None,
        channel_slugs: None,
        publish: false,
    };

    let (left, right) = tokio::join!(
        service.create_with_receipt(
            tenant_id,
            security.clone(),
            input.clone(),
            true,
            "pages-postgres-create-concurrent-1".to_string(),
        ),
        peer_service.create_with_receipt(
            tenant_id,
            security,
            input,
            true,
            "pages-postgres-create-concurrent-1".to_string(),
        ),
    );
    let first = left?;
    let second = right?;
    assert_eq!(first.id, second.id);
    assert_eq!(
        page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .all(&primary)
            .await?
            .len(),
        1,
        "two PostgreSQL pools must commit one page for one create key"
    );

    db_scope.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_concurrent_unpublish_replays_across_independent_connections()
-> TestResult<()> {
    let Some(db_scope) = ScopedPg::new().await? else {
        return Ok(());
    };
    let primary = db_scope.connection().await?;
    let secondary = db_scope.connection().await?;
    let tenant_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let service = PageService::new(primary.clone(), mock_transactional_event_bus());
    let peer_service = PageService::new(secondary.clone(), mock_transactional_event_bus());
    let (page_id, _) = seed_page(&primary, tenant_id, "concurrent-unpublish").await?;

    let (left, right) = tokio::join!(
        service.unpublish_with_receipt(
            tenant_id,
            security.clone(),
            page_id,
            Some(1),
            "pages-postgres-unpublish-1".to_string(),
        ),
        peer_service.unpublish_with_receipt(
            tenant_id,
            security,
            page_id,
            Some(1),
            "pages-postgres-unpublish-1".to_string(),
        ),
    );
    let first = left?;
    let second = right?;
    assert_eq!(serde_json::to_value(&first)?, serde_json::to_value(&second)?);
    assert_eq!(second.version, 2);
    assert_eq!(
        second.status,
        rustok_content::entities::node::ContentStatus::Draft
    );

    db_scope.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_concurrent_delete_replays_after_the_page_row_is_removed()
-> TestResult<()> {
    let Some(db_scope) = ScopedPg::new().await? else {
        return Ok(());
    };
    let primary = db_scope.connection().await?;
    let secondary = db_scope.connection().await?;
    let tenant_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let service = PageService::new(primary.clone(), mock_transactional_event_bus());
    let peer_service = PageService::new(secondary.clone(), mock_transactional_event_bus());
    let created = service
        .create(
            tenant_id,
            security.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Concurrent delete".to_string(),
                    slug: Some("concurrent-delete".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    let (left, right) = tokio::join!(
        service.delete_with_receipt(
            tenant_id,
            security.clone(),
            created.id,
            Some(created.version),
            "pages-postgres-delete-1".to_string(),
        ),
        peer_service.delete_with_receipt(
            tenant_id,
            security,
            created.id,
            Some(created.version),
            "pages-postgres-delete-1".to_string(),
        ),
    );
    left?;
    right?;
    assert!(
        page::Entity::find_by_id(created.id).one(&primary).await?.is_none(),
        "the page row must be deleted once"
    );

    db_scope.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_duplicate_restores_replay_across_independent_connections()
-> TestResult<()> {
    let Some(db_scope) = ScopedPg::new().await? else {
        return Ok(());
    };
    let primary = db_scope.connection().await?;
    let secondary = db_scope.connection().await?;
    let tenant_id = Uuid::new_v4();
    let user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let a = PageService::new(primary.clone(), mock_transactional_event_bus());
    let b = PageService::new(secondary.clone(), mock_transactional_event_bus());

    let (page_id, base) = seed_page(&primary, tenant_id, "duplicate-restores").await?;
    let v2 = a
        .save_document(
            tenant_id,
            user.clone(),
            page_id,
            command(&base, "v2"),
        )
        .await?;
    let v2_revision = v2.body.expect("v2 body").updated_at;
    let v3 = a
        .save_document(
            tenant_id,
            user.clone(),
            page_id,
            command(&v2_revision, "v3"),
        )
        .await?;
    let v3_revision = v3.body.expect("v3 body").updated_at;
    let history = a
        .body_revision_history(tenant_id, user.clone(), page_id, "en")
        .await?;
    let v2_revision_id = history[1].id;
    let restore = RestorePageBodyRevisionInput {
        expected_revision: v3_revision,
        revision_id: v2_revision_id,
    };

    let (left, right) = tokio::join!(
        a.restore_body_revision(tenant_id, user.clone(), page_id, restore.clone()),
        b.restore_body_revision(tenant_id, user, page_id, restore),
    );
    let first = left?;
    let second = right?;
    assert_eq!(serde_json::to_value(&first)?, serde_json::to_value(&second)?);
    assert_eq!(
        revision_count(&primary, tenant_id, page_id).await?,
        3,
        "two saves and one restore must create exactly three journal rows"
    );

    db_scope.close().await?;
    Ok(())
}
