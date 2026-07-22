use rustok_api::{normalize_locale_tag, Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::dto::{
    ForumRelationQuoteResponse, ForumRelationSnapshotQuery, ForumRelationSnapshotResponse,
};
use crate::entities::{
    forum_audience_mention, forum_quote, forum_relation_revision, forum_user_mention,
};
use crate::error::{ForumError, ForumResult};
use crate::services::rbac::enforce_scope;

const MAX_MENTIONS_PER_REVISION: usize = 32;
const MAX_QUOTES_PER_REVISION: usize = 32;

#[derive(Clone)]
pub struct ForumRelationReadService {
    db: DatabaseConnection,
}

impl ForumRelationReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: ForumRelationSnapshotQuery,
    ) -> ForumResult<ForumRelationSnapshotResponse> {
        let target_kind = normalize_target_kind(&query.target_kind)?;
        enforce_scope(&security, resource_for_target_kind(target_kind), Action::Read)?;
        if tenant_id.is_nil() || query.target_id.is_nil() {
            return Err(ForumError::relation_revision_unavailable());
        }
        let locale = normalize_locale_tag(&query.locale)
            .ok_or_else(ForumError::relation_revision_unavailable)?;
        if query.revision_id.is_some_and(|revision_id| revision_id <= 0) {
            return Err(ForumError::relation_revision_unavailable());
        }

        let mut select = forum_relation_revision::Entity::find()
            .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
            .filter(forum_relation_revision::Column::TargetKind.eq(target_kind))
            .filter(forum_relation_revision::Column::TargetId.eq(query.target_id))
            .filter(forum_relation_revision::Column::Locale.eq(&locale));
        if let Some(revision_id) = query.revision_id {
            select = select.filter(forum_relation_revision::Column::RevisionId.eq(revision_id));
        }
        let revision = select
            .order_by_desc(forum_relation_revision::Column::RevisionId)
            .one(&self.db)
            .await?
            .ok_or_else(ForumError::relation_revision_unavailable)?;

        let user_rows = forum_user_mention::Entity::find()
            .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
            .filter(forum_user_mention::Column::SourceKind.eq(target_kind))
            .filter(forum_user_mention::Column::SourceId.eq(query.target_id))
            .filter(forum_user_mention::Column::SourceLocale.eq(&locale))
            .filter(
                forum_user_mention::Column::SourceRevisionId.eq(revision.revision_id),
            )
            .order_by_asc(forum_user_mention::Column::MentionedUserId)
            .all(&self.db)
            .await?;
        let audience_rows = forum_audience_mention::Entity::find()
            .filter(forum_audience_mention::Column::TenantId.eq(tenant_id))
            .filter(forum_audience_mention::Column::SourceKind.eq(target_kind))
            .filter(forum_audience_mention::Column::SourceId.eq(query.target_id))
            .filter(forum_audience_mention::Column::SourceLocale.eq(&locale))
            .filter(
                forum_audience_mention::Column::SourceRevisionId.eq(revision.revision_id),
            )
            .order_by_asc(forum_audience_mention::Column::Audience)
            .all(&self.db)
            .await?;
        let quote_rows = forum_quote::Entity::find()
            .filter(forum_quote::Column::TenantId.eq(tenant_id))
            .filter(forum_quote::Column::SourceKind.eq(target_kind))
            .filter(forum_quote::Column::SourceId.eq(query.target_id))
            .filter(forum_quote::Column::SourceLocale.eq(&locale))
            .filter(forum_quote::Column::SourceRevisionId.eq(revision.revision_id))
            .order_by_asc(forum_quote::Column::QuotedKind)
            .order_by_asc(forum_quote::Column::QuotedId)
            .order_by_asc(forum_quote::Column::QuotedRevisionId)
            .all(&self.db)
            .await?;

        if user_rows.len() + audience_rows.len() > MAX_MENTIONS_PER_REVISION
            || quote_rows.len() > MAX_QUOTES_PER_REVISION
        {
            return Err(ForumError::Validation(
                "Persisted Forum relation snapshot exceeds owner read limits".to_string(),
            ));
        }

        Ok(ForumRelationSnapshotResponse {
            revision_id: revision.revision_id,
            target_kind: revision.target_kind,
            target_id: revision.target_id,
            locale: revision.locale,
            user_ids: user_rows
                .into_iter()
                .map(|row| row.mentioned_user_id)
                .collect(),
            audiences: audience_rows
                .into_iter()
                .map(|row| row.audience)
                .collect(),
            quotes: quote_rows
                .into_iter()
                .map(|row| ForumRelationQuoteResponse {
                    target_kind: row.quoted_kind,
                    target_id: row.quoted_id,
                    revision_id: row.quoted_revision_id,
                })
                .collect(),
            created_at: revision.created_at.to_rfc3339(),
        })
    }
}

fn normalize_target_kind(value: &str) -> ForumResult<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "topic" => Ok("topic"),
        "reply" => Ok("reply"),
        _ => Err(ForumError::relation_revision_unavailable()),
    }
}

fn resource_for_target_kind(target_kind: &str) -> Resource {
    match target_kind {
        "topic" => Resource::ForumTopics,
        "reply" => Resource::ForumReplies,
        _ => unreachable!("target kind is normalized before authorization"),
    }
}
