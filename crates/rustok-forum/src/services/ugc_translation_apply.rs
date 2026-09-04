use chrono::Utc;
use rustok_api::{RichTextDocument, TenantLocale};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseTransaction, EntityTrait, QueryFilter, Statement, sea_query::Expr,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    entities::{forum_reply, forum_reply_body, forum_topic, forum_topic_translation},
    error::ForumError,
    richtext::serialize_discussion,
};

use super::{ReplyService, TopicService};

/// Forum UGC deliberately uses the existing moderation-subject revision as a
/// conservative owner clock. A change to any owner-visible topic/reply state,
/// including localized content, invalidates Translation snapshots instead of
/// allowing a stale machine patch to cross an unrelated producer edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ForumUgcTranslationApplyResult {
    pub resource_revision: i64,
    pub target_revision: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ApplyExactForumTopicTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub title: String,
    pub body: RichTextDocument,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ApplyExactForumReplyTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub body: RichTextDocument,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
}

#[derive(Debug, Error)]
pub(crate) enum ForumUgcTranslationApplyError {
    #[error("Forum UGC translation revision changed after the proposal snapshot")]
    RevisionConflict,

    #[error("Forum UGC translation revision state is unavailable")]
    RevisionUnavailable,

    #[error("Forum UGC translation owner apply does not support this database backend")]
    UnsupportedDatabaseBackend,

    #[error(transparent)]
    Forum(#[from] ForumError),
}

impl From<sea_orm::DbErr> for ForumUgcTranslationApplyError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Forum(ForumError::from(error))
    }
}

pub(crate) type ForumUgcTranslationApplyResultT<T> =
    Result<T, ForumUgcTranslationApplyError>;

