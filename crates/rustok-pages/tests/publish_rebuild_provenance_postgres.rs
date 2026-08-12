use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput, SavePageDocumentInput,
};
use rustok_pages::entities::{
    page_body, page_publish_operation, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
    TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages publish rebuild provenance harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_publish_provenance_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let db = scoped_connection(&database_url, &schema_name).await?;
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule
            .migrations()
            .into_iter()
            .chain(PagesModule.migrations())
        {
            migration.up(&manager).await?;
        }
        Ok(Some(Self {
            control,
            database_url,
            schema_name,
        }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
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

#[tokio::test]
async fn reviewed_publish_retains_exact_provenance_and_rolls_back_aggregate_mismatch_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    enable_pages_module(&db, tenant_id).await?;
    let service = page_service(&db);
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "publish-rebuild-provenance-postgres",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let reviewed_input = || ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    };

    let en_project = project("home-en", "Home", "home", "English provenance");
    let fr_project = project("home-fr", "Accueil", "accueil", "French provenance");
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![
                    PageTranslationInput {
                        locale: "en".to_string(),
                        title: "Home".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "fr".to_string(),
                        title: "Accueil".to_string(),
                        slug: Some("accueil".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: en_project.clone(),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let en_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("English draft body is missing"))?
        .updated_at
        .clone();

    let fr_saved = service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: format!("page:{}:initial", draft.id),
                body: PageBodyInput {
                    locale: "fr".to_string(),
                    document: fr_project.clone(),
                },
            },
        )
        .await?;
    let fr_revision = fr_saved
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("French saved body is missing"))?
        .updated_at
        .clone();

    let published = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![
                    PageBodyRevisionInput {
                        locale: "fr".to_string(),
                        revision: fr_revision.clone(),
                    },
                    PageBodyRevisionInput {
                        locale: "en".to_string(),
                        revision: en_revision.clone(),
                    },
                ],
                idempotency_key: "publish-provenance-postgres-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;
    assert!(!published.replayed);

    let operation = page_publish_operation::Entity::find_by_id(published.operation_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish operation is missing"))?;
    let sources = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(operation.id))
        .order_by_asc(page_publish_rebuild_source::Column::Locale)
        .all(&db)
        .await?;
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources
            .iter()
            .map(|source| source.locale.as_str())
            .collect::<Vec<_>>(),
        vec!["en", "fr"]
    );

    assert_source_matches_published_locale(
        &db,
        &sources[0],
        tenant_id,
        draft.id,
        &en_revision,
        &reviewed.review_hash,
    )
    .await?;
    assert_source_matches_published_locale(
        &db,
        &sources[1],
        tenant_id,
        draft.id,
        &fr_revision,
        &reviewed.review_hash,
    )
    .await?;

    assert_aggregate_mismatch_rolls_back(
        &db,
        &operation,
        AggregateMismatch::ArtifactSet,
        "publish-provenance-artifact-mismatch",
    )
    .await?;
    assert_aggregate_mismatch_rolls_back(
        &db,
        &operation,
        AggregateMismatch::SanitizedSet,
        "publish-provenance-sanitized-mismatch",
    )
    .await?;

    assert_provenance_survives_artifact_row_loss(&db, &sources[1]).await?;

    database.cleanup().await
}

