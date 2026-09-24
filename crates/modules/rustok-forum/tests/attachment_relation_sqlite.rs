use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_core::MigrationSource;
use rustok_forum::{
    ForumAttachmentRelationAdmissionRequest, ForumAttachmentRelationInput,
    ForumAttachmentRelationRevision, ForumAttachmentRelationService, ForumAttachmentUsage,
    ForumContentTarget, ForumModule,
};
use rustok_media::{
    entities::{asset, asset_reference, blob},
    lifecycle::{AssetState, BlobState},
    migrations as media_migrations,
    MediaError, MediaService,
};
use rustok_outbox::SysEventsMigration;
use rustok_storage::StorageRuntime;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, EntityTrait, PaginatorTrait,
    QueryFilter,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn sqlite_attachment_relation_migration_enforces_owner_invariants() -> TestResult<()> {
    let db = Database::connect("sqlite::memory:").await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    create_minimal_fixture(&db).await?;

    let migration = attachment_migration()?;
    let manager = SchemaManager::new(&db);
    migration.up(&manager).await?;

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let topic_a = Uuid::new_v4();
    let topic_b = Uuid::new_v4();
    let missing_target = Uuid::new_v4();
    let media_a = Uuid::new_v4();
    let media_b = Uuid::new_v4();
    let reference_a = Uuid::new_v4();
    let reference_b = Uuid::new_v4();

    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{tenant_a}'), ('{tenant_b}');"
    ))
    .await?;
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics (id, tenant_id, updated_at) VALUES ('{topic_a}', '{tenant_a}', CURRENT_TIMESTAMP), ('{topic_b}', '{tenant_b}', CURRENT_TIMESTAMP);"
    ))
    .await?;

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_attachment_relation_heads
            (tenant_id, target_kind, target_id, locale, relation_revision, source_revision)
        VALUES
            ('{tenant_a}', 'topic', '{topic_a}', 'en', 1, 7)
        "#
    ))
    .await?;

    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_attachment_relations
            (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)
        VALUES
            ('{reference_a}', '{tenant_a}', 'topic', '{topic_a}', 'en', 0, '{media_a}', 'inline')
        "#
    ))
    .await?;

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relations
                (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)
            VALUES
                ('{reference_b}', '{tenant_a}', 'topic', '{topic_a}', 'en', 0, '{media_b}', 'attachment')
            "#
        ))
        .await
        .is_err(),
        "duplicate target position must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relation_heads
                (tenant_id, target_kind, target_id, locale, relation_revision, source_revision)
            VALUES
                ('{tenant_a}', 'topic', '{topic_b}', 'ru', 1, 7)
            "#
        ))
        .await
        .is_err(),
        "cross-tenant attachment target must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relation_heads
                (tenant_id, target_kind, target_id, locale, relation_revision, source_revision)
            VALUES
                ('{tenant_a}', 'topic', '{missing_target}', 'ru', 1, 7)
            "#
        ))
        .await
        .is_err(),
        "missing attachment target must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relation_heads
                (tenant_id, target_kind, target_id, locale, relation_revision, source_revision)
            VALUES
                ('{tenant_a}', 'comment', '{topic_a}', 'en', 1, 7)
            "#
        ))
        .await
        .is_err(),
        "invalid attachment target kind must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            UPDATE forum_attachment_relation_heads
            SET target_id = '{topic_b}'
            WHERE tenant_id = '{tenant_a}'
              AND target_kind = 'topic'
              AND target_id = '{topic_a}'
              AND locale = 'en'
            "#
        ))
        .await
        .is_err(),
        "attachment head identity must be immutable"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            UPDATE forum_attachment_relation_heads
            SET relation_revision = 1
            WHERE tenant_id = '{tenant_a}'
              AND target_kind = 'topic'
              AND target_id = '{topic_a}'
              AND locale = 'en'
            "#
        ))
        .await
        .is_err(),
        "attachment relation revision must increase monotonically"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            UPDATE forum_attachment_relations
            SET caption = 'tampered'
            WHERE reference_id = '{reference_a}'
            "#
        ))
        .await
        .is_err(),
        "attachment relation rows must be immutable"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relations
                (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)
            VALUES
                ('{reference_b}', '{tenant_a}', 'topic', '{topic_a}', 'en', 32, '{media_b}', 'inline')
            "#
        ))
        .await
        .is_err(),
        "attachment position 32 must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relations
                (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)
            VALUES
                ('{reference_b}', '{tenant_a}', 'topic', '{topic_a}', 'en', 1, '{media_b}', 'invalid')
            "#
        ))
        .await
        .is_err(),
        "invalid attachment usage must be rejected"
    );

    assert!(
        db.execute_unprepared(&format!(
            r#"
            INSERT INTO forum_attachment_relations
                (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)
            VALUES
                ('{reference_b}', '{tenant_a}', 'topic', '{topic_a}', 'en', 0, '{media_b}', 'attachment')
            "#
        ))
        .await
        .is_err(),
        "relation rows must preserve unique positions"
    );

    db.execute_unprepared(&format!(
        "INSERT INTO forum_attachment_relations          (reference_id, tenant_id, target_kind, target_id, locale, position, media_id, usage)          VALUES ('{reference_b}', '{tenant_a}', 'topic', '{topic_a}', 'en', 1, '{media_b}', 'attachment')"
    ))
    .await?;

    db.execute_unprepared(&format!("DELETE FROM tenants WHERE id = '{tenant_a}'"))
        .await?;

    let head_count = rustok_forum::entities::forum_attachment_relation_head::Entity::find()
        .count(&db)
        .await?;
    assert_eq!(head_count, 0, "tenant deletion must cascade attachment heads");

    let relation_count = rustok_forum::entities::forum_attachment_relation::Entity::find()
        .count(&db)
        .await?;
    assert_eq!(
        relation_count, 0,
        "tenant deletion must cascade attachment relations"
    );

    migration.down(&manager).await?;
    Ok(())
}

