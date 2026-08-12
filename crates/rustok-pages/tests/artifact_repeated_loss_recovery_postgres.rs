use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
    RollbackPageInput, SavePageDocumentInput,
};
use rustok_pages::entities::{
    page_artifact_binding_replacement_operation, page_artifact_rebuild_operation, page_body,
    page_publish_operation_artifact, page_publish_rebuild_source, page_published_landing_artifact,
    page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT, PagesError, PagesModule,
};
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages repeated artifact-loss recovery harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_repeated_loss_recovery_{}_{}",
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

struct RollbackFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    previous_publish_operation_id: Uuid,
    previous_artifact_id: Uuid,
    current_publish_version: i32,
    current_source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn missing_binding_activation_recovers_same_locale_after_rebuilt_artifact_is_lost_again_on_postgres(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup("same_locale").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_multilocale_fixture(&db, &service, "repeated-loss-same-locale").await?;

    remove_source_binding_manifest_and_artifact(&db, &fixture.en_source).await?;
    let first_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "repeated-loss-en-rebuild-v1",
    )
    .await?;
    let first_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        first_rebuild.operation_id,
        fixture.publish_version,
        fixture.en_source.artifact_id,
        "repeated-loss-en-activate-v1",
    )
    .await?;
    assert_eq!(first_activation.version, fixture.publish_version + 1);

    remove_current_rebuilt_binding_and_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        "en",
        first_rebuild.rebuilt_artifact_id,
        true,
    )
    .await?;
    assert!(
        page_artifact_rebuild_operation::Entity::find_by_id(first_rebuild.operation_id)
            .one(&db)
            .await?
            .is_some()
    );
    assert!(
        page_artifact_binding_replacement_operation::Entity::find_by_id(first_activation.operation_id)
            .one(&db)
            .await?
            .is_some()
    );

    let second_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "repeated-loss-en-rebuild-v2",
    )
    .await?;
    let second_input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: second_rebuild.operation_id,
        expected_version: first_activation.version,
        expected_current_artifact_id: fixture.en_source.artifact_id,
        idempotency_key: "repeated-loss-en-activate-v2".to_string(),
    };
    let events_before_second_activation = SysEvents::find().count(&db).await?;
    let second_activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            second_input.clone(),
        )
        .await?;
    assert_eq!(second_activation.version, fixture.publish_version + 2);
    assert_eq!(
        second_activation.replacement_artifact_id,
        second_rebuild.rebuilt_artifact_id
    );
    assert_eq!(
        current_binding(&db, fixture.tenant_id, fixture.page_id, "en")
            .await?
            .artifact_id,
        second_rebuild.rebuilt_artifact_id
    );
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_second_activation + 2
    );

    let replay = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            second_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, second_activation.operation_id);
    assert_eq!(replay.version, second_activation.version);
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_second_activation + 2
    );

    database.cleanup().await
}