impl TopicService {
    pub(crate) async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        input: ApplyExactForumTopicTranslationInput,
    ) -> ForumUgcTranslationApplyResultT<ForumUgcTranslationApplyResult> {
        ensure_distinct_locales(&input.source_locale, &input.target_locale)?;
        if input.title.trim().is_empty() {
            return Err(ForumError::Validation("Topic title cannot be empty".to_string()).into());
        }
        let stored_body = serialize_discussion(input.body)?;

        let current_revision =
            lock_subject_and_revision_in_tx(txn, tenant_id, ForumUgcSubjectKind::Topic, topic_id)
                .await?;

        let source = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(
                forum_topic_translation::Column::Locale.eq(input.source_locale.as_str()),
            )
            .one(txn)
            .await?
            .ok_or_else(|| {
                ForumError::Validation("Exact source Forum topic locale is not present".to_string())
            })?;

        let target = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(
                forum_topic_translation::Column::Locale.eq(input.target_locale.as_str()),
            )
            .one(txn)
            .await?;

        validate_expected_revisions(
            current_revision,
            target.is_some(),
            input.expected_resource_revision,
            input.expected_source_revision,
            input.expected_target_revision,
        )?;

        let now = Utc::now();
        let changed = match target {
            Some(target) if target.title == input.title && target.body == stored_body => false,
            Some(target) => {
                let mut active: forum_topic_translation::ActiveModel = target.into();
                active.title = Set(input.title);
                active.body = Set(stored_body);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
                true
            }
            None => {
                let inserted = forum_topic_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    topic_id: Set(topic_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.target_locale.as_str().to_string()),
                    title: Set(input.title),
                    // UGC Translation does not own route copy. Preserve the exact
                    // source slug when materializing a new locale; a later author
                    // route edit remains Forum-owned and advances the same clock.
                    slug: Set(source.slug),
                    body: Set(stored_body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(_) => true,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(ForumUgcTranslationApplyError::RevisionConflict);
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        if changed {
            let updated = forum_topic::Entity::update_many()
                .col_expr(
                    forum_topic::Column::UpdatedAt,
                    Expr::value(now.fixed_offset()),
                )
                .filter(forum_topic::Column::TenantId.eq(tenant_id))
                .filter(forum_topic::Column::Id.eq(topic_id))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(ForumError::TopicNotFound(topic_id).into());
            }
        }

        finish_apply(
            txn,
            tenant_id,
            ForumUgcSubjectKind::Topic,
            topic_id,
            current_revision,
            changed,
        )
        .await
    }
}

impl ReplyService {
    pub(crate) async fn apply_exact_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        input: ApplyExactForumReplyTranslationInput,
    ) -> ForumUgcTranslationApplyResultT<ForumUgcTranslationApplyResult> {
        ensure_distinct_locales(&input.source_locale, &input.target_locale)?;
        let stored_body = serialize_discussion(input.body)?;

        let current_revision =
            lock_subject_and_revision_in_tx(txn, tenant_id, ForumUgcSubjectKind::Reply, reply_id)
                .await?;

        forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_body::Column::Locale.eq(input.source_locale.as_str()))
            .one(txn)
            .await?
            .ok_or_else(|| {
                ForumError::Validation("Exact source Forum reply locale is not present".to_string())
            })?;

        let target = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_body::Column::Locale.eq(input.target_locale.as_str()))
            .one(txn)
            .await?;

        validate_expected_revisions(
            current_revision,
            target.is_some(),
            input.expected_resource_revision,
            input.expected_source_revision,
            input.expected_target_revision,
        )?;

        let now = Utc::now();
        let changed = match target {
            Some(target) if target.body == stored_body => false,
            Some(target) => {
                let mut active: forum_reply_body::ActiveModel = target.into();
                active.body = Set(stored_body);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
                true
            }
            None => {
                let inserted = forum_reply_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    reply_id: Set(reply_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(input.target_locale.as_str().to_string()),
                    body: Set(stored_body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await;
                match inserted {
                    Ok(_) => true,
                    Err(error) if is_unique_constraint(&error) => {
                        return Err(ForumUgcTranslationApplyError::RevisionConflict);
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };

        if changed {
            let updated = forum_reply::Entity::update_many()
                .col_expr(
                    forum_reply::Column::UpdatedAt,
                    Expr::value(now.fixed_offset()),
                )
                .filter(forum_reply::Column::TenantId.eq(tenant_id))
                .filter(forum_reply::Column::Id.eq(reply_id))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(ForumError::ReplyNotFound(reply_id).into());
            }
        }

        finish_apply(
            txn,
            tenant_id,
            ForumUgcSubjectKind::Reply,
            reply_id,
            current_revision,
            changed,
        )
        .await
    }
}

fn ensure_distinct_locales(
    source_locale: &TenantLocale,
    target_locale: &TenantLocale,
) -> ForumUgcTranslationApplyResultT<()> {
    if source_locale == target_locale {
        return Err(ForumError::Validation(
            "Source and target locales must differ for exact Forum UGC translation".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_expected_revisions(
    current_revision: i64,
    target_exists: bool,
    expected_resource_revision: i64,
    expected_source_revision: i64,
    expected_target_revision: Option<i64>,
) -> ForumUgcTranslationApplyResultT<()> {
    if expected_resource_revision != current_revision
        || expected_source_revision != current_revision
    {
        return Err(ForumUgcTranslationApplyError::RevisionConflict);
    }

    match (target_exists, expected_target_revision) {
        (false, None) => Ok(()),
        (true, Some(target_revision)) if target_revision == current_revision => Ok(()),
        _ => Err(ForumUgcTranslationApplyError::RevisionConflict),
    }
}

async fn finish_apply(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ForumUgcSubjectKind,
    subject_id: Uuid,
    previous_revision: i64,
    changed: bool,
) -> ForumUgcTranslationApplyResultT<ForumUgcTranslationApplyResult> {
    let applied_revision = current_subject_revision_in_tx(txn, tenant_id, kind, subject_id).await?;
    if changed && applied_revision <= previous_revision {
        return Err(ForumUgcTranslationApplyError::RevisionUnavailable);
    }
    if !changed && applied_revision != previous_revision {
        return Err(ForumUgcTranslationApplyError::RevisionConflict);
    }

    Ok(ForumUgcTranslationApplyResult {
        resource_revision: applied_revision,
        target_revision: applied_revision,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForumUgcSubjectKind {
    Topic,
    Reply,
}

impl ForumUgcSubjectKind {
    const fn subject_table(self) -> &'static str {
        match self {
            Self::Topic => "forum_topics",
            Self::Reply => "forum_replies",
        }
    }

    const fn revision_table(self) -> (&'static str, &'static str) {
        match self {
            Self::Topic => ("forum_topic_moderation_subject_revisions", "topic_id"),
            Self::Reply => ("forum_reply_moderation_subject_revisions", "reply_id"),
        }
    }

    fn not_found(self, subject_id: Uuid) -> ForumError {
        match self {
            Self::Topic => ForumError::TopicNotFound(subject_id),
            Self::Reply => ForumError::ReplyNotFound(subject_id),
        }
    }
}

async fn lock_subject_and_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ForumUgcSubjectKind,
    subject_id: Uuid,
) -> ForumUgcTranslationApplyResultT<i64> {
    let backend = txn.get_database_backend();
    let subject_table = kind.subject_table();
    let (revision_table, revision_id_column) = kind.revision_table();

    match backend {
        DatabaseBackend::Postgres => {
            let subject_sql = format!(
                "SELECT id FROM {subject_table} WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE"
            );
            let subject = txn
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    subject_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await?;
            if subject.is_none() {
                return Err(kind.not_found(subject_id).into());
            }

            let revision_sql = format!(
                "SELECT revision FROM {revision_table} WHERE tenant_id = $1 AND {revision_id_column} = $2 FOR UPDATE"
            );
            if txn
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    revision_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await?
                .is_none()
            {
                return Err(ForumUgcTranslationApplyError::RevisionUnavailable);
            }
        }
        DatabaseBackend::Sqlite => {
            let reserve_sql = format!(
                "UPDATE {revision_table} SET revision = revision WHERE tenant_id = ? AND {revision_id_column} = ?"
            );
            let reserve = txn
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    reserve_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await?;
            if reserve.rows_affected() != 1 {
                return Err(ForumUgcTranslationApplyError::RevisionUnavailable);
            }

            let subject_sql = format!(
                "SELECT id FROM {subject_table} WHERE tenant_id = ? AND id = ? AND deleted_at IS NULL"
            );
            if txn
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    subject_sql,
                    vec![tenant_id.into(), subject_id.into()],
                ))
                .await?
                .is_none()
            {
                return Err(kind.not_found(subject_id).into());
            }
        }
        _ => return Err(ForumUgcTranslationApplyError::UnsupportedDatabaseBackend),
    }

    current_subject_revision_in_tx(txn, tenant_id, kind, subject_id).await
}

async fn current_subject_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: ForumUgcSubjectKind,
    subject_id: Uuid,
) -> ForumUgcTranslationApplyResultT<i64> {
    let backend = txn.get_database_backend();
    let (table, id_column) = kind.revision_table();
    let sql = match backend {
        DatabaseBackend::Postgres => {
            format!("SELECT revision FROM {table} WHERE tenant_id = $1 AND {id_column} = $2")
        }
        DatabaseBackend::Sqlite => {
            format!("SELECT revision FROM {table} WHERE tenant_id = ? AND {id_column} = ?")
        }
        _ => return Err(ForumUgcTranslationApplyError::UnsupportedDatabaseBackend),
    };

    let row = txn
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into(), subject_id.into()],
        ))
        .await?
        .ok_or(ForumUgcTranslationApplyError::RevisionUnavailable)?;
    let revision: i64 = row
        .try_get("", "revision")
        .map_err(|error| ForumUgcTranslationApplyError::Forum(ForumError::from(error)))?;
    if revision <= 0 {
        return Err(ForumUgcTranslationApplyError::RevisionUnavailable);
    }
    Ok(revision)
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_core::SecurityContext;
    use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use sea_orm::{ConnectOptions, Database, DatabaseConnection, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use crate::{
        CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ReplyService,
        TopicService, migrations,
    };

    use super::*;

    async fn setup_forum_test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:forum_ugc_translation_apply_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect Forum UGC translation test database")
    }

    async fn ensure_forum_schema(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        SysEventsMigration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
        db.execute_unprepared(
            "CREATE TABLE users (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                PRIMARY KEY (id),
                UNIQUE (tenant_id, id)
            )",
        )
        .await
        .expect("identity owner table should exist for Forum tests");

        for migration in rustok_taxonomy::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("taxonomy migration should apply");
        }
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("Forum migration should apply");
        }
    }

    fn event_bus(db: &DatabaseConnection) -> TransactionalEventBus {
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
    }

    async fn seed_topic_and_reply(
        db: &DatabaseConnection,
    ) -> (Uuid, TopicService, ReplyService, Uuid, Uuid) {
        let tenant_id = Uuid::new_v4();
        let security = SecurityContext::system();
        let bus = event_bus(db);
        let category = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "General".to_string(),
                    slug: "general".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("category should be created");
        let topic_service = TopicService::new(db.clone(), bus.clone());
        let topic = topic_service
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Source topic".to_string(),
                    slug: Some("source-topic".to_string()),
                    body: RichTextDocument::single_paragraph("Source topic body"),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("topic should be created");
        let reply_service = ReplyService::new(db.clone(), bus);
        let reply = reply_service
            .create(
                tenant_id,
                security,
                topic.id,
                CreateReplyInput {
                    locale: "en".to_string(),
                    content: RichTextDocument::single_paragraph("Source reply body"),
                    parent_reply_id: None,
                },
            )
            .await
            .expect("reply should be created");
        (tenant_id, topic_service, reply_service, topic.id, reply.id)
    }

    #[tokio::test]
    async fn topic_translation_apply_uses_single_owner_revision_fence() {
        let db = setup_forum_test_db().await;
        ensure_forum_schema(&db).await;
        let (tenant_id, topic_service, _, topic_id, _) = seed_topic_and_reply(&db).await;

        let txn = db.begin().await.expect("translation transaction should begin");
        let revision = current_subject_revision_in_tx(
            &txn,
            tenant_id,
            ForumUgcSubjectKind::Topic,
            topic_id,
        )
        .await
        .expect("topic revision should exist");
        let applied = topic_service
            .apply_exact_translation_in_tx(
                &txn,
                tenant_id,
                topic_id,
                ApplyExactForumTopicTranslationInput {
                    source_locale: TenantLocale::new("en").expect("source locale"),
                    target_locale: TenantLocale::new("nl").expect("target locale"),
                    title: "Vertaald onderwerp".to_string(),
                    body: RichTextDocument::single_paragraph("Vertaald onderwerp lichaam"),
                    expected_resource_revision: revision,
                    expected_source_revision: revision,
                    expected_target_revision: None,
                },
            )
            .await
            .expect("exact topic translation should apply");
        assert!(applied.resource_revision > revision);
        assert_eq!(applied.target_revision, applied.resource_revision);
        txn.commit().await.expect("translation transaction should commit");

        let stored = forum_topic_translation::Entity::find()
            .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
            .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
            .filter(forum_topic_translation::Column::Locale.eq("nl"))
            .one(&db)
            .await
            .expect("target topic locale query should succeed")
            .expect("target topic locale should exist");
        assert_eq!(stored.title, "Vertaald onderwerp");
        assert_eq!(stored.slug.as_deref(), Some("source-topic"));

        let stale = db.begin().await.expect("stale transaction should begin");
        let error = topic_service
            .apply_exact_translation_in_tx(
                &stale,
                tenant_id,
                topic_id,
                ApplyExactForumTopicTranslationInput {
                    source_locale: TenantLocale::new("en").expect("source locale"),
                    target_locale: TenantLocale::new("nl").expect("target locale"),
                    title: "Stale translation".to_string(),
                    body: RichTextDocument::single_paragraph("Stale body"),
                    expected_resource_revision: revision,
                    expected_source_revision: revision,
                    expected_target_revision: None,
                },
            )
            .await
            .expect_err("stale topic proposal must be rejected");
        assert!(matches!(error, ForumUgcTranslationApplyError::RevisionConflict));
        stale.rollback().await.expect("stale transaction should roll back");
    }

    #[tokio::test]
    async fn reply_translation_apply_requires_exact_target_presence_revision() {
        let db = setup_forum_test_db().await;
        ensure_forum_schema(&db).await;
        let (tenant_id, _, reply_service, _, reply_id) = seed_topic_and_reply(&db).await;

        let txn = db.begin().await.expect("translation transaction should begin");
        let revision = current_subject_revision_in_tx(
            &txn,
            tenant_id,
            ForumUgcSubjectKind::Reply,
            reply_id,
        )
        .await
        .expect("reply revision should exist");
        let applied = reply_service
            .apply_exact_translation_in_tx(
                &txn,
                tenant_id,
                reply_id,
                ApplyExactForumReplyTranslationInput {
                    source_locale: TenantLocale::new("en").expect("source locale"),
                    target_locale: TenantLocale::new("nl").expect("target locale"),
                    body: RichTextDocument::single_paragraph("Vertaald antwoord"),
                    expected_resource_revision: revision,
                    expected_source_revision: revision,
                    expected_target_revision: None,
                },
            )
            .await
            .expect("exact reply translation should apply");
        txn.commit().await.expect("translation transaction should commit");

        let conflict_txn = db.begin().await.expect("conflict transaction should begin");
        let error = reply_service
            .apply_exact_translation_in_tx(
                &conflict_txn,
                tenant_id,
                reply_id,
                ApplyExactForumReplyTranslationInput {
                    source_locale: TenantLocale::new("en").expect("source locale"),
                    target_locale: TenantLocale::new("nl").expect("target locale"),
                    body: RichTextDocument::single_paragraph("Second translation"),
                    expected_resource_revision: applied.resource_revision,
                    expected_source_revision: applied.resource_revision,
                    // The target now exists. `None` must not silently overwrite it.
                    expected_target_revision: None,
                },
            )
            .await
            .expect_err("target presence mismatch must be rejected");
        assert!(matches!(error, ForumUgcTranslationApplyError::RevisionConflict));
        conflict_txn
            .rollback()
            .await
            .expect("conflict transaction should roll back");
    }
}
