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
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT, PagesError, PagesModule};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Statement,
};
use sea_orm_migration::SchemaManager;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages multi-locale repair rollback evidence harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_multilocale_rollback_evidence_{}_{}",
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

struct MultiPublishFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    first_publish_operation_id: Uuid,
    first_en_artifact_id: Uuid,
    first_fr_artifact_id: Uuid,
    second_publish_operation_id: Uuid,
    second_publish_version: i32,
    second_en_source: page_publish_rebuild_source::Model,
    second_fr_source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

struct RecoveredCurrent {
    en_activation_operation_id: Uuid,
    en_rebuilt_artifact_id: Uuid,
    fr_rebuilt_artifact_id: Uuid,
    current_version: i32,
}

#[tokio::test]
async fn rollback_continues_after_two_locale_physical_loss_recovery_on_postgres() -> TestResult<()>
{
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_multi_publish_fixture(&db, &service, "multilocale-rollback-success").await?;
    let recovered = recover_second_publish(&db, &service, &fixture, "success").await?;

    let input = RollbackPageInput {
        expected_version: recovered.current_version,
        idempotency_key: "multilocale-repair-rollback-v1".to_string(),
    };
    let rollback = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input.clone(),
        )
        .await?;
    assert!(!rollback.replayed);
    assert_eq!(
        rollback.target_publish_operation_id,
        fixture.first_publish_operation_id
    );
    assert_eq!(rollback.version, recovered.current_version + 1);

    let bindings = current_bindings(&db, &fixture).await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].locale, "en");
    assert_eq!(bindings[0].artifact_id, fixture.first_en_artifact_id);
    assert_eq!(bindings[1].locale, "fr");
    assert_eq!(bindings[1].artifact_id, fixture.first_fr_artifact_id);

    let replay = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, rollback.operation_id);
    assert_eq!(replay.version, rollback.version);

    database.cleanup().await
}

#[tokio::test]
async fn rollback_rejects_repaired_cursor_with_noncanonical_activation_request_hash_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("request_hash").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_multi_publish_fixture(&db, &service, "multilocale-rollback-request-hash").await?;
    let recovered = recover_second_publish(&db, &service, &fixture, "request-hash").await?;

    let activation = page_artifact_binding_replacement_operation::Entity::find_by_id(
        recovered.en_activation_operation_id,
    )
    .one(&db)
    .await?
    .ok_or_else(|| std::io::Error::other("English activation receipt is missing"))?;
    let tampered_hash = different_sha256(&activation.request_hash);
    let changed = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_artifact_binding_replacement_operations SET request_hash = $1 WHERE id = $2",
            vec![tampered_hash.into(), activation.id.into()],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    assert_rollback_rejected_without_binding_change(
        &db,
        &service,
        &fixture,
        &recovered,
        "multilocale-request-hash-rollback-v1",
    )
    .await?;

    database.cleanup().await
}

#[tokio::test]
async fn rollback_rejects_individually_valid_but_noncontiguous_activation_prefix_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("prefix_gap").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_multi_publish_fixture(&db, &service, "multilocale-rollback-prefix-gap").await?;
    let recovered = recover_second_publish(&db, &service, &fixture, "prefix-gap").await?;

    let activation = page_artifact_binding_replacement_operation::Entity::find_by_id(
        recovered.en_activation_operation_id,
    )
    .one(&db)
    .await?
    .ok_or_else(|| std::io::Error::other("English activation receipt is missing"))?;
    let tampered_expected_version = fixture.second_publish_version + 1;
    let tampered_result_version = tampered_expected_version + 1;
    assert_eq!(tampered_result_version, recovered.current_version);
    let tampered_request_hash = activation_request_hash(
        activation.tenant_id,
        activation.page_id,
        activation.rebuild_operation_id,
        tampered_expected_version,
        activation.expected_current_artifact_id,
    )?;
    let changed = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_artifact_binding_replacement_operations SET expected_version = $1, result_version = $2, request_hash = $3 WHERE id = $4",
            vec![
                tampered_expected_version.into(),
                tampered_result_version.into(),
                tampered_request_hash.into(),
                activation.id.into(),
            ],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    assert_rollback_rejected_without_binding_change(
        &db,
        &service,
        &fixture,
        &recovered,
        "multilocale-prefix-gap-rollback-v1",
    )
    .await?;

    database.cleanup().await
}