async fn assert_source_matches_published_locale(
    db: &DatabaseConnection,
    source: &page_publish_rebuild_source::Model,
    tenant_id: Uuid,
    page_id: Uuid,
    expected_revision: &str,
    expected_review_hash: &str,
) -> TestResult<()> {
    assert_eq!(source.tenant_id, tenant_id);
    assert_eq!(source.page_id, page_id);
    assert_eq!(source.source_format, PAGE_BUILDER_DOCUMENT_FORMAT);
    assert_eq!(source.source_revision, expected_revision);
    assert_eq!(source.review_hash, expected_review_hash);
    assert_eq!(source.sanitized_hash.len(), 64);
    assert_eq!(source.source_hash.len(), 64);
    assert_eq!(source.artifact_hash.len(), 64);
    assert_eq!(source.materialization_hash.len(), 64);
    assert_eq!(source.provenance_hash.len(), 64);
    assert!(source.sanitized_project.is_object());
    assert!(source.materialization_identity.is_object());

    let body = page_body::Entity::find_by_id(source.page_body_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("retained source body is missing"))?;
    assert_eq!(body.tenant_id, tenant_id);
    assert_eq!(body.page_id, page_id);
    assert_eq!(body.locale, source.locale);
    assert_eq!(body.format, source.source_format);
    assert_eq!(body.updated_at.to_string(), source.source_revision);

    let artifact = page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("retained source artifact is missing"))?;
    assert_eq!(artifact.tenant_id, tenant_id);
    assert_eq!(artifact.page_id, page_id);
    assert_eq!(artifact.locale, source.locale);
    assert_eq!(artifact.source_hash, source.source_hash);
    assert_eq!(artifact.artifact_hash, source.artifact_hash);
    assert_eq!(
        artifact.materialization_hash.as_deref(),
        Some(source.materialization_hash.as_str())
    );
    assert_eq!(
        artifact.materialization_identity.as_ref(),
        Some(&source.materialization_identity)
    );
    assert_eq!(
        artifact.runtime_snapshots.as_ref(),
        Some(&source.runtime_snapshots)
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum AggregateMismatch {
    ArtifactSet,
    SanitizedSet,
}

async fn assert_aggregate_mismatch_rolls_back(
    db: &DatabaseConnection,
    canonical: &page_publish_operation::Model,
    mismatch: AggregateMismatch,
    idempotency_key: &str,
) -> TestResult<()> {
    let operation_count_before = page_publish_operation::Entity::find()
        .filter(page_publish_operation::Column::TenantId.eq(canonical.tenant_id))
        .filter(page_publish_operation::Column::PageId.eq(canonical.page_id))
        .count(db)
        .await?;
    let source_count_before = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::TenantId.eq(canonical.tenant_id))
        .filter(page_publish_rebuild_source::Column::PageId.eq(canonical.page_id))
        .count(db)
        .await?;
    let manifest_count_before = page_publish_operation_artifact::Entity::find()
        .filter(page_publish_operation_artifact::Column::TenantId.eq(canonical.tenant_id))
        .filter(page_publish_operation_artifact::Column::PageId.eq(canonical.page_id))
        .count(db)
        .await?;

    let txn = db.begin().await?;
    let fake_operation_id = Uuid::new_v4();
    let mut active: page_publish_operation::ActiveModel = canonical.clone().into();
    active.id = Set(fake_operation_id);
    active.idempotency_key = Set(idempotency_key.to_string());
    active.request_hash = Set(different_sha256(&canonical.request_hash));
    match mismatch {
        AggregateMismatch::ArtifactSet => {
            active.artifact_set_hash = Set(different_sha256(&canonical.artifact_set_hash));
        }
        AggregateMismatch::SanitizedSet => {
            active.sanitized_set_hash = Set(different_sha256(&canonical.sanitized_set_hash));
        }
    }
    let insert_error = active
        .insert(&txn)
        .await
        .expect_err("mismatched publish aggregate must reject receipt persistence");
    let message = insert_error.to_string();
    match mismatch {
        AggregateMismatch::ArtifactSet => assert!(message.contains("artifact_set_hash")),
        AggregateMismatch::SanitizedSet => assert!(message.contains("sanitized_set_hash")),
    }
    txn.rollback().await?;

    assert!(
        page_publish_operation::Entity::find_by_id(fake_operation_id)
            .one(db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_publish_operation_artifact::Entity::find()
            .filter(page_publish_operation_artifact::Column::OperationId.eq(fake_operation_id))
            .count(db)
            .await?,
        0
    );
    assert_eq!(
        page_publish_rebuild_source::Entity::find()
            .filter(page_publish_rebuild_source::Column::OperationId.eq(fake_operation_id))
            .count(db)
            .await?,
        0
    );
    assert_eq!(
        page_publish_operation::Entity::find()
            .filter(page_publish_operation::Column::TenantId.eq(canonical.tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(canonical.page_id))
            .count(db)
            .await?,
        operation_count_before
    );
    assert_eq!(
        page_publish_rebuild_source::Entity::find()
            .filter(page_publish_rebuild_source::Column::TenantId.eq(canonical.tenant_id))
            .filter(page_publish_rebuild_source::Column::PageId.eq(canonical.page_id))
            .count(db)
            .await?,
        source_count_before
    );
    assert_eq!(
        page_publish_operation_artifact::Entity::find()
            .filter(page_publish_operation_artifact::Column::TenantId.eq(canonical.tenant_id))
            .filter(page_publish_operation_artifact::Column::PageId.eq(canonical.page_id))
            .count(db)
            .await?,
        manifest_count_before
    );
    Ok(())
}

async fn assert_provenance_survives_artifact_row_loss(
    db: &DatabaseConnection,
    source: &page_publish_rebuild_source::Model,
) -> TestResult<()> {
    let retained_before = source.clone();
    page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(source.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(source.page_id))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    page_publish_operation_artifact::Entity::delete_many()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(source.operation_id))
        .filter(page_publish_operation_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    let deleted = page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)
        .exec(db)
        .await?;
    assert_eq!(deleted.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
            .one(db)
            .await?
            .is_none()
    );

    let retained_after = page_publish_rebuild_source::Entity::find_by_id(source.id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("provenance disappeared with artifact row"))?;
    assert_eq!(retained_after, retained_before);
    assert_eq!(retained_after.artifact_id, source.artifact_id);
    Ok(())
}

fn different_sha256(value: &str) -> String {
    let replacement = if value.starts_with('0') { '1' } else { '0' };
    format!("{replacement}{}", &value[1..])
}

fn project(id: &str, title: &str, slug: &str, content: &str) -> serde_json::Value {
    json!({
        "pages": [{
            "id": id,
            "flyPageMeta": {
                "title": title,
                "description": content,
                "slug": slug
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": content
                }]
            }
        }]
    })
}

fn page_service(db: &DatabaseConnection) -> PageService {
    PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    )
}

async fn enable_pages_module(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (\
            id UUID PRIMARY KEY NOT NULL, \
            tenant_id UUID NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled BOOLEAN NOT NULL, \
            settings JSONB NOT NULL, \
            created_at TIMESTAMPTZ NOT NULL, \
            updated_at TIMESTAMPTZ NOT NULL\
        )",
    )
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) \
         VALUES ($1, $2, 'pages', TRUE, '{}'::jsonb, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![Uuid::new_v4().into(), tenant_id.into()],
    ))
    .await?;
    Ok(())
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
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

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
