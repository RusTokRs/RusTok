use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use chrono::Utc;
use flex::delete_attached_localized_values;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QueryResult, QuerySelect, Statement, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::audience::{ForumAudienceEvaluator, ForumAudienceFacts};
use crate::dto::{
    CreateTopicInput, MAX_FORUM_CATEGORY_TREE_DEPTH, MAX_FORUM_CATEGORY_TREE_NODES, TopicResponse,
    UpdateTopicInput,
};
use crate::entities::{
    forum_category, forum_category_policy, forum_reply, forum_solution, forum_topic,
    forum_topic_channel_access,
};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::{ReplyStatus, TopicStatus};
use crate::visibility::ForumCategoryVisibility;

use super::category::CategoryService;
use super::projection_invalidation::{
    publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
};
use super::rbac::enforce_owned_scope;
use super::topic;
use super::topic_audience::load_policy_for_topic;
use super::topic_route::{
    ForumTopicRouteService, ForumTopicSlugRenameResult, RenameForumTopicSlugInput,
};
use super::user_stats::UserStatsService;

const FORUM_TOPIC_DELETED_ROUTE_REASON: &str = "Topic deleted";

/// Public owner service for topic commands.
///
/// Explicit root-service lifecycle writes happen here. The wrapped persistence
/// service remains a compatibility path, while database triggers provide the
/// final consistency barrier for direct SQL and older deployments.
pub struct TopicService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    inner: topic::TopicService,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: topic::TopicService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .create_with_relations(tenant_id, security, input)
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .update_with_inline_relations(tenant_id, topic_id, security, input.into())
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn rename_slug(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        let existing = self.inner.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Update,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        let result = ForumTopicRouteService::rename_topic_slug_in_tx(
            &txn,
            tenant_id,
            topic_id,
            &input,
        )
        .await?;
        if result.changed {
            let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
            let mut active: forum_topic::ActiveModel = topic.into();
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic_id,
            )
            .await?;
        }
        txn.commit().await?;
        Ok(result)
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let existing = self.inner.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Delete,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        claim_topic_delete_in_tx(&txn, tenant_id, topic_id).await?;
        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;

        let public_reply_count = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .filter(forum_reply::Column::Status.eq(ReplyStatus::Approved))
            .count(&txn)
            .await?;
        let public_reply_count = i32::try_from(public_reply_count).map_err(|_| {
            ForumError::Validation("Forum reply count exceeds supported range".to_string())
        })?;

        let solution_author_id = if let Some(solution) = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&txn)
            .await?
        {
            forum_reply::Entity::find_by_id(solution.reply_id)
                .filter(forum_reply::Column::TenantId.eq(tenant_id))
                .filter(forum_reply::Column::TopicId.eq(topic_id))
                .one(&txn)
                .await?
                .and_then(|reply| reply.author_id)
        } else {
            None
        };

        record_topic_route_tombstone_visibility_snapshot_in_tx(
            &txn,
            tenant_id,
            &topic,
        )
        .await?;
        ForumTopicRouteService::record_delete_tombstones_in_tx(
            &txn,
            tenant_id,
            topic_id,
            FORUM_TOPIC_DELETED_ROUTE_REASON,
        )
        .await?;

        delete_attached_localized_values(&txn, tenant_id, "topic", topic_id)
            .await
            .map_err(map_flex_cleanup_error)?;
        forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .exec(&txn)
            .await?;

        UserStatsService::decrement_topic_thread_aggregated_in_tx(
            &txn,
            tenant_id,
            topic_id,
            topic.author_id,
            solution_author_id,
        )
        .await?;
        mark_topic_thread_deleted_in_tx(&txn, tenant_id, topic_id).await?;

        CategoryService::adjust_counters_in_tx(
            &txn,
            tenant_id,
            topic.category_id,
            -1,
            -public_reply_count,
        )
        .await?;

        if topic.status != TopicStatus::Archived {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    tenant_id,
                    security.user_id,
                    DomainEvent::ForumTopicStatusChanged {
                        topic_id,
                        old_status: topic.status.to_string(),
                        new_status: TopicStatus::Archived.to_string(),
                        moderator_id: security.user_id,
                    },
                )
                .await?;
        }
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        publish_forum_category_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic.category_id,
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    pub(crate) async fn find_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        self.inner.find_topic(tenant_id, topic_id).await
    }

    pub(crate) async fn find_topic_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::find_topic_in_tx(txn, tenant_id, topic_id).await
    }

    pub(crate) async fn adjust_reply_count_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        delta: i32,
    ) -> ForumResult<crate::entities::forum_topic::Model> {
        topic::TopicService::adjust_reply_count_in_tx(txn, tenant_id, topic_id, delta).await
    }

    pub(crate) async fn set_pinned_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_pinned: bool,
    ) -> ForumResult<()> {
        topic::TopicService::set_pinned_in_tx(txn, tenant_id, topic_id, is_pinned).await
    }

    pub(crate) async fn set_locked_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        is_locked: bool,
    ) -> ForumResult<()> {
        topic::TopicService::set_locked_in_tx(txn, tenant_id, topic_id, is_locked).await
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        status: TopicStatus,
    ) -> ForumResult<()> {
        topic::TopicService::set_status_in_tx(txn, tenant_id, topic_id, status).await
    }
}

