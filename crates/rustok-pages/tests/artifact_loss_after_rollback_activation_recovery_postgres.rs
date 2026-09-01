use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
    RollbackPageInput, SavePageDocumentInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_body, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_rollback_operation,
    page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{PagesError, PagesModule};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Statement,
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
    async fn setup(label: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages rollback-activated artifact-loss recovery harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_rollback_activated_recovery_{}_{}",
            label,
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

struct RollbackActivatedFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    first_en_source: page_publish_rebuild_source::Model,
    first_fr_source: page_publish_rebuild_source::Model,
    rollback_operation_id: Uuid,
    rollback_version: i32,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn rollback_activated_publish_recovers_two_lost_locales_sequentially_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_rollback_activated_fixture(&db, &service, "rollback-anchor-success").await?;

    remove_source_artifact(&db, &fixture.first_en_source).await?;
    remove_source_artifact(&db, &fixture.first_fr_source).await?;

    let en_rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.first_en_source.id,
                expected_provenance_hash: fixture.first_en_source.provenance_hash.clone(),
                idempotency_key: "rollback-anchor-rebuild-en-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;
    let fr_rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.first_fr_source.id,
                expected_provenance_hash: fixture.first_fr_source.provenance_hash.clone(),
                idempotency_key: "rollback-anchor-rebuild-fr-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;

    let en_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: en_rebuild.operation_id,
                expected_version: fixture.rollback_version,
                expected_current_artifact_id: fixture.first_en_source.artifact_id,
                idempotency_key: "rollback-anchor-activate-en-v1".to_string(),
            },
        )
        .await?;
    assert_eq!(en_activation.version, fixture.rollback_version + 1);

    let fr_input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: fr_rebuild.operation_id,
        expected_version: en_activation.version,
        expected_current_artifact_id: fixture.first_fr_source.artifact_id,
        idempotency_key: "rollback-anchor-activate-fr-v1".to_string(),
    };
    let fr_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            fr_input.clone(),
        )
        .await?;
    assert_eq!(fr_activation.version, fixture.rollback_version + 2);

    let bindings = current_bindings(&db, fixture.tenant_id, fixture.page_id).await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].locale, "en");
    assert_eq!(bindings[0].artifact_id, en_rebuild.rebuilt_artifact_id);
    assert_eq!(bindings[1].locale, "fr");
    assert_eq!(bindings[1].artifact_id, fr_rebuild.rebuilt_artifact_id);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.first_en_source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.first_fr_source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        2
    );

    let replay = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            fr_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, fr_activation.operation_id);
    assert_eq!(replay.version, fr_activation.version);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        2
    );

    database.cleanup().await
}

#[tokio::test]
async fn rollback_activated_recovery_rejects_noncanonical_rollback_anchor_hash_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("anchor_hash").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_rollback_activated_fixture(&db, &service, "rollback-anchor-hash").await?;

    let rollback = page_rollback_operation::Entity::find_by_id(fixture.rollback_operation_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("rollback anchor receipt is missing"))?;
    let tampered_hash = different_sha256(&rollback.request_hash);
    let changed = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_rollback_operations SET request_hash = $1 WHERE id = $2",
            vec![tampered_hash.into(), rollback.id.into()],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    remove_source_artifact(&db, &fixture.first_en_source).await?;
    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.first_en_source.id,
                expected_provenance_hash: fixture.first_en_source.provenance_hash.clone(),
                idempotency_key: "rollback-anchor-hash-rebuild-en-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;

    let before = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before rollback-anchor rejection"))?;
    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: fixture.rollback_version,
                expected_current_artifact_id: fixture.first_en_source.artifact_id,
                idempotency_key: "rollback-anchor-hash-activate-en-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let after = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after rollback-anchor rejection"))?;
    assert_eq!(after.version, before.version);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        0
    );
    assert!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
            .filter(page_published_landing_artifact::Column::Locale.eq("en"))
            .one(&db)
            .await?
            .is_none()
    );

    database.cleanup().await
}

#[tokio::test]
async fn rollback_activated_recovery_rejects_unexplained_version_drift_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("version_drift").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_rollback_activated_fixture(&db, &service, "rollback-anchor-version-drift").await?;

    remove_source_artifact(&db, &fixture.first_en_source).await?;
    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.first_en_source.id,
                expected_provenance_hash: fixture.first_en_source.provenance_hash.clone(),
                idempotency_key: "rollback-anchor-drift-rebuild-en-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;

    let drifted_version = fixture.rollback_version + 1;
    let changed = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE pages SET version = $1 WHERE id = $2 AND tenant_id = $3",
            vec![
                drifted_version.into(),
                fixture.page_id.into(),
                fixture.tenant_id.into(),
            ],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: drifted_version,
                expected_current_artifact_id: fixture.first_en_source.artifact_id,
                idempotency_key: "rollback-anchor-drift-activate-en-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let current = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after drift rejection"))?;
    assert_eq!(current.version, drifted_version);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        0
    );
    assert!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
            .filter(page_published_landing_artifact::Column::Locale.eq("en"))
            .one(&db)
            .await?
            .is_none()
    );

    database.cleanup().await
}

