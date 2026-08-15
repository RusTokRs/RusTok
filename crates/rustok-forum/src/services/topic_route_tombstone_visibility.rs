use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, QueryResult, Statement,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::audience::{ForumAudienceEvaluator, ForumAudienceFacts};
use crate::entities::{forum_topic, forum_topic_channel_access};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

use super::category_audience::lock_category_tree_in_tx;
use super::category_visibility::is_category_public_to_anonymous;
use super::topic_audience::load_policy_for_topic;
use super::topic_audience_lock::lock_topic_audience_scopes_in_tx;

const MAX_FORUM_ROUTE_TOMBSTONE_CHANNEL_SLUG_LEN: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredForumTopicRouteTombstoneVisibility {
    publicly_disclosable: bool,
    route_channel_restricted: bool,
    route_channel_count: u64,
    route_channel_digest: String,
}

/// Forum-owned immutable disclosure snapshot for deleted topic routes.
///
/// The write side captures current anonymous visibility before the topic is soft-deleted. The read
/// side exposes only one boolean decision for a routed public channel. It never returns audience
/// selectors, historical channel lists, topic identity metadata or alias persistence details.
pub struct ForumTopicRouteTombstoneVisibilityService {
    db: DatabaseConnection,
}

impl ForumTopicRouteTombstoneVisibilityService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Acquires the category-tree policy scope before the caller claims the topic row.
    pub(crate) async fn lock_category_scope_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> ForumResult<()> {
        lock_category_tree_in_tx(txn, tenant_id).await
    }

    /// Acquires the canonical topic-audience advisory scope after the topic row has been claimed.
    pub(crate) async fn lock_topic_audience_scope_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<()> {
        lock_topic_audience_scopes_in_tx(txn, tenant_id, &[topic_id]).await
    }

    /// Records one exact immutable snapshot after the caller has locked and reloaded the topic row.
    /// Existing rows are compared only; replay never appends channels to an earlier snapshot.
    pub(crate) async fn record_locked_delete_snapshot_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic: &forum_topic::Model,
    ) -> ForumResult<()> {
        let channel_slugs = load_source_channel_slugs(txn, tenant_id, topic.id).await?;
        let snapshot = StoredForumTopicRouteTombstoneVisibility {
            publicly_disclosable: topic.status == TopicStatus::Open
                && is_category_public_to_anonymous(txn, tenant_id, topic.category_id).await?
                && topic_audience_allows_public_in_tx(txn, tenant_id, topic).await?,
            route_channel_restricted: !channel_slugs.is_empty(),
            route_channel_count: u64::try_from(channel_slugs.len())
                .map_err(|_| ForumError::TopicRouteResolutionConflict)?,
            route_channel_digest: route_channel_digest(&channel_slugs),
        };

        match load_snapshot(txn, tenant_id, topic.id).await? {
            Some(existing) => {
                if existing != snapshot {
                    return Err(ForumError::TopicRouteResolutionConflict);
                }
            }
            None => {
                insert_snapshot_in_tx(txn, tenant_id, topic.id, &snapshot).await?;
                insert_snapshot_channels_in_tx(txn, tenant_id, topic.id, &channel_slugs).await?;
            }
        }

        let stored = load_snapshot(txn, tenant_id, topic.id)
            .await?
            .ok_or(ForumError::TopicRouteResolutionConflict)?;
        let stored_channels = load_snapshot_channel_slugs(txn, tenant_id, topic.id).await?;
        validate_sealed_channel_scope(&stored, &stored_channels)?;
        if stored != snapshot || stored_channels != channel_slugs {
            return Err(ForumError::TopicRouteResolutionConflict);
        }
        Ok(())
    }

    /// Returns whether a stored gone route may be disclosed to one anonymous routed channel.
    /// Missing snapshots, private snapshots and nonmatching channel scopes all return `false`.
    pub async fn can_disclose_public_gone(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        channel_slug: Option<&str>,
    ) -> ForumResult<bool> {
        let Some(snapshot) = load_snapshot(&self.db, tenant_id, topic_id).await? else {
            return Ok(false);
        };
        let channel_slugs = load_snapshot_channel_slugs(&self.db, tenant_id, topic_id).await?;
        validate_sealed_channel_scope(&snapshot, &channel_slugs)?;
        if !snapshot.publicly_disclosable {
            return Ok(false);
        }
        if !snapshot.route_channel_restricted {
            return Ok(true);
        }
        let Some(channel_slug) = normalize_channel_slug(channel_slug)? else {
            return Ok(false);
        };
        Ok(channel_slugs.binary_search(&channel_slug).is_ok())
    }
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
        if !ForumAudienceEvaluator::decide(tenant_id, &layer.constraints, &security, &facts)?
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

