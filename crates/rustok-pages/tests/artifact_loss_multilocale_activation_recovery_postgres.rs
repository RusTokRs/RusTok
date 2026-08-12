use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
    SavePageDocumentInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT, PagesError, PagesModule};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages multi-locale artifact-loss activation recovery harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_multilocale_recovery_{}_{}",
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

struct MultiLocaleFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    publish_version: i32,
    en_source: page_publish_rebuild_source::Model,
    fr_source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn missing_binding_activation_recovers_two_lost_locales_sequentially_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_multilocale_fixture(&db, &service, "multilocale-recovery-success").await?;

    remove_binding_manifest_and_source_artifact(&db, &fixture.en_source).await?;
    remove_binding_manifest_and_source_artifact(&db, &fixture.fr_source).await?;
    let en_rebuild = rebuild_source(
        &service,
        &fixture,
        &fixture.en_source,
        "multilocale-rebuild-en-v1",
    )
    .await?;
    let fr_rebuild = rebuild_source(
        &service,
        &fixture,
        &fixture.fr_source,
        "multilocale-rebuild-fr-v1",
    )
    .await?;
    let events_before_activation = SysEvents::find().count(&db).await?;

    let en_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: en_rebuild.operation_id,
                expected_version: fixture.publish_version,
                expected_current_artifact_id: fixture.en_source.artifact_id,
                idempotency_key: "multilocale-activate-en-v1".to_string(),
            },
        )
        .await?;
    assert_eq!(en_activation.version, fixture.publish_version + 1);

    let fr_activation_input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: fr_rebuild.operation_id,
        expected_version: en_activation.version,
        expected_current_artifact_id: fixture.fr_source.artifact_id,
        idempotency_key: "multilocale-activate-fr-v1".to_string(),
    };
    let fr_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            fr_activation_input.clone(),
        )
        .await?;
    assert_eq!(fr_activation.version, fixture.publish_version + 2);
    assert_eq!(
        fr_activation.replacement_artifact_id,
        fr_rebuild.rebuilt_artifact_id
    );

    let bindings = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .order_by_asc(page_published_landing_artifact::Column::Locale)
        .all(&db)
        .await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].locale, "en");
    assert_eq!(bindings[0].artifact_id, en_rebuild.rebuilt_artifact_id);
    assert_eq!(bindings[1].locale, "fr");
    assert_eq!(bindings[1].artifact_id, fr_rebuild.rebuilt_artifact_id);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.en_source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.fr_source.artifact_id)
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
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_activation + 4
    );

    let replay = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            fr_activation_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, fr_activation.operation_id);
    assert_eq!(replay.version, fr_activation.version);
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_activation + 4
    );

    database.cleanup().await
}

#[tokio::test]
async fn missing_binding_activation_rejects_unexplained_version_between_locales_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("unexplained_version").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_multilocale_fixture(&db, &service, "multilocale-recovery-gap").await?;

    remove_binding_manifest_and_source_artifact(&db, &fixture.en_source).await?;
    remove_binding_manifest_and_source_artifact(&db, &fixture.fr_source).await?;
    let en_rebuild = rebuild_source(
        &service,
        &fixture,
        &fixture.en_source,
        "multilocale-gap-rebuild-en-v1",
    )
    .await?;
    let fr_rebuild = rebuild_source(
        &service,
        &fixture,
        &fixture.fr_source,
        "multilocale-gap-rebuild-fr-v1",
    )
    .await?;

    let en_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: en_rebuild.operation_id,
                expected_version: fixture.publish_version,
                expected_current_artifact_id: fixture.en_source.artifact_id,
                idempotency_key: "multilocale-gap-activate-en-v1".to_string(),
            },
        )
        .await?;

    let current = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("page is missing before unexplained version fixture")
        })?;
    let mut advanced: page::ActiveModel = current.into();
    advanced.version = Set(en_activation.version + 1);
    advanced.update(&db).await?;
    let events_before_rejection = SysEvents::find().count(&db).await?;

    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: fr_rebuild.operation_id,
                expected_version: en_activation.version + 1,
                expected_current_artifact_id: fixture.fr_source.artifact_id,
                idempotency_key: "multilocale-gap-activate-fr-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
                && message.contains("source publish version is stale")
                && message.contains("not fully explained")
    ));
    assert_eq!(SysEvents::find().count(&db).await?, events_before_rejection);
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
        1
    );
    assert!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
            .filter(page_published_landing_artifact::Column::Locale.eq("fr"))
            .one(&db)
            .await?
            .is_none()
    );

    database.cleanup().await
}

async fn create_multilocale_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<MultiLocaleFixture> {
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
                        title: "Multi-locale recovery".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "fr".to_string(),
                        title: "Récupération multi-locale".to_string(),
                        slug: Some("accueil".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home-en", "Multi-locale recovery", "home")?,
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
                    document: project_json("home-fr", "Récupération multi-locale", "accueil")?,
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
                        locale: "en".to_string(),
                        revision: en_revision,
                    },
                    PageBodyRevisionInput {
                        locale: "fr".to_string(),
                        revision: fr_revision,
                    },
                ],
                idempotency_key: format!("{scenario}-publish-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let sources = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(published.operation_id))
        .order_by_asc(page_publish_rebuild_source::Column::Locale)
        .all(db)
        .await?;
    if sources.len() != 2 || sources[0].locale != "en" || sources[1].locale != "fr" {
        return Err(
            std::io::Error::other("published provenance did not retain en/fr sources").into(),
        );
    }

    Ok(MultiLocaleFixture {
        tenant_id,
        page_id: draft.id,
        publish_version: published.version,
        en_source: sources[0].clone(),
        fr_source: sources[1].clone(),
        reviewed,
    })
}

async fn rebuild_source(
    service: &PageService,
    fixture: &MultiLocaleFixture,
    source: &page_publish_rebuild_source::Model,
    idempotency_key: &str,
) -> TestResult<rustok_pages::dto::RebuildPageArtifactResult> {
    Ok(service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: source.id,
                expected_provenance_hash: source.provenance_hash.clone(),
                idempotency_key: idempotency_key.to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?)
}

async fn remove_binding_manifest_and_source_artifact(
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
                "description": "Sequential multi-locale artifact-loss recovery",
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
