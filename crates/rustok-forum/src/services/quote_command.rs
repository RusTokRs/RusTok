use std::collections::BTreeSet;

use rustok_api::{normalize_locale_tag, Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    ForumQuoteReferenceInput, ForumQuoteTargetKindInput, ForumRelationQuoteResponse,
    ForumRelationSnapshotResponse, SetForumQuotesInput,
};
use crate::entities::{
    forum_audience_mention, forum_quote, forum_relation_revision, forum_reply, forum_reply_body,
    forum_topic, forum_topic_translation, forum_user_mention,
};
use crate::error::{ForumError, ForumResult};
use crate::mentions::{
    ForumContentTarget, ForumQuoteReference, FORUM_MAX_MENTION_TARGETS_PER_REVISION,
    FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
};

use super::mention_relation::MentionRelationService;
use super::rbac::enforce_owned_scope;

#[derive(Clone, Copy)]
enum ForumQuoteSource {
    Topic(Uuid),
    Reply(Uuid),
}

impl ForumQuoteSource {
    fn target(self) -> ForumContentTarget {
        match self {
            Self::Topic(id) => ForumContentTarget::topic(id),
            Self::Reply(id) => ForumContentTarget::reply(id),
        }
    }

    fn resource(self) -> Resource {
        match self {
            Self::Topic(_) => Resource::ForumTopics,
            Self::Reply(_) => Resource::ForumReplies,
        }
    }
}

#[derive(Clone)]
pub struct ForumQuoteCommandService {
    db: DatabaseConnection,
}

impl ForumQuoteCommandService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn set_topic_quotes(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: SetForumQuotesInput,
    ) -> ForumResult<ForumRelationSnapshotResponse> {
        self.set_quotes(tenant_id, ForumQuoteSource::Topic(topic_id), security, input)
            .await
    }

    pub async fn set_reply_quotes(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: SetForumQuotesInput,
    ) -> ForumResult<ForumRelationSnapshotResponse> {
        self.set_quotes(tenant_id, ForumQuoteSource::Reply(reply_id), security, input)
            .await
    }

    async fn set_quotes(
        &self,
        tenant_id: Uuid,
        source: ForumQuoteSource,
        security: SecurityContext,
        input: SetForumQuotesInput,
    ) -> ForumResult<ForumRelationSnapshotResponse> {
        if tenant_id.is_nil() {
            return Err(ForumError::relation_revision_unavailable());
        }
        let locale = normalize_locale_tag(&input.locale)
            .ok_or_else(ForumError::relation_revision_unavailable)?;
        let target = source.target();
        if target.id().is_nil() {
            return Err(ForumError::relation_revision_unavailable());
        }

        let (owner_id, body, body_format) = self.load_source(tenant_id, source, &locale).await?;
        enforce_owned_scope(&security, source.resource(), Action::Update, owner_id)?;
        let quotes = normalize_quote_references(input.quotes)?;

        let relations = MentionRelationService::new(self.db.clone());
        let prepared = relations
            .prepare(
                tenant_id,
                target,
                &locale,
                &body,
                &body_format,
                &security,
                quotes,
            )
            .await?;
        let txn = self.db.begin().await?;
        let result = relations.persist_in_tx(&txn, prepared).await?;
        let response = load_snapshot_in_tx(
            &txn,
            tenant_id,
            target,
            &locale,
            result.source().revision_id(),
        )
        .await?;
        txn.commit().await?;
        Ok(response)
    }

    async fn load_source(
        &self,
        tenant_id: Uuid,
        source: ForumQuoteSource,
        locale: &str,
    ) -> ForumResult<(Option<Uuid>, String, String)> {
        match source {
            ForumQuoteSource::Topic(topic_id) => {
                let topic = forum_topic::Entity::find_by_id(topic_id)
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await?
                    .ok_or(ForumError::TopicNotFound(topic_id))?;
                ensure_not_deleted(
                    &self.db,
                    "forum_topics",
                    "id",
                    tenant_id,
                    topic_id,
                    ForumError::TopicDeleted,
                )
                .await?;
                let translation = forum_topic_translation::Entity::find()
                    .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
                    .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
                    .filter(forum_topic_translation::Column::Locale.eq(locale))
                    .one(&self.db)
                    .await?
                    .ok_or_else(ForumError::relation_revision_unavailable)?;
                Ok((topic.author_id, translation.body, translation.body_format))
            }
            ForumQuoteSource::Reply(reply_id) => {
                let reply = forum_reply::Entity::find_by_id(reply_id)
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await?
                    .ok_or(ForumError::ReplyNotFound(reply_id))?;
                ensure_not_deleted(
                    &self.db,
                    "forum_replies",
                    "id",
                    tenant_id,
                    reply_id,
                    ForumError::ReplyDeleted,
                )
                .await?;
                let body = forum_reply_body::Entity::find()
                    .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
                    .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
                    .filter(forum_reply_body::Column::Locale.eq(locale))
                    .one(&self.db)
                    .await?
                    .ok_or_else(ForumError::relation_revision_unavailable)?;
                Ok((reply.author_id, body.body, body.body_format))
            }
        }
    }
}