async fn load_source_channel_slugs<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<String>>
where
    C: ConnectionTrait,
{
    let rows = forum_topic_channel_access::Entity::find()
        .filter(forum_topic_channel_access::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_channel_access::Column::TopicId.eq(topic_id))
        .order_by_asc(forum_topic_channel_access::Column::ChannelSlug)
        .all(db)
        .await?;
    rows.into_iter()
        .map(|row| {
            normalize_channel_slug(Some(row.channel_slug.as_str()))?
                .ok_or(ForumError::TopicRouteResolutionConflict)
        })
        .collect()
}

async fn load_snapshot_channel_slugs<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<String>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT channel_slug
            FROM forum_topic_route_tombstone_channels
            WHERE tenant_id = $1 AND topic_id = $2
            ORDER BY channel_slug
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT channel_slug
            FROM forum_topic_route_tombstone_channels
            WHERE tenant_id = ? AND topic_id = ?
            ORDER BY channel_slug
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    db.query_all(statement)
        .await?
        .into_iter()
        .map(|row| {
            let value: String = row.try_get("", "channel_slug")?;
            normalize_channel_slug(Some(value.as_str()))?
                .ok_or(ForumError::TopicRouteResolutionConflict)
        })
        .collect()
}

async fn load_snapshot<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Option<StoredForumTopicRouteTombstoneVisibility>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT publicly_disclosable, route_channel_restricted,
                   route_channel_count, route_channel_digest
            FROM forum_topic_route_tombstone_visibility
            WHERE tenant_id = $1 AND topic_id = $2
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT publicly_disclosable, route_channel_restricted,
                   route_channel_count, route_channel_digest
            FROM forum_topic_route_tombstone_visibility
            WHERE tenant_id = ? AND topic_id = ?
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    db.query_one(statement)
        .await?
        .map(snapshot_from_row)
        .transpose()
}

fn snapshot_from_row(row: QueryResult) -> ForumResult<StoredForumTopicRouteTombstoneVisibility> {
    let route_channel_count: i64 = row.try_get("", "route_channel_count")?;
    let route_channel_count =
        u64::try_from(route_channel_count).map_err(|_| ForumError::TopicRouteResolutionConflict)?;
    let route_channel_digest: String = row.try_get("", "route_channel_digest")?;
    if route_channel_digest.len() != 64
        || !route_channel_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    let snapshot = StoredForumTopicRouteTombstoneVisibility {
        publicly_disclosable: row.try_get("", "publicly_disclosable")?,
        route_channel_restricted: row.try_get("", "route_channel_restricted")?,
        route_channel_count,
        route_channel_digest,
    };
    if snapshot.route_channel_restricted != (snapshot.route_channel_count > 0) {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    Ok(snapshot)
}

async fn insert_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    snapshot: &StoredForumTopicRouteTombstoneVisibility,
) -> ForumResult<()> {
    let channel_count = i64::try_from(snapshot.route_channel_count)
        .map_err(|_| ForumError::TopicRouteResolutionConflict)?;
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_route_tombstone_visibility (
                tenant_id, topic_id, publicly_disclosable, route_channel_restricted,
                route_channel_count, route_channel_digest, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, topic_id) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.publicly_disclosable.into(),
                snapshot.route_channel_restricted.into(),
                channel_count.into(),
                snapshot.route_channel_digest.clone().into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_route_tombstone_visibility (
                tenant_id, topic_id, publicly_disclosable, route_channel_restricted,
                route_channel_count, route_channel_digest, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, topic_id) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                topic_id.into(),
                snapshot.publicly_disclosable.into(),
                snapshot.route_channel_restricted.into(),
                channel_count.into(),
                snapshot.route_channel_digest.clone().into(),
            ],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    txn.execute(statement).await?;
    Ok(())
}