impl Deref for TopicService {
    type Target = topic::TopicService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopicRouteTombstoneVisibilitySnapshot {
    publicly_disclosable: bool,
    route_channel_restricted: bool,
}

async fn record_topic_route_tombstone_visibility_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic: &forum_topic::Model,
) -> ForumResult<()> {
    let category_public = category_is_public_in_tx(txn, tenant_id, topic.category_id).await?;
    let audience_public = topic_audience_allows_public_in_tx(txn, tenant_id, topic).await?;
    let source_channel_count = forum_topic_channel_access::Entity::find()
        .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_channel_access::Column::TopicId.eq(topic.id))
        .count(txn)
        .await?;
    let snapshot = TopicRouteTombstoneVisibilitySnapshot {
        publicly_disclosable: topic.status == TopicStatus::Open
            && category_public
            && audience_public,
        route_channel_restricted: source_channel_count > 0,
    };

    insert_tombstone_visibility_snapshot_in_tx(txn, tenant_id, topic.id, snapshot).await?;
    insert_tombstone_channels_in_tx(txn, tenant_id, topic.id).await?;

    let stored = load_tombstone_visibility_snapshot(txn, tenant_id, topic.id)
        .await?
        .ok_or(ForumError::TopicRouteResolutionConflict)?;
    if stored != snapshot {
        return Err(ForumError::TopicRouteResolutionConflict);
    }

    let snapshot_channel_count = count_tombstone_channels(txn, tenant_id, topic.id).await?;
    let matching_channel_count = count_matching_tombstone_channels(txn, tenant_id, topic.id).await?;
    if snapshot_channel_count != source_channel_count || matching_channel_count != source_channel_count
    {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    Ok(())
}

async fn topic_audience_allows_public_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic: &forum_topic::Model,
) -> ForumResult<bool> {
    let policy = load_policy_for_topic(txn, tenant_id, topic).await?;
    let security = SecurityContext::public_read();
    let facts = ForumAudienceFacts::default();
    for layer in &policy.inherited_category_layers {
        if !ForumAudienceEvaluator::decide(
            tenant_id,
            &layer.constraints,
            &security,
            &facts,
        )?
        .allowed
        {
            return Ok(false);
        }
    }
    if let Some(constraints) = &policy.configured_constraints
        && !ForumAudienceEvaluator::decide(tenant_id, constraints, &security, &facts)?.allowed
    {
        return Ok(false);
    }
    Ok(true)
}

async fn category_is_public_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<bool> {
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Id)
        .limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
        return Err(ForumError::Validation(format!(
            "Forum category visibility tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
        )));
    }
    let parents = categories
        .into_iter()
        .map(|category| (category.id, category.parent_id))
        .collect::<HashMap<_, _>>();
    if !parents.contains_key(&category_id) {
        return Err(ForumError::CategoryNotFound(category_id));
    }

    let mut authenticated_overrides = HashSet::new();
    for policy in forum_category_policy::Entity::find()
        .filter(forum_category_policy::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?
    {
        if let Some(visibility) = policy.visibility_override {
            if visibility != ForumCategoryVisibility::Authenticated {
                return Err(ForumError::Validation(
                    "Forum category visibility storage contains a broadening override".to_string(),
                ));
            }
            authenticated_overrides.insert(policy.category_id);
        }
    }

    let mut current = Some(category_id);
    let mut visited = HashSet::new();
    let mut depth = 0usize;
    while let Some(current_id) = current {
        if depth > MAX_FORUM_CATEGORY_TREE_DEPTH {
            return Err(ForumError::Validation(format!(
                "Forum category visibility tree exceeds the maximum depth of {MAX_FORUM_CATEGORY_TREE_DEPTH}"
            )));
        }
        if !visited.insert(current_id) {
            return Err(ForumError::Validation(
                "Forum category visibility tree contains a hierarchy cycle".to_string(),
            ));
        }
        if authenticated_overrides.contains(&current_id) {
            return Ok(false);
        }
        current = parents.get(&current_id).copied().ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category visibility tree references missing category {current_id}"
            ))
        })?;
        depth += 1;
    }
    Ok(true)
}

async fn insert_tombstone_visibility_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    snapshot: TopicRouteTombstoneVisibilitySnapshot,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_route_tombstone_visibility (
                tenant_id, topic_id, publicly_disclosable,
                route_channel_restricted, created_at
            )
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, topic_id) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.publicly_disclosable.into(),
                snapshot.route_channel_restricted.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_route_tombstone_visibility (
                tenant_id, topic_id, publicly_disclosable,
                route_channel_restricted, created_at
            )
            VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, topic_id) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.publicly_disclosable.into(),
                snapshot.route_channel_restricted.into(),
            ],
        ),
        backend => return Err(unsupported_tombstone_visibility_backend(backend)),
    };
    txn.execute(statement).await?;
    Ok(())
}

