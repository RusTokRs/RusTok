use std::collections::HashSet;

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Select,
    sea_query::{Expr, Query, SelectStatement},
};
use uuid::Uuid;

use crate::entities::{forum_topic, forum_topic_channel_access};
use crate::error::{ForumError, ForumResult};
use crate::state_machine::TopicStatus;

pub const MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES: usize = 100;
const MAX_FORUM_CHANNEL_SLUG_LEN: usize = 128;

/// Exact current storefront visibility scope owned by Forum.
///
/// FORUM-20 will extend this value with inherited ACL, role, trust, group and
/// explicit allow/deny inputs. Until those owner contracts exist, this scope is
/// deliberately limited to the already-supported public/exact-channel policy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumTopicVisibilityScope {
    channel_slug: Option<String>,
}

impl ForumTopicVisibilityScope {
    pub fn storefront(channel_slug: Option<&str>) -> ForumResult<Self> {
        let channel_slug = normalize_channel_slug(channel_slug)?;
        Ok(Self { channel_slug })
    }

    pub fn channel_slug(&self) -> Option<&str> {
        self.channel_slug.as_deref()
    }
}

/// Forum-owned exact topic visibility evaluation.
///
/// Missing, foreign-tenant, inactive and nonmatching channel targets all resolve
/// to absent values so callers cannot turn the policy into an existence oracle.
pub struct ForumTopicVisibilityService {
    db: DatabaseConnection,
}

impl ForumTopicVisibilityService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn is_topic_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        scope: &ForumTopicVisibilityScope,
    ) -> ForumResult<bool> {
        Ok(apply_storefront_visibility(
            forum_topic::Entity::find()
                .filter(forum_topic::Column::TenantId.eq(tenant_id))
                .filter(forum_topic::Column::Id.eq(topic_id)),
            tenant_id,
            scope,
        )
        .one(&self.db)
        .await?
        .is_some())
    }

    /// Filters an exact caller-selected topic set without discovering additional
    /// topics. Raw input is capped before deduplication and output preserves the
    /// first occurrence order from the caller.
    pub async fn filter_visible_topic_ids(
        &self,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
        scope: &ForumTopicVisibilityScope,
    ) -> ForumResult<Vec<Uuid>> {
        if topic_ids.len() > MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES {
            return Err(ForumError::Validation(format!(
                "Forum topic visibility candidates must not exceed {MAX_FORUM_TOPIC_VISIBILITY_CANDIDATES}"
            )));
        }
        if topic_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut unique_ids = Vec::with_capacity(topic_ids.len());
        let mut seen = HashSet::with_capacity(topic_ids.len());
        for topic_id in topic_ids {
            if seen.insert(*topic_id) {
                unique_ids.push(*topic_id);
            }
        }

        let visible = apply_storefront_visibility(
            forum_topic::Entity::find()
                .filter(forum_topic::Column::TenantId.eq(tenant_id))
                .filter(forum_topic::Column::Id.is_in(unique_ids.clone())),
            tenant_id,
            scope,
        )
        .all(&self.db)
        .await?
        .into_iter()
        .map(|topic| topic.id)
        .collect::<HashSet<_>>();

        Ok(unique_ids
            .into_iter()
            .filter(|topic_id| visible.contains(topic_id))
            .collect())
    }
}

pub(crate) fn apply_storefront_visibility(
    select: Select<forum_topic::Entity>,
    tenant_id: Uuid,
    scope: &ForumTopicVisibilityScope,
) -> Select<forum_topic::Entity> {
    let unrestricted = Expr::col((forum_topic::Entity, forum_topic::Column::Id))
        .not_in_subquery(all_topic_channel_access_subquery(tenant_id));
    let channel_condition = match scope.channel_slug() {
        Some(channel_slug) => Condition::any().add(unrestricted).add(
            Expr::col((forum_topic::Entity, forum_topic::Column::Id)).in_subquery(
                matching_topic_channel_access_subquery(tenant_id, channel_slug),
            ),
        ),
        None => Condition::all().add(unrestricted),
    };

    select
        .filter(forum_topic::Column::Status.eq(TopicStatus::Open))
        .filter(channel_condition)
}

fn all_topic_channel_access_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(
            Expr::col((
                forum_topic_channel_access::Entity,
                forum_topic_channel_access::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .to_owned()
}

fn matching_topic_channel_access_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(forum_topic_channel_access::Column::TopicId)
        .from(forum_topic_channel_access::Entity)
        .and_where(
            Expr::col((
                forum_topic_channel_access::Entity,
                forum_topic_channel_access::Column::TenantId,
            ))
            .eq(tenant_id),
        )
        .and_where(
            Expr::col((
                forum_topic_channel_access::Entity,
                forum_topic_channel_access::Column::ChannelSlug,
            ))
            .eq(channel_slug),
        )
        .to_owned()
}

fn normalize_channel_slug(channel_slug: Option<&str>) -> ForumResult<Option<String>> {
    let Some(channel_slug) = channel_slug.map(str::trim).filter(|slug| !slug.is_empty()) else {
        return Ok(None);
    };
    if channel_slug.len() > MAX_FORUM_CHANNEL_SLUG_LEN {
        return Err(ForumError::Validation(format!(
            "Forum channel slug must not exceed {MAX_FORUM_CHANNEL_SLUG_LEN} bytes"
        )));
    }
    Ok(Some(channel_slug.to_ascii_lowercase()))
}