#[tokio::test]
async fn repeated_locale_recovery_rejects_missing_binding_while_prior_rebuilt_artifact_still_exists_on_postgres(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup("prior_rebuilt_live").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_multilocale_fixture(&db, &service, "repeated-loss-prior-live").await?;

    remove_source_binding_manifest_and_artifact(&db, &fixture.en_source).await?;
    let first_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "prior-live-en-rebuild-v1",
    )
    .await?;
    let first_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        first_rebuild.operation_id,
        fixture.publish_version,
        fixture.en_source.artifact_id,
        "prior-live-en-activate-v1",
    )
    .await?;

    remove_current_rebuilt_binding_and_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        "en",
        first_rebuild.rebuilt_artifact_id,
        false,
    )
    .await?;
    assert!(
        page_static_landing_artifact::Entity::find_by_id(first_rebuild.rebuilt_artifact_id)
            .one(&db)
            .await?
            .is_some()
    );

    let second_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "prior-live-en-rebuild-v2",
    )
    .await?;
    let events_before_rejection = SysEvents::find().count(&db).await?;
    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: second_rebuild.operation_id,
                expected_version: first_activation.version,
                expected_current_artifact_id: fixture.en_source.artifact_id,
                idempotency_key: "prior-live-en-activate-v2".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
                && message.contains("prior rebuilt immutable artifact still exists")
    ));
    assert_eq!(SysEvents::find().count(&db).await?, events_before_rejection);
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
async fn another_locale_can_recover_after_repeated_locale_loss_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("repeat_then_other").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_multilocale_fixture(&db, &service, "repeated-loss-then-other").await?;

    remove_source_binding_manifest_and_artifact(&db, &fixture.en_source).await?;
    remove_source_binding_manifest_and_artifact(&db, &fixture.fr_source).await?;

    let en_first_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "repeat-other-en-rebuild-v1",
    )
    .await?;
    let en_first_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        en_first_rebuild.operation_id,
        fixture.publish_version,
        fixture.en_source.artifact_id,
        "repeat-other-en-activate-v1",
    )
    .await?;
    remove_current_rebuilt_binding_and_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        "en",
        en_first_rebuild.rebuilt_artifact_id,
        true,
    )
    .await?;

    let en_second_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.en_source,
        "repeat-other-en-rebuild-v2",
    )
    .await?;
    let en_second_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        en_second_rebuild.operation_id,
        en_first_activation.version,
        fixture.en_source.artifact_id,
        "repeat-other-en-activate-v2",
    )
    .await?;

    let fr_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.fr_source,
        "repeat-other-fr-rebuild-v1",
    )
    .await?;
    let fr_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        fr_rebuild.operation_id,
        en_second_activation.version,
        fixture.fr_source.artifact_id,
        "repeat-other-fr-activate-v1",
    )
    .await?;
    assert_eq!(fr_activation.version, fixture.publish_version + 3);

    let bindings = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .order_by_asc(page_published_landing_artifact::Column::Locale)
        .all(&db)
        .await?;
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].locale, "en");
    assert_eq!(bindings[0].artifact_id, en_second_rebuild.rebuilt_artifact_id);
    assert_eq!(bindings[1].locale, "fr");
    assert_eq!(bindings[1].artifact_id, fr_rebuild.rebuilt_artifact_id);

    database.cleanup().await
}

#[tokio::test]
async fn rollback_continues_after_same_locale_is_recovered_twice_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("rollback").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_rollback_fixture(&db, &service, "repeated-loss-rollback").await?;

    remove_source_binding_manifest_and_artifact(&db, &fixture.current_source).await?;
    let first_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.current_source,
        "rollback-repeat-rebuild-v1",
    )
    .await?;
    let first_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        first_rebuild.operation_id,
        fixture.current_publish_version,
        fixture.current_source.artifact_id,
        "rollback-repeat-activate-v1",
    )
    .await?;
    remove_current_rebuilt_binding_and_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        "en",
        first_rebuild.rebuilt_artifact_id,
        true,
    )
    .await?;

    let second_rebuild = rebuild_source(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.reviewed,
        &fixture.current_source,
        "rollback-repeat-rebuild-v2",
    )
    .await?;
    let second_activation = activate_rebuild(
        &service,
        fixture.tenant_id,
        fixture.page_id,
        second_rebuild.operation_id,
        first_activation.version,
        fixture.current_source.artifact_id,
        "rollback-repeat-activate-v2",
    )
    .await?;

    let rollback_input = RollbackPageInput {
        expected_version: second_activation.version,
        idempotency_key: "rollback-after-repeated-loss-v1".to_string(),
    };
    let rollback = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            rollback_input.clone(),
        )
        .await?;
    assert!(!rollback.replayed);
    assert_eq!(
        rollback.target_publish_operation_id,
        fixture.previous_publish_operation_id
    );
    assert_eq!(rollback.version, second_activation.version + 1);
    assert_eq!(
        current_binding(&db, fixture.tenant_id, fixture.page_id, "en")
            .await?
            .artifact_id,
        fixture.previous_artifact_id
    );

    let replay = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            rollback_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, rollback.operation_id);
    assert_eq!(replay.version, rollback.version);

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
                        title: "Repeated recovery".to_string(),
                        slug: Some("home".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                    PageTranslationInput {
                        locale: "fr".to_string(),
                        title: "Récupération répétée".to_string(),
                        slug: Some("accueil".to_string()),
                        meta_title: None,
                        meta_description: None,
                    },
                ],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home-en", "Repeated recovery", "home")?,
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
                    document: project_json("home-fr", "Récupération répétée", "accueil")?,
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
        return Err(std::io::Error::other("published provenance did not retain en/fr sources").into());
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