#[tokio::test]
async fn sqlite_attachment_relation_service_coordinates_media_retention_and_cas() -> TestResult<()> {
    let db = Database::connect("sqlite::memory:").await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    create_minimal_fixture(&db).await?;

    let outbox_manager = SchemaManager::new(&db);
    SysEventsMigration.up(&outbox_manager).await?;
    for migration in media_migrations::migrations() {
        migration.up(&outbox_manager).await?;
    }

    let attachment_migration = attachment_migration()?;
    attachment_migration.up(&outbox_manager).await?;

    let tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let media_id = Uuid::new_v4();
    let blob_id = Uuid::new_v4();

    db.execute_unprepared(&format!(
        "INSERT INTO tenants (id) VALUES ('{tenant_id}')"
    ))
    .await?;
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics (id, tenant_id, updated_at) VALUES ('{topic_id}', '{tenant_id}', CURRENT_TIMESTAMP)"
    ))
    .await?;
    seed_ready_media(&db, tenant_id, media_id, blob_id).await?;

    let storage = StorageRuntime::in_memory();
    let media = Arc::new(MediaService::new(db.clone(), storage));

    let forum = ForumAttachmentRelationService::new(db.clone(), media.clone());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "forum-attachment-test",
    )
    .with_deadline(Duration::from_secs(60));

    let first = forum
        .replace_attachment_relations(
            context.clone().with_idempotency_key("forum-attachment-create"),
            ForumAttachmentRelationAdmissionRequest {
                tenant_id,
                target: ForumContentTarget::topic(topic_id),
                source_revision: 1,
                locale: "en-US".to_string(),
                expected_relation_revision: ForumAttachmentRelationRevision::EMPTY,
                attachments: vec![ForumAttachmentRelationInput {
                    media_id,
                    usage: ForumAttachmentUsage::Inline,
                    position: 0,
                    caption: Some("  first caption  ".to_string()),
                }],
            },
        )
        .await?;

    assert_eq!(
        first.relation_revision,
        ForumAttachmentRelationRevision::FIRST
    );
    assert_eq!(first.locale, "en-US");
    assert_eq!(first.attachments.len(), 1);
    assert_eq!(first.attachments[0].media_id, media_id);
    assert_eq!(
        first.attachments[0].caption.as_deref(),
        Some("first caption")
    );
    assert_eq!(
        asset_reference::Entity::find()
            .filter(asset_reference::Column::TenantId.eq(tenant_id))
            .filter(asset_reference::Column::MediaId.eq(media_id))
            .count(&db)
            .await?,
        1
    );

    let exact_retry = forum
        .replace_attachment_relations(
            context
                .clone()
                .with_idempotency_key("forum-attachment-exact-retry"),
            ForumAttachmentRelationAdmissionRequest {
                tenant_id,
                target: ForumContentTarget::topic(topic_id),
                source_revision: 1,
                locale: "en-US".to_string(),
                expected_relation_revision: ForumAttachmentRelationRevision::EMPTY,
                attachments: first
                    .attachments
                    .iter()
                    .map(|relation| ForumAttachmentRelationInput {
                        media_id: relation.media_id,
                        usage: relation.usage,
                        position: relation.position,
                        caption: relation.caption.clone(),
                    })
                    .collect(),
            },
        )
        .await?;

    assert_eq!(
        exact_retry.relation_revision,
        ForumAttachmentRelationRevision::FIRST,
        "exact stale retries must not advance the relation revision"
    );
    assert_eq!(exact_retry.attachments, first.attachments);

    let blocked = media.delete(tenant_id, media_id).await;
    assert!(
        matches!(blocked, Err(MediaError::AssetReferenced(id)) if id == media_id),
        "Media deletion must be fenced by the committed Forum relation"
    );

    let current = forum
        .get_attachment_relations(tenant_id, ForumContentTarget::topic(topic_id), "en-US")
        .await?;
    assert_eq!(current.relation_revision, first.relation_revision);
    assert_eq!(current.attachments, first.attachments);

    let cleared = forum
        .replace_attachment_relations(
            context.with_idempotency_key("forum-attachment-clear"),
            ForumAttachmentRelationAdmissionRequest {
                tenant_id,
                target: ForumContentTarget::topic(topic_id),
                source_revision: 2,
                locale: "en-US".to_string(),
                expected_relation_revision: ForumAttachmentRelationRevision::FIRST,
                attachments: Vec::new(),
            },
        )
        .await?;

    assert_eq!(cleared.relation_revision.value(), 2);
    assert!(cleared.attachments.is_empty());
    assert_eq!(
        asset_reference::Entity::find()
            .filter(asset_reference::Column::TenantId.eq(tenant_id))
            .filter(asset_reference::Column::MediaId.eq(media_id))
            .count(&db)
            .await?,
        0,
        "clear must release removed Media holds after Forum commit"
    );

    Ok(())
}