async fn create_rollback_activated_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<RollbackActivatedFixture> {
    let tenant_id = Uuid::new_v4();
    enable_pages_module(db, tenant_id).await?;
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        scenario,
        json!({ "surface": "storefront", "channel": "web" }),
    )?;

    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![
                    PageTranslationInput {
                        locale: "en".to_string(),
                        title: "Rollback activated A".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "fr".to_string(),
                        title: "Rollback activé A".to_string(),
                        slug: Some("accueil".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home-en", "Rollback activated A EN", "home")?,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let first_en_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("first English body is missing"))?
        .updated_at
        .clone();
    let first_fr_saved = service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: format!("page:{}:initial", draft.id),
                body: PageBodyInput {
                    locale: "fr".to_string(),
                    document: project_json("home-fr", "Rollback activé A FR", "accueil")?,
                },
            },
        )
        .await?;
    let first_fr_revision = first_fr_saved
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("first French body is missing"))?
        .updated_at
        .clone();
    let first_publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![
                    PageBodyRevisionInput {
                        locale: "en".to_string(),
                        revision: first_en_revision,
                    },
                    PageBodyRevisionInput {
                        locale: "fr".to_string(),
                        revision: first_fr_revision,
                    },
                ],
                idempotency_key: format!("{scenario}-publish-a-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let first_sources = publish_sources(db, first_publish.operation_id).await?;
    assert_eq!(first_sources.len(), 2);

    let unpublished = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(first_publish.version),
        )
        .await?;

    let en_before = body_for_locale(db, tenant_id, draft.id, "en").await?;
    service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: en_before.updated_at.to_string(),
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home-en", "Rollback activated B EN", "home")?,
                },
            },
        )
        .await?;
    let fr_before = body_for_locale(db, tenant_id, draft.id, "fr").await?;
    service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: fr_before.updated_at.to_string(),
                body: PageBodyInput {
                    locale: "fr".to_string(),
                    document: project_json("home-fr", "Rollback activé B FR", "accueil")?,
                },
            },
        )
        .await?;
    let second_en = body_for_locale(db, tenant_id, draft.id, "en").await?;
    let second_fr = body_for_locale(db, tenant_id, draft.id, "fr").await?;
    let second_publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: unpublished.version,
                expected_body_revisions: vec![
                    PageBodyRevisionInput {
                        locale: "en".to_string(),
                        revision: second_en.updated_at.to_string(),
                    },
                    PageBodyRevisionInput {
                        locale: "fr".to_string(),
                        revision: second_fr.updated_at.to_string(),
                    },
                ],
                idempotency_key: format!("{scenario}-publish-b-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;

    let rollback = service
        .rollback_to_previous(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            RollbackPageInput {
                expected_version: second_publish.version,
                idempotency_key: format!("{scenario}-rollback-to-a-v1"),
            },
        )
        .await?;
    assert_eq!(
        rollback.target_publish_operation_id,
        first_publish.operation_id
    );
    assert_eq!(rollback.version, second_publish.version + 1);

    let bindings = current_bindings(db, tenant_id, draft.id).await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].artifact_id, first_sources[0].artifact_id);
    assert_eq!(bindings[1].artifact_id, first_sources[1].artifact_id);

    Ok(RollbackActivatedFixture {
        tenant_id,
        page_id: draft.id,
        first_en_source: first_sources[0].clone(),
        first_fr_source: first_sources[1].clone(),
        rollback_operation_id: rollback.operation_id,
        rollback_version: rollback.version,
        reviewed,
    })
}

async fn publish_sources(
    db: &DatabaseConnection,
    operation_id: Uuid,
) -> TestResult<Vec<page_publish_rebuild_source::Model>> {
    let sources = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(operation_id))
        .order_by_asc(page_publish_rebuild_source::Column::Locale)
        .all(db)
        .await?;
    if sources.len() == 2 && sources[0].locale == "en" && sources[1].locale == "fr" {
        Ok(sources)
    } else {
        Err(std::io::Error::other("publish did not retain exact en/fr provenance").into())
    }
}

async fn body_for_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
) -> TestResult<page_body::Model> {
    Ok(page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(page_id))
        .filter(page_body::Column::Locale.eq(locale))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other(format!("body `{locale}` is missing")))?)
}

async fn remove_source_artifact(
    db: &DatabaseConnection,
    source: &page_publish_rebuild_source::Model,
) -> TestResult<()> {
    let removed_binding = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(source.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(source.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq(&source.locale))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_binding.rows_affected, 1);
    let removed_manifest = page_publish_operation_artifact::Entity::delete_many()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(source.operation_id))
        .filter(page_publish_operation_artifact::Column::Locale.eq(&source.locale))
        .filter(page_publish_operation_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_manifest.rows_affected, 1);
    let deleted = page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)
        .exec(db)
        .await?;
    assert_eq!(deleted.rows_affected, 1);
    Ok(())
}

async fn current_bindings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<Vec<page_published_landing_artifact::Model>> {
    Ok(page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .order_by_asc(page_published_landing_artifact::Column::Locale)
        .all(db)
        .await?)
}

fn different_sha256(value: &str) -> String {
    if value.starts_with('f') {
        "e".repeat(64)
    } else {
        "f".repeat(64)
    }
}

fn reviewed_input(reviewed: &PageBuilderReviewedPublishRuntime) -> ReviewedPagePublishRuntimeInput {
    ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    }
}

fn project_json(page_id: &str, title: &str, slug: &str) -> TestResult<serde_json::Value> {
    Ok(json!({
        "pages": [{
            "id": page_id,
            "flyPageMeta": {
                "title": title,
                "description": "Rollback-activated artifact-loss recovery",
                "slug": slug
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": title
                }]
            }
        }]
    }))
}

fn page_service(db: &DatabaseConnection) -> PageService {
    PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    )
}

async fn enable_pages_module(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (\
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
    db.execute_raw(Statement::from_sql_and_values(
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
