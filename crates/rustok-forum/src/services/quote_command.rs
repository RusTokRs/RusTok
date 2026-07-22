use std::collections::BTreeSet;

use rustok_api::{normalize_locale_tag, Action, Resource};
use rustok_core::SecurityContext;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    ForumQuoteReferenceInput, ForumQuoteTargetKindInput, ForumRelationSnapshotQuery,
    ForumRelationSnapshotResponse, SetForumQuotesInput,
};
use crate::entities::{
    forum_reply, forum_reply_body, forum_topic, forum_topic_translation,
};
use crate::error::{ForumError, ForumResult};
use crate::mentions::{
    ForumContentTarget, ForumQuoteReference, FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
};

use super::mention_relation::MentionRelationService;
use super::rbac::enforce_owned_scope;
use super::relation_read::ForumRelationReadService;

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

        let (owner_id, body, body_format) = self
            .load_source(tenant_id, source, &locale)
            .await?;
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
        let revision_id = result.source().revision_id();
        txn.commit().await?;

        ForumRelationReadService::new(self.db.clone())
            .get(
                tenant_id,
                security,
                ForumRelationSnapshotQuery {
                    target_kind: target_kind(target).to_string(),
                    target_id: target.id(),
                    locale,
                    revision_id: Some(revision_id),
                },
            )
            .await
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

fn normalize_quote_references(
    inputs: Vec<ForumQuoteReferenceInput>,
) -> ForumResult<Vec<ForumQuoteReference>> {
    let inputs = inputs.into_iter().collect::<BTreeSet<_>>();
    if inputs.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumError::Validation(format!(
            "Forum revision exceeds the {FORUM_MAX_QUOTE_REFERENCES_PER_REVISION}-quote limit"
        )));
    }
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