async fn create_rollback_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<RollbackFixture> {
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
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Repeated rollback A".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home", "Repeated rollback A", "home")?,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let first_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("rollback first body is missing"))?
        .updated_at
        .clone();
    let first = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: first_revision,
                }],
                idempotency_key: format!("{scenario}-publish-a-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let first_source = publish_source(db, first.operation_id, "en").await?;

    let unpublished = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(first.version),
        )
        .await?;
    let current_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("rollback body is missing before second save"))?;
    service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: current_body.updated_at.to_string(),
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("home", "Repeated rollback B", "home")?,
                },
            },
        )
        .await?;
    let second_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("rollback body is missing before second publish"))?;
    let second = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: unpublished.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: second_body.updated_at.to_string(),
                }],
                idempotency_key: format!("{scenario}-publish-b-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let second_source = publish_source(db, second.operation_id, "en").await?;

    Ok(RollbackFixture {
        tenant_id,
        page_id: draft.id,
        previous_publish_operation_id: first.operation_id,
        previous_artifact_id: first_source.artifact_id,
        current_publish_version: second.version,
        current_source: second_source,
        reviewed,
    })
}

async fn publish_source(
    db: &DatabaseConnection,
    operation_id: Uuid,
    locale: &str,
) -> TestResult<page_publish_rebuild_source::Model> {
    Ok(page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq(locale))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?)
}

async fn rebuild_source(
    service: &PageService,
    tenant_id: Uuid,
    page_id: Uuid,
    reviewed: &PageBuilderReviewedPublishRuntime,
    source: &page_publish_rebuild_source::Model,
    idempotency_key: &str,
) -> TestResult<rustok_pages::dto::RebuildPageArtifactResult> {
    Ok(service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            page_id,
            RebuildPageArtifactInput {
                source_id: source.id,
                expected_provenance_hash: source.provenance_hash.clone(),
                idempotency_key: idempotency_key.to_string(),
                runtime: reviewed_input(reviewed),
            },
        )
        .await?)
}

async fn activate_rebuild(
    service: &PageService,
    tenant_id: Uuid,
    page_id: Uuid,
    rebuild_operation_id: Uuid,
    expected_version: i32,
    source_artifact_id: Uuid,
    idempotency_key: &str,
) -> TestResult<rustok_pages::dto::ReplacePageArtifactBindingResult> {
    Ok(service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id,
                expected_version,
                expected_current_artifact_id: source_artifact_id,
                idempotency_key: idempotency_key.to_string(),
            },
        )
        .await?)
}

async fn remove_source_binding_manifest_and_artifact(
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

async fn remove_current_rebuilt_binding_and_artifact(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    artifact_id: Uuid,
    delete_artifact: bool,
) -> TestResult<()> {
    let removed_binding = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq(locale))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_binding.rows_affected, 1);
    if delete_artifact {
        let deleted = page_static_landing_artifact::Entity::delete_by_id(artifact_id)
            .exec(db)
            .await?;
        assert_eq!(deleted.rows_affected, 1);
    }
    Ok(())
}

async fn current_binding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
) -> TestResult<page_published_landing_artifact::Model> {
    Ok(page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq(locale))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("current published binding is missing"))?)
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
                "description": "Repeated immutable artifact-loss recovery",
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
