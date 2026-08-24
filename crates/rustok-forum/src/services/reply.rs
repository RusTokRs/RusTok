use std::collections::HashMap;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection,
    DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource};
use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse, UpdateReplyInput,
};
use crate::entities::{forum_reply, forum_reply_body, forum_solution};
use crate::error::{ForumError, ForumResult};
use crate::richtext::{normalize_discussion, project_stored_discussion, serialize_discussion};
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::user_stats::UserStatsService;
use crate::services::vote::{VoteService, VoteSummary};
use crate::services::{CategoryService, TopicService};
use crate::state_machine::{ReplyStatus, TopicStatus};

pub struct ReplyService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ReplyService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        enforce_scope(&security, Resource::ForumReplies, Action::Create)?;
        let locale = normalize_locale(&input.locale)?;
        let txn = self.db.begin().await?;
        let topic = TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        let category =
            CategoryService::find_category_in_tx(&txn, tenant_id, topic.category_id).await?;

        if topic.status == TopicStatus::Closed {
            return Err(ForumError::TopicClosed);
        }
        if topic.status == TopicStatus::Archived {
            return Err(ForumError::TopicArchived);
        }

        let stored_body = serialize_discussion(input.content)?;

        if let Some(parent_reply_id) = input.parent_reply_id {
            let parent = Self::find_reply_in_tx(&txn, tenant_id, parent_reply_id).await?;
            if parent.topic_id != topic_id {
                return Err(ForumError::Validation(
                    "Parent reply belongs to another topic".to_string(),
                ));
            }
        }

        let position = Self::next_position_in_tx(&txn, topic_id).await?;
        let reply_id = Uuid::new_v4();
        let now = Utc::now();
        forum_reply::ActiveModel {
            id: Set(reply_id),
            tenant_id: Set(tenant_id),
            topic_id: Set(topic_id),
            author_id: Set(security.user_id),
            parent_reply_id: Set(input.parent_reply_id),
            status: Set(if category.moderated {
                ReplyStatus::Pending
            } else {
                ReplyStatus::Approved
            }),
            position: Set(position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        forum_reply_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            reply_id: Set(reply_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            body: Set(stored_body),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let topic = TopicService::adjust_reply_count_in_tx(&txn, tenant_id, topic_id, 1).await?;
        CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, 1).await?;
        UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, security.user_id, 1).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicReplied {
                    topic_id,
                    reply_id,
                    author_id: security.user_id,
                },
            )
            .await?;

        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
    ) -> ForumResult<ReplyResponse> {
        self.get_with_locale_fallback(tenant_id, security, reply_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        reply_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ReplyResponse> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let reply = self.find_reply(tenant_id, reply_id).await?;
        let bodies = self.load_bodies(tenant_id, reply_id).await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, reply.topic_id)
            .await?;
        let vote_summary = VoteService::new(self.db.clone())
            .reply_vote_summary(tenant_id, reply_id, security.user_id)
            .await?;
        to_reply_response(
            reply,
            bodies,
            vote_summary,
            solution_reply_id,
            &locale,
            fallback_locale.as_deref(),
        )
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        let locale = normalize_locale(&input.locale)?;
        let existing = self.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;

        let Some(content) = input.content else {
            return self.get(tenant_id, security, reply_id, &locale).await;
        };
        let stored_body = serialize_discussion(content)?;

        let txn = self.db.begin().await?;
        self.upsert_body_in_tx(&txn, tenant_id, reply_id, &locale, stored_body)
            .await?;

        let mut active: forum_reply::ActiveModel = existing.into();
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        let reply = self.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Delete,
            reply.author_id,
        )?;
        let txn = self.db.begin().await?;
        let solution_removed = forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .one(&txn)
            .await?
            .is_some_and(|solution| solution.reply_id == reply_id);
        forum_reply::Entity::delete_by_id(reply_id)
            .exec(&txn)
            .await?;
        let topic =
            TopicService::adjust_reply_count_in_tx(&txn, tenant_id, reply.topic_id, -1).await?;
        CategoryService::adjust_counters_in_tx(&txn, tenant_id, topic.category_id, 0, -1).await?;
        UserStatsService::adjust_reply_count_in_tx(&txn, tenant_id, reply.author_id, -1).await?;
        if solution_removed {
            UserStatsService::adjust_solution_count_in_tx(&txn, tenant_id, reply.author_id, -1)
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        self.list_for_topic_with_locale_fallback(tenant_id, security, topic_id, filter, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let (replies, total) = self
            .fetch_reply_page(tenant_id, topic_id, filter.page, filter.per_page, None)
            .await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, topic_id)
            .await?;
        let reply_ids: Vec<Uuid> = replies.iter().map(|reply| reply.id).collect();
        let bodies_map = self.load_bodies_map(tenant_id, &reply_ids).await?;
        let vote_summaries = VoteService::new(self.db.clone())
            .reply_vote_summaries(tenant_id, &reply_ids, security.user_id)
            .await?;

        let items = replies
            .into_iter()
            .map(|reply| {
                let bodies = bodies_map.get(&reply.id).cloned().unwrap_or_default();
                let resolved = resolve_reply_body(&bodies, &locale, fallback_locale.as_deref());
                let body = resolved.item.ok_or_else(|| {
                    ForumError::Validation("Forum reply body is unavailable".to_string())
                })?;
                let preview: String = project_stored_discussion(&body.body)?
                    .plain_text
                    .chars()
                    .take(200)
                    .collect();
                Ok(ReplyListItem {
                    id: reply.id,
                    locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    topic_id: reply.topic_id,
                    author_id: reply.author_id,
                    content_preview: preview,
                    status: reply.status.to_string(),
                    vote_score: vote_summaries
                        .get(&reply.id)
                        .map(|summary| summary.score)
                        .unwrap_or_default(),
                    current_user_vote: vote_summaries
                        .get(&reply.id)
                        .and_then(|summary| summary.current_user_vote),
                    is_solution: Some(reply.id) == solution_reply_id,
                    parent_reply_id: reply.parent_reply_id,
                    created_at: reply.created_at.to_rfc3339(),
                })
            })
            .collect::<ForumResult<Vec<_>>>()?;

        Ok((items, total))
    }

    #[instrument(skip(self, security))]
    pub async fn list_response_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        self.list_response_for_topic_by_statuses_with_locale_fallback(
            tenant_id,
            security,
            topic_id,
            filter,
            fallback_locale,
            None,
        )
        .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        enforce_scope(&security, Resource::ForumReplies, Action::List)?;
        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let (replies, total) = self
            .fetch_reply_page(tenant_id, topic_id, filter.page, filter.per_page, statuses)
            .await?;
        let solution_reply_id = self
            .load_solution_reply_id_for_topic(tenant_id, topic_id)
            .await?;
        let reply_ids: Vec<Uuid> = replies.iter().map(|reply| reply.id).collect();
        let bodies_map = self.load_bodies_map(tenant_id, &reply_ids).await?;
        let vote_summaries = VoteService::new(self.db.clone())
            .reply_vote_summaries(tenant_id, &reply_ids, security.user_id)
            .await?;

        let items = replies
            .into_iter()
            .map(|reply| {
                let reply_id = reply.id;
                to_reply_response(
                    reply,
                    bodies_map.get(&reply_id).cloned().unwrap_or_default(),
                    vote_summaries.get(&reply_id).copied().unwrap_or_default(),
                    solution_reply_id,
                    &locale,
                    fallback_locale.as_deref(),
                )
            })
            .collect::<ForumResult<Vec<_>>>()?;

        Ok((items, total))
    }

    pub(crate) async fn find_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        Self::find_reply_in_conn(&self.db, tenant_id, reply_id).await
    }

    pub(crate) async fn find_reply_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        Self::find_reply_in_conn(txn, tenant_id, reply_id).await
    }

    async fn find_reply_in_conn(
        conn: &impl sea_orm::ConnectionTrait,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<forum_reply::Model> {
        forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))
    }

    pub(crate) async fn set_status_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        status: ReplyStatus,
    ) -> ForumResult<forum_reply::Model> {
        let reply = Self::find_reply_in_tx(txn, tenant_id, reply_id).await?;
        let mut active: forum_reply::ActiveModel = reply.clone().into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(reply)
    }

    async fn load_bodies(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
    ) -> ForumResult<Vec<forum_reply_body::Model>> {
        Ok(forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .all(&self.db)
            .await?)
    }

    async fn load_solution_reply_id_for_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> ForumResult<Option<Uuid>> {
        Ok(forum_solution::Entity::find()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(topic_id))
            .one(&self.db)
            .await?
            .map(|solution| solution.reply_id))
    }

    async fn fetch_reply_page(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        page: u64,
        per_page: u64,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<forum_reply::Model>, u64)> {
        let mut query = forum_reply::Entity::find()
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .order_by_asc(forum_reply::Column::Position);

        if let Some(statuses) = statuses {
            if !statuses.is_empty() {
                let mut condition = Condition::any();
                for status in statuses {
                    condition = condition.add(forum_reply::Column::Status.eq(*status));
                }
                query = query.filter(condition);
            }
        }

        let paginator = query.paginate(&self.db, per_page.max(1));
        let total = paginator.num_items().await?;
        let replies = paginator.fetch_page(page.saturating_sub(1)).await?;
        Ok((replies, total))
    }

    async fn load_bodies_map(
        &self,
        tenant_id: Uuid,
        reply_ids: &[Uuid],
    ) -> ForumResult<HashMap<Uuid, Vec<forum_reply_body::Model>>> {
        if reply_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.is_in(reply_ids.to_vec()))
            .all(&self.db)
            .await?;
        let mut map: HashMap<Uuid, Vec<forum_reply_body::Model>> = HashMap::new();
        for row in rows {
            map.entry(row.reply_id).or_default().push(row);
        }
        Ok(map)
    }

    async fn next_position_in_tx(txn: &DatabaseTransaction, topic_id: Uuid) -> ForumResult<i64> {
        Ok(forum_reply::Entity::find()
            .filter(forum_reply::Column::TopicId.eq(topic_id))
            .order_by_desc(forum_reply::Column::Position)
            .one(txn)
            .await?
            .map(|reply| reply.position + 1)
            .unwrap_or(1))
    }

    async fn upsert_body_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
        body: String,
    ) -> ForumResult<()> {
        let existing = forum_reply_body::Entity::find()
            .filter(forum_reply_body::Column::TenantId.eq(tenant_id))
            .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
            .filter(forum_reply_body::Column::Locale.eq(locale))
            .one(txn)
            .await?;
        let now = Utc::now();

        match existing {
            Some(existing) => {
                let mut active: forum_reply_body::ActiveModel = existing.into();
                active.body = Set(body);
                active.updated_at = Set(now.into());
                active.update(txn).await?;
            }
            None => {
                forum_reply_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    reply_id: Set(reply_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.to_string()),
                    body: Set(body),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }
}

fn to_reply_response(
    reply: forum_reply::Model,
    bodies: Vec<forum_reply_body::Model>,
    vote_summary: VoteSummary,
    solution_reply_id: Option<Uuid>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> ForumResult<ReplyResponse> {
    let resolved = resolve_reply_body(&bodies, locale, fallback_locale);
    let body = resolved
        .item
        .ok_or_else(|| ForumError::Validation("Forum reply body is unavailable".to_string()))?;
    let content = project_stored_discussion(&body.body)?;

    Ok(ReplyResponse {
        id: reply.id,
        requested_locale: locale.to_string(),
        locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        topic_id: reply.topic_id,
        author_id: reply.author_id,
        content: content.view,
        content_plain_text: content.plain_text,
        status: reply.status.to_string(),
        vote_score: vote_summary.score,
        current_user_vote: vote_summary.current_user_vote,
        is_solution: Some(reply.id) == solution_reply_id,
        parent_reply_id: reply.parent_reply_id,
        created_at: reply.created_at.to_rfc3339(),
        updated_at: reply.updated_at.to_rfc3339(),
    })
}

fn normalize_locale(locale: &str) -> ForumResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid locale".to_string()))
}

fn resolve_reply_body<'a>(
    bodies: &'a [forum_reply_body::Model],
    locale: &str,
    fallback_locale: Option<&str>,
) -> rustok_content::ResolvedLocale<'a, forum_reply_body::Model> {
    resolve_by_locale_with_fallback(bodies, locale, fallback_locale, |body| body.locale.as_str())
}

