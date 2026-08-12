use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_artifact_rebuild_operation, page_body,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT, PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
    PAGES_CACHE_ENTITY_KIND, PagesError, PagesModule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement, TransactionTrait,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages explicit repair PostgreSQL harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_pages_explicit_repair_{}", Uuid::new_v4().simple());
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
async fn explicit_repair_receipts_and_activation_are_atomic_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    enable_pages_module(&db, tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus.clone());
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-repair-postgres",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let reviewed_input = || ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    };
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Explicit repair PostgreSQL",
                "description": "PostgreSQL rebuild and activation receipt regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Explicit repair PostgreSQL"
                }]
            }
        }]
    });

    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Explicit repair PostgreSQL".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project.clone(),
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let body_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("draft body is missing"))?
        .updated_at
        .clone();
    service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_revision,
                }],
                idempotency_key: "explicit-repair-postgres-publish-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;

    let source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::TenantId.eq(tenant_id))
        .filter(page_publish_rebuild_source::Column::PageId.eq(draft.id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?;
    let original_artifact = page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published artifact is missing"))?;
    let binding_before = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published binding is missing"))?;
    let page_before = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing"))?;
    let artifact_count_before = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .count(&db)
        .await?;

    let body = page_body::Entity::find_by_id(source.page_body_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("source body is missing"))?;
    let mut body_active: page_body::ActiveModel = body.into();
    body_active.content = Set(serde_json::to_string(&json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "changed",
                "type": "wrapper",
                "content": "Mutable draft must not become rebuild authority"
            }
        }]
    }))?);
    body_active.updated_at = Set((Utc::now() + ChronoDuration::seconds(1)).into());
    body_active.update(&db).await?;

    let mut corrupted: page_static_landing_artifact::ActiveModel = original_artifact.clone().into();
    corrupted.document_html = Set("<main>corrupted retained artifact</main>".to_string());
    corrupted.update(&db).await?;

    let rebuild_input = RebuildPageArtifactInput {
        source_id: source.id,
        expected_provenance_hash: source.provenance_hash.clone(),
        idempotency_key: "explicit-repair-postgres-rebuild-v1".to_string(),
        runtime: reviewed_input(),
    };
    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            rebuild_input.clone(),
        )
        .await?;
    assert!(!rebuilt.replayed);
    assert_eq!(rebuilt.source_artifact_id, original_artifact.id);
    assert_ne!(rebuilt.rebuilt_artifact_id, original_artifact.id);
    assert_eq!(rebuilt.artifact_hash, source.artifact_hash);
    assert_eq!(rebuilt.materialization_hash, source.materialization_hash);
    assert_eq!(
        rebuilt.artifact_instance_key,
        format!("rebuild:{}", rebuilt.operation_id)
    );
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before + 1
    );
    let binding_after_rebuild =
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after rebuild"))?;
    assert_eq!(binding_after_rebuild.artifact_id, original_artifact.id);
    assert_eq!(
        page::Entity::find_by_id(draft.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("page disappeared after rebuild"))?
            .version,
        page_before.version
    );

    let rebuild_replay = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            rebuild_input.clone(),
        )
        .await?;
    assert!(rebuild_replay.replayed);
    assert_eq!(rebuild_replay.operation_id, rebuilt.operation_id);
    assert_eq!(
        rebuild_replay.rebuilt_artifact_id,
        rebuilt.rebuilt_artifact_id
    );

    let rebuild_conflict = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            RebuildPageArtifactInput {
                expected_provenance_hash: "1".repeat(64),
                ..rebuild_input
            },
        )
        .await;
    assert!(matches!(
        rebuild_conflict,
        Err(PagesError::PublishIdempotencyConflict(message))
            if message.contains(PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT)
    ));
    assert_eq!(
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        1
    );
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before + 1
    );

    assert_rebuild_receipt_constraint_rolls_back_page_marker(&db, tenant_id, draft.id).await?;

    let stale = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuilt.operation_id,
                expected_version: page_before.version,
                expected_current_artifact_id: Uuid::new_v4(),
                idempotency_key: "explicit-repair-postgres-stale-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        stale,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
    ));
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id),)
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        0
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after stale activation"))?
            .artifact_id,
        original_artifact.id
    );

    let events_before_activation = outbox_event_ids(&db).await?;
    let replace_input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: rebuilt.operation_id,
        expected_version: page_before.version,
        expected_current_artifact_id: original_artifact.id,
        idempotency_key: "explicit-repair-postgres-activate-v1".to_string(),
    };
    let replaced = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            replace_input.clone(),
        )
        .await?;
    assert!(!replaced.replayed);
    assert_eq!(replaced.rebuild_operation_id, rebuilt.operation_id);
    assert_eq!(replaced.previous_artifact_id, original_artifact.id);
    assert_eq!(
        replaced.replacement_artifact_id,
        rebuilt.rebuilt_artifact_id
    );
    assert_eq!(replaced.version, page_before.version + 1);

    let binding_after =
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after activation"))?;
    assert_eq!(binding_after.artifact_id, rebuilt.rebuilt_artifact_id);
    let page_after = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after activation"))?;
    assert_eq!(page_after.version, page_before.version + 1);
    assert_eq!(page_after.status, "published");
    assert!(
        page_static_landing_artifact::Entity::find_by_id(original_artifact.id)
            .one(&db)
            .await?
            .is_some()
    );
    assert!(
        page_static_landing_artifact::Entity::find_by_id(rebuilt.rebuilt_artifact_id)
            .one(&db)
            .await?
            .is_some()
    );
    assert_activation_lifecycle_pair(&db, &events_before_activation, draft.id).await?;

    let activation_replay = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            replace_input,
        )
        .await?;
    assert!(activation_replay.replayed);
    assert_eq!(activation_replay.operation_id, replaced.operation_id);
    assert_eq!(activation_replay.version, replaced.version);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id),)
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        1
    );
    assert_eq!(
        page::Entity::find_by_id(draft.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("page disappeared after activation replay"))?
            .version,
        replaced.version
    );

    let reused_rebuild = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuilt.operation_id,
                expected_version: replaced.version,
                expected_current_artifact_id: original_artifact.id,
                idempotency_key: "explicit-repair-postgres-reuse-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        reused_rebuild,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
    ));
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id),)
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        1
    );

    assert_activation_receipt_conflict_rolls_back_page_and_outbox(
        &db,
        &event_bus,
        tenant_id,
        draft.id,
        replaced.version,
    )
    .await?;

    database.cleanup().await
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
         VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "pages".into(),
            true.into(),
            json!({}).into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn assert_rebuild_receipt_constraint_rolls_back_page_marker(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<()> {
    let page_before = page::Entity::find_by_id(page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before rebuild constraint test"))?;
    let receipt = page_artifact_rebuild_operation::Entity::find()
        .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
        .filter(page_artifact_rebuild_operation::Column::PageId.eq(page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("rebuild receipt is missing"))?;

    let txn = db.begin().await?;
    let mut marker: page::ActiveModel = page_before.clone().into();
    marker.version = Set(page_before.version + 100);
    marker.updated_at = Set(Utc::now().into());
    marker.update(&txn).await?;

    let duplicate_id = Uuid::new_v4();
    let mut duplicate: page_artifact_rebuild_operation::ActiveModel = receipt.into();
    duplicate.id = Set(duplicate_id);
    duplicate.rebuilt_artifact_id = Set(Uuid::new_v4());
    duplicate.artifact_instance_key = Set(format!("rebuild:{duplicate_id}"));
    duplicate.created_at = Set(Utc::now().into());
    let duplicate_insert = duplicate.insert(&txn).await;
    assert!(duplicate_insert.is_err());
    txn.rollback().await?;

    assert_eq!(
        page::Entity::find_by_id(page_id)
            .one(db)
            .await?
            .ok_or_else(|| std::io::Error::other("page disappeared after rebuild rollback"))?
            .version,
        page_before.version
    );
    assert_eq!(
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        1
    );
    Ok(())
}

async fn assert_activation_lifecycle_pair(
    db: &DatabaseConnection,
    before: &HashSet<Uuid>,
    page_id: Uuid,
) -> TestResult<()> {
    let new_events = SysEvents::find()
        .all(db)
        .await?
        .into_iter()
        .filter(|event| !before.contains(&event.id))
        .collect::<Vec<_>>();
    assert_eq!(new_events.len(), 2);

    let mut updated = 0;
    let mut published = 0;
    for stored in new_events {
        let envelope: EventEnvelope = serde_json::from_value(stored.payload)?;
        match envelope.event {
            DomainEvent::NodeUpdated { node_id, kind } => {
                assert_eq!(node_id, page_id);
                assert_eq!(kind, PAGES_CACHE_ENTITY_KIND);
                updated += 1;
            }
            DomainEvent::NodePublished { node_id, kind } => {
                assert_eq!(node_id, page_id);
                assert_eq!(kind, PAGES_CACHE_ENTITY_KIND);
                published += 1;
            }
            other => panic!("unexpected activation lifecycle event: {other:?}"),
        }
    }
    assert_eq!(updated, 1);
    assert_eq!(published, 1);
    Ok(())
}

async fn assert_activation_receipt_conflict_rolls_back_page_and_outbox(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    page_id: Uuid,
    expected_version: i32,
) -> TestResult<()> {
    let page_before = page::Entity::find_by_id(page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before activation rollback test"))?;
    assert_eq!(page_before.version, expected_version);
    let receipt = page_artifact_binding_replacement_operation::Entity::find()
        .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
        .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("activation receipt is missing"))?;

    let txn = db.begin().await?;
    let mut marker: page::ActiveModel = page_before.clone().into();
    marker.version = Set(page_before.version + 100);
    marker.updated_at = Set(Utc::now().into());
    marker.update(&txn).await?;
    let rolled_back_event_id = event_bus
        .publish_in_tx_with_envelope_id(
            &txn,
            tenant_id,
            None,
            DomainEvent::NodePublished {
                node_id: page_id,
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
        .await?;

    let mut duplicate: page_artifact_binding_replacement_operation::ActiveModel = receipt.into();
    duplicate.id = Set(Uuid::new_v4());
    duplicate.idempotency_key = Set("explicit-repair-postgres-duplicate-activation".to_string());
    duplicate.request_hash = Set("f".repeat(64));
    duplicate.replaced_at = Set(Utc::now().into());
    duplicate.created_at = Set(Utc::now().into());
    let duplicate_insert = duplicate.insert(&txn).await;
    assert!(duplicate_insert.is_err());
    txn.rollback().await?;

    assert_eq!(
        page::Entity::find_by_id(page_id)
            .one(db)
            .await?
            .ok_or_else(|| std::io::Error::other("page disappeared after activation rollback"))?
            .version,
        page_before.version
    );
    assert!(
        SysEvents::find_by_id(rolled_back_event_id)
            .one(db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        1
    );
    Ok(())
}

async fn outbox_event_ids(db: &DatabaseConnection) -> TestResult<HashSet<Uuid>> {
    Ok(SysEvents::find()
        .all(db)
        .await?
        .into_iter()
        .map(|event| event.id)
        .collect())
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