async fn assert_rollback_rejected_without_binding_change(
    db: &DatabaseConnection,
    service: &PageService,
    fixture: &MultiPublishFixture,
    recovered: &RecoveredCurrent,
    idempotency_key: &str,
) -> TestResult<()> {
    let before = page::Entity::find_by_id(fixture.page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before rollback rejection"))?;
    assert_eq!(before.version, recovered.current_version);
    let result = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RollbackPageInput {
                expected_version: recovered.current_version,
                idempotency_key: idempotency_key.to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let after = page::Entity::find_by_id(fixture.page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after rollback rejection"))?;
    assert_eq!(after.version, before.version);

    let bindings = current_bindings(db, fixture).await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].artifact_id, recovered.en_rebuilt_artifact_id);
    assert_eq!(bindings[1].artifact_id, recovered.fr_rebuilt_artifact_id);
    Ok(())
}

async fn recover_second_publish(
    db: &DatabaseConnection,
    service: &PageService,
    fixture: &MultiPublishFixture,
    key_prefix: &str,
) -> TestResult<RecoveredCurrent> {
    remove_source_artifact(db, &fixture.second_en_source).await?;
    remove_source_artifact(db, &fixture.second_fr_source).await?;

    let en_rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.second_en_source.id,
                expected_provenance_hash: fixture.second_en_source.provenance_hash.clone(),
                idempotency_key: format!("{key_prefix}-rebuild-en-v1"),
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
                source_id: fixture.second_fr_source.id,
                expected_provenance_hash: fixture.second_fr_source.provenance_hash.clone(),
                idempotency_key: format!("{key_prefix}-rebuild-fr-v1"),
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
                expected_version: fixture.second_publish_version,
                expected_current_artifact_id: fixture.second_en_source.artifact_id,
                idempotency_key: format!("{key_prefix}-activate-en-v1"),
            },
        )
        .await?;
    let fr_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: fr_rebuild.operation_id,
                expected_version: en_activation.version,
                expected_current_artifact_id: fixture.second_fr_source.artifact_id,
                idempotency_key: format!("{key_prefix}-activate-fr-v1"),
            },
        )
        .await?;
    assert_eq!(fr_activation.version, fixture.second_publish_version + 2);
    assert_eq!(
        page_publish_operation_artifact::Entity::find()
            .filter(
                page_publish_operation_artifact::Column::OperationId
                    .eq(fixture.second_publish_operation_id),
            )
            .count(db)
            .await?,
        0
    );

    Ok(RecoveredCurrent {
        en_activation_operation_id: en_activation.operation_id,
        en_rebuilt_artifact_id: en_rebuild.rebuilt_artifact_id,
        fr_rebuilt_artifact_id: fr_rebuild.rebuilt_artifact_id,
        current_version: fr_activation.version,
    })
}

async fn create_multi_publish_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<MultiPublishFixture> {
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
                        title: "Repair rollback A".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "fr".to_string(),
                        title: "Réparation rollback A".to_string(),
                        slug: Some("accueil".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home-en", "Repair rollback A EN", "home")?,
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
                    document: project_json("home-fr", "Réparation rollback A FR", "accueil")?,
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
                    document: project_json("home-en", "Repair rollback B EN", "home")?,
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
                    document: project_json("home-fr", "Réparation rollback B FR", "accueil")?,
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
    let second_sources = publish_sources(db, second_publish.operation_id).await?;
    assert_eq!(second_sources.len(), 2);

    Ok(MultiPublishFixture {
        tenant_id,
        page_id: draft.id,
        first_publish_operation_id: first_publish.operation_id,
        first_en_artifact_id: first_sources[0].artifact_id,
        first_fr_artifact_id: first_sources[1].artifact_id,
        second_publish_operation_id: second_publish.operation_id,
        second_publish_version: second_publish.version,
        second_en_source: second_sources[0].clone(),
        second_fr_source: second_sources[1].clone(),
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
    fixture: &MultiPublishFixture,
) -> TestResult<Vec<page_published_landing_artifact::Model>> {
    Ok(page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .order_by_asc(page_published_landing_artifact::Column::Locale)
        .all(db)
        .await?)
}

fn activation_request_hash(
    tenant_id: Uuid,
    page_id: Uuid,
    rebuild_operation_id: Uuid,
    expected_version: i32,
    expected_current_artifact_id: Uuid,
) -> TestResult<String> {
    stable_hash(&(
        PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT,
        tenant_id,
        page_id,
        rebuild_operation_id,
        expected_version,
        expected_current_artifact_id,
    ))
}

fn stable_hash(value: &impl Serialize) -> TestResult<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
                "description": "Multi-locale repair rollback evidence",
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