#[cfg(test)]
mod tests {
    use super::ReplyService;
    use crate::{
        CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
        ListRepliesFilter, TopicService, migrations,
    };
    use rustok_core::SecurityContext;
    use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
    use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use std::sync::Arc;
    use uuid::Uuid;

    async fn setup_forum_test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:forum_reply_service_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect forum reply test sqlite database")
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
        .expect("identity owner table should exist for forum tests");

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
                .expect("forum migration should apply");
        }
    }

    #[tokio::test]
    async fn list_response_preserves_reply_order_by_position() {
        let db = setup_forum_test_db().await;
        ensure_forum_schema(&db).await;

        let transport = OutboxTransport::new(db.clone());
        let event_bus = TransactionalEventBus::new(Arc::new(transport));

        let tenant_id = Uuid::new_v4();
        let security = SecurityContext::system();

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

        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateTopicInput {
                    locale: "en".to_string(),
                    category_id: category.id,
                    title: "Ordered topic".to_string(),
                    slug: Some("ordered-topic".to_string()),
                    body: rustok_api::RichTextDocument::single_paragraph("Body"),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("topic should be created");

        let service = ReplyService::new(db.clone(), event_bus.clone());
        for content in ["first", "second", "third"] {
            service
                .create(
                    tenant_id,
                    security.clone(),
                    topic.id,
                    CreateReplyInput {
                        locale: "en".to_string(),
                        content: rustok_api::RichTextDocument::single_paragraph(content),
                        parent_reply_id: None,
                    },
                )
                .await
                .expect("reply should be created");
        }

        let (replies, total) = service
            .list_response_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic.id,
                ListRepliesFilter {
                    locale: Some("en".to_string()),
                    page: 1,
                    per_page: 20,
                },
                None,
            )
            .await
            .expect("reply list should load");

        assert_eq!(total, 3);
        assert_eq!(replies.len(), 3);
        let contents = replies
            .into_iter()
            .map(|reply| reply.content.document)
            .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                rustok_api::RichTextDocument::single_paragraph("first"),
                rustok_api::RichTextDocument::single_paragraph("second"),
                rustok_api::RichTextDocument::single_paragraph("third"),
            ]
        );
    }
}

impl ReplyService {
    pub(crate) async fn update_with_relations(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        let locale = normalize_locale(&input.locale)?;
        let existing = self.find_reply(tenant_id, reply_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumReplies,
            Action::Update,
            existing.author_id,
        )?;

        let Some(content) = input.content else {
            return self.get(tenant_id, security, reply_id, &locale).await;
        };
        let document = normalize_discussion(content)?;
        let stored_body = serialize_discussion(document.clone())?;
        let relation_service =
            super::mention_relation::MentionRelationService::new(self.db.clone());
        let prepared_relations = relation_service
            .prepare(
                tenant_id,
                crate::mentions::ForumContentTarget::reply(reply_id),
                &locale,
                &document,
                &security,
                std::iter::empty(),
            )
            .await?;

        let txn = self.db.begin().await?;
        self.upsert_body_in_tx(&txn, tenant_id, reply_id, &locale, stored_body)
            .await?;
        relation_service
            .persist_in_tx(&txn, prepared_relations)
            .await?;

        let mut active: forum_reply::ActiveModel = existing.into();
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        txn.commit().await?;
        self.get(tenant_id, security, reply_id, &locale).await
    }
}