async fn insert_snapshot_channels_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    channel_slugs: &[String],
) -> ForumResult<()> {
    for channel_slug in channel_slugs {
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                INSERT INTO forum_topic_route_tombstone_channels (
                    tenant_id, topic_id, channel_slug
                )
                VALUES ($1, $2, $3)
                ON CONFLICT (tenant_id, topic_id, channel_slug) DO NOTHING
                "#,
                vec![
                    tenant_id.into(),
                    topic_id.into(),
                    channel_slug.clone().into(),
                ],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_topic_route_tombstone_channels (
                    tenant_id, topic_id, channel_slug
                )
                VALUES (?, ?, ?)
                ON CONFLICT (tenant_id, topic_id, channel_slug) DO NOTHING
                "#,
                vec![
                    tenant_id.into(),
                    topic_id.into(),
                    channel_slug.clone().into(),
                ],
            ),
            backend => return Err(unsupported_backend(backend)),
        };
        txn.execute(statement).await?;
    }
    Ok(())
}

fn validate_sealed_channel_scope(
    snapshot: &StoredForumTopicRouteTombstoneVisibility,
    channel_slugs: &[String],
) -> ForumResult<()> {
    let channel_count =
        u64::try_from(channel_slugs.len()).map_err(|_| ForumError::TopicRouteResolutionConflict)?;
    if snapshot.route_channel_count != channel_count
        || snapshot.route_channel_restricted != !channel_slugs.is_empty()
        || snapshot.route_channel_digest != route_channel_digest(channel_slugs)
    {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    Ok(())
}

fn route_channel_digest(channel_slugs: &[String]) -> String {
    let mut digest = Sha256::new();
    for channel_slug in channel_slugs {
        digest.update((channel_slug.len() as u64).to_be_bytes());
        digest.update(channel_slug.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn normalize_channel_slug(channel_slug: Option<&str>) -> ForumResult<Option<String>> {
    let Some(raw_slug) = channel_slug else {
        return Ok(None);
    };
    if raw_slug.chars().any(char::is_control) {
        return Err(ForumError::Validation(
            "Forum topic route tombstone channel slug is invalid".to_string(),
        ));
    }
    let trimmed = raw_slug.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_FORUM_ROUTE_TOMBSTONE_CHANNEL_SLUG_LEN {
        return Err(ForumError::Validation(
            "Forum topic route tombstone channel slug is invalid".to_string(),
        ));
    }
    Ok(Some(trimmed.to_ascii_lowercase()))
}

fn unsupported_backend(backend: DatabaseBackend) -> ForumError {
    ForumError::Validation(format!(
        "Forum topic route tombstone visibility does not support database backend {backend:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_digest_is_order_and_boundary_sensitive() {
        assert_ne!(
            route_channel_digest(&["ab".to_string(), "c".to_string()]),
            route_channel_digest(&["a".to_string(), "bc".to_string()]),
        );
        assert_ne!(
            route_channel_digest(&["a".to_string(), "b".to_string()]),
            route_channel_digest(&["b".to_string(), "a".to_string()]),
        );
    }

    #[test]
    fn request_channel_normalization_is_bounded() {
        assert_eq!(
            normalize_channel_slug(Some(" Public ")).expect("channel"),
            Some("public".to_string())
        );
        assert!(normalize_channel_slug(Some("\nprivate")).is_err());
    }
}