async fn create_minimal_fixture(db: &sea_orm::DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL
        );

        CREATE TABLE users (
            id TEXT PRIMARY KEY NOT NULL
        );

        CREATE TABLE forum_topics (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE forum_replies (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .await?;
    Ok(())
}

fn attachment_migration() -> TestResult<Box<dyn MigrationTrait>> {
    ForumModule
        .migrations()
        .into_iter()
        .last()
        .ok_or_else(|| "Forum must expose the attachment relation migration".into())
}

async fn seed_ready_media(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    media_id: Uuid,
    blob_id: Uuid,
) -> TestResult<()> {
    let now = chrono::Utc::now().fixed_offset();

    let asset = asset::ActiveModel {
        id: sea_orm::ActiveValue::Set(media_id),
        tenant_id: sea_orm::ActiveValue::Set(tenant_id),
        owner_module: sea_orm::ActiveValue::Set("test".to_string()),
        uploaded_by: sea_orm::ActiveValue::Set(None),
        upload_session_id: sea_orm::ActiveValue::Set(None),
        active_blob_id: sea_orm::ActiveValue::Set(Some(blob_id)),
        original_name: sea_orm::ActiveValue::Set("fixture.png".to_string()),
        lifecycle_state: sea_orm::ActiveValue::Set(AssetState::Active.as_str().to_string()),
        metadata: sea_orm::ActiveValue::Set(serde_json::json!({})),
        created_at: sea_orm::ActiveValue::Set(now),
        updated_at: sea_orm::ActiveValue::Set(now),
        delete_requested_at: sea_orm::ActiveValue::Set(None),
        deleted_at: sea_orm::ActiveValue::Set(None),
    };
    asset.insert(db).await?;

    let blob = blob::ActiveModel {
        id: sea_orm::ActiveValue::Set(blob_id),
        tenant_id: sea_orm::ActiveValue::Set(tenant_id),
        asset_id: sea_orm::ActiveValue::Set(media_id),
        object_key: sea_orm::ActiveValue::Set(format!("media/test/{media_id}")),
        mime_type: sea_orm::ActiveValue::Set("image/png".to_string()),
        size: sea_orm::ActiveValue::Set(1),
        checksum_sha256: sea_orm::ActiveValue::Set("0".repeat(64)),
        width: sea_orm::ActiveValue::Set(Some(1)),
        height: sea_orm::ActiveValue::Set(Some(1)),
        state: sea_orm::ActiveValue::Set(BlobState::Ready.as_str().to_string()),
        created_at: sea_orm::ActiveValue::Set(now),
        ready_at: sea_orm::ActiveValue::Set(Some(now)),
        delete_requested_at: sea_orm::ActiveValue::Set(None),
        deleted_at: sea_orm::ActiveValue::Set(None),
        reconcile_attempts: sea_orm::ActiveValue::Set(0),
        last_reconciled_at: sea_orm::ActiveValue::Set(now),
        last_error: sea_orm::ActiveValue::Set(None),
    };
    blob.insert(db).await?;

    Ok(())
}