async fn ensure_not_deleted(
    db: &DatabaseConnection,
    table: &'static str,
    id_column: &'static str,
    tenant_id: Uuid,
    source_id: Uuid,
    deleted_error: ForumError,
) -> ForumResult<()> {
    let statement = Statement::from_string(
        db.get_database_backend(),
        format!(
            "SELECT 1 AS active FROM {table} \
             WHERE tenant_id = '{tenant_id}' AND {id_column} = '{source_id}' \
               AND deleted_at IS NULL"
        ),
    );
    if db.query_one(statement).await?.is_none() {
        return Err(deleted_error);
    }
    Ok(())
}

async fn load_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: &str,
    revision_id: i64,
) -> ForumResult<ForumRelationSnapshotResponse> {
    let target_kind = target_kind(target);
    let revision = forum_relation_revision::Entity::find_by_id(revision_id)
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq(target_kind))
        .filter(forum_relation_revision::Column::TargetId.eq(target.id()))
        .filter(forum_relation_revision::Column::Locale.eq(locale))
        .one(txn)
        .await?
        .ok_or_else(ForumError::relation_revision_unavailable)?;
    let user_rows = forum_user_mention::Entity::find()
        .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
        .filter(forum_user_mention::Column::SourceKind.eq(target_kind))
        .filter(forum_user_mention::Column::SourceId.eq(target.id()))
        .filter(forum_user_mention::Column::SourceLocale.eq(locale))
        .filter(forum_user_mention::Column::SourceRevisionId.eq(revision_id))
        .order_by_asc(forum_user_mention::Column::MentionedUserId)
        .limit((FORUM_MAX_MENTION_TARGETS_PER_REVISION + 1) as u64)
        .all(txn)
        .await?;
    let audience_rows = forum_audience_mention::Entity::find()
        .filter(forum_audience_mention::Column::TenantId.eq(tenant_id))
        .filter(forum_audience_mention::Column::SourceKind.eq(target_kind))
        .filter(forum_audience_mention::Column::SourceId.eq(target.id()))
        .filter(forum_audience_mention::Column::SourceLocale.eq(locale))
        .filter(forum_audience_mention::Column::SourceRevisionId.eq(revision_id))
        .order_by_asc(forum_audience_mention::Column::Audience)
        .limit((FORUM_MAX_MENTION_TARGETS_PER_REVISION + 1) as u64)
        .all(txn)
        .await?;
    let quote_rows = forum_quote::Entity::find()
        .filter(forum_quote::Column::TenantId.eq(tenant_id))
        .filter(forum_quote::Column::SourceKind.eq(target_kind))
        .filter(forum_quote::Column::SourceId.eq(target.id()))
        .filter(forum_quote::Column::SourceLocale.eq(locale))
        .filter(forum_quote::Column::SourceRevisionId.eq(revision_id))
        .order_by_asc(forum_quote::Column::QuotedKind)
        .order_by_asc(forum_quote::Column::QuotedId)
        .order_by_asc(forum_quote::Column::QuotedRevisionId)
        .limit((FORUM_MAX_QUOTE_REFERENCES_PER_REVISION + 1) as u64)
        .all(txn)
        .await?;
    if user_rows.len() + audience_rows.len() > FORUM_MAX_MENTION_TARGETS_PER_REVISION
        || quote_rows.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION
    {
        return Err(ForumError::Validation(
            "Persisted Forum relation snapshot exceeds owner command limits".to_string(),
        ));
    }
    Ok(ForumRelationSnapshotResponse {
        revision_id,
        target_kind: revision.target_kind,
        target_id: revision.target_id,
        locale: revision.locale,
        user_ids: user_rows.into_iter().map(|row| row.mentioned_user_id).collect(),
        audiences: audience_rows.into_iter().map(|row| row.audience).collect(),
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

fn normalize_quote_references(
    inputs: Vec<ForumQuoteReferenceInput>,
) -> ForumResult<Vec<ForumQuoteReference>> {
    if inputs.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumError::Validation(format!(
            "Forum revision exceeds the {FORUM_MAX_QUOTE_REFERENCES_PER_REVISION}-quote limit"
        )));
    }
    let inputs = inputs.into_iter().collect::<BTreeSet<_>>();
    inputs
        .into_iter()
        .map(|input| {
            let target = match input.target_kind {
                ForumQuoteTargetKindInput::Topic => ForumContentTarget::topic(input.target_id),
                ForumQuoteTargetKindInput::Reply => ForumContentTarget::reply(input.target_id),
            };
            ForumQuoteReference::new(target, input.revision_id)
        })
        .collect()
}