async fn insert_tombstone_channels_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_route_tombstone_channels (
                tenant_id, topic_id, channel_slug
            )
            SELECT tenant_id, topic_id, channel_slug
            FROM forum_topic_channel_access
            WHERE tenant_id = $1 AND topic_id = $2
            ON CONFLICT (tenant_id, topic_id, channel_slug) DO NOTHING
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_route_tombstone_channels (
                tenant_id, topic_id, channel_slug
            )
            SELECT tenant_id, topic_id, channel_slug
            FROM forum_topic_channel_access
            WHERE tenant_id = ? AND topic_id = ?
            ON CONFLICT (tenant_id, topic_id, channel_slug) DO NOTHING
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_tombstone_visibility_backend(backend)),
    };
    txn.execute(statement).await?;
    Ok(())
}

async fn load_tombstone_visibility_snapshot(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<TopicRouteTombstoneVisibilitySnapshot>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT publicly_disclosable, route_channel_restricted
            FROM forum_topic_route_tombstone_visibility
            WHERE tenant_id = $1 AND topic_id = $2
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT publicly_disclosable, route_channel_restricted
            FROM forum_topic_route_tombstone_visibility
            WHERE tenant_id = ? AND topic_id = ?
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_tombstone_visibility_backend(backend)),
    };
    txn.query_one(statement)
        .await?
        .map(tombstone_visibility_snapshot_from_row)
        .transpose()
}

fn tombstone_visibility_snapshot_from_row(
    row: QueryResult,
) -> ForumResult<TopicRouteTombstoneVisibilitySnapshot> {
    Ok(TopicRouteTombstoneVisibilitySnapshot {
        publicly_disclosable: row.try_get("", "publicly_disclosable")?,
        route_channel_restricted: row.try_get("", "route_channel_restricted")?,
    })
}

async fn count_tombstone_channels(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<u64> {
    count_tombstone_channel_query(
        txn,
        tenant_id,
        topic_id,
        "SELECT COUNT(*) AS row_count FROM forum_topic_route_tombstone_channels WHERE tenant_id = $1 AND topic_id = $2",
        "SELECT COUNT(*) AS row_count FROM forum_topic_route_tombstone_channels WHERE tenant_id = ? AND topic_id = ?",
    )
    .await
}

async fn count_matching_tombstone_channels(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<u64> {
    count_tombstone_channel_query(
        txn,
        tenant_id,
        topic_id,
        r#"
        SELECT COUNT(*) AS row_count
        FROM forum_topic_route_tombstone_channels snapshot
        JOIN forum_topic_channel_access source
          ON source.tenant_id = snapshot.tenant_id
         AND source.topic_id = snapshot.topic_id
         AND source.channel_slug = snapshot.channel_slug
        WHERE snapshot.tenant_id = $1 AND snapshot.topic_id = $2
        "#,
        r#"
        SELECT COUNT(*) AS row_count
        FROM forum_topic_route_tombstone_channels snapshot
        JOIN forum_topic_channel_access source
          ON source.tenant_id = snapshot.tenant_id
         AND source.topic_id = snapshot.topic_id
         AND source.channel_slug = snapshot.channel_slug
        WHERE snapshot.tenant_id = ? AND snapshot.topic_id = ?
        "#,
    )
    .await
}

async fn count_tombstone_channel_query(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    postgres_sql: &str,
    sqlite_sql: &str,
) -> ForumResult<u64> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            postgres_sql,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sqlite_sql,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_tombstone_visibility_backend(backend)),
    };
    let row = txn
        .query_one(statement)
        .await?
        .ok_or(ForumError::TopicRouteResolutionConflict)?;
    let count: i64 = row.try_get("", "row_count")?;
    u64::try_from(count).map_err(|_| ForumError::TopicRouteResolutionConflict)
}

fn unsupported_tombstone_visibility_backend(backend: DatabaseBackend) -> ForumError {
    ForumError::Validation(format!(
        "Forum topic route tombstone visibility does not support database backend {backend:?}"
    ))
}

async fn claim_topic_delete_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_topics \
             SET updated_at = updated_at \
             WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

async fn mark_topic_thread_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<()> {
    txn.execute_unprepared(&format!(
        "UPDATE forum_replies \
         SET status = 'deleted', deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
         WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}' AND deleted_at IS NULL"
    ))
    .await?;

    let result = txn
        .execute_unprepared(&format!(
            "UPDATE forum_topics \
             SET status = 'archived', is_locked = TRUE, reply_count = 0, last_reply_at = NULL, \
                 deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}' AND deleted_at IS NULL"
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicDeleted);
    }
    Ok(())
}

fn map_flex_cleanup_error(error: rustok_core::field_schema::FlexError) -> ForumError {
    match error {
        rustok_core::field_schema::FlexError::Database(message) => {
            ForumError::Database(sea_orm::DbErr::Custom(message))
        }
        other => ForumError::Validation(other.to_string()),
    }
}