fn target_kind(target: ForumContentTarget) -> &'static str {
    match target.kind() {
        crate::mentions::ForumContentTargetKind::Topic => "topic",
        crate::mentions::ForumContentTargetKind::Reply => "reply",
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_quote_references;
    use crate::dto::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput};
    use crate::mentions::FORUM_MAX_QUOTE_REFERENCES_PER_REVISION;
    use uuid::Uuid;

    #[test]
    fn quote_inputs_are_deduplicated_and_bounded() {
        let target_id = Uuid::new_v4();
        let quote = ForumQuoteReferenceInput {
            target_kind: ForumQuoteTargetKindInput::Reply,
            target_id,
            revision_id: 7,
        };
        let normalized = normalize_quote_references(vec![quote.clone(), quote])
            .expect("duplicate references should normalize");
        assert_eq!(normalized.len(), 1);

        let oversized = (0..=FORUM_MAX_QUOTE_REFERENCES_PER_REVISION)
            .map(|index| ForumQuoteReferenceInput {
                target_kind: ForumQuoteTargetKindInput::Topic,
                target_id: Uuid::new_v4(),
                revision_id: index as i64 + 1,
            })
            .collect();
        assert!(normalize_quote_references(oversized).is_err());
    }

    #[test]
    fn quote_inputs_reject_nil_targets_and_non_positive_revisions() {
        assert!(
            normalize_quote_references(vec![ForumQuoteReferenceInput {
                target_kind: ForumQuoteTargetKindInput::Topic,
                target_id: Uuid::nil(),
                revision_id: 1,
            }])
            .is_err()
        );
        assert!(
            normalize_quote_references(vec![ForumQuoteReferenceInput {
                target_kind: ForumQuoteTargetKindInput::Reply,
                target_id: Uuid::new_v4(),
                revision_id: 0,
            }])
            .is_err()
        );
    }
}
