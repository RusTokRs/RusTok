use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, JoinType,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, TransactionTrait,
};
use std::{collections::HashMap, time::Instant};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    dto::validation::validate_body_format, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::{Action, PermissionScope, Resource, SecurityContext};
use rustok_telemetry::metrics;

use crate::dto::{
    CommentListItem, CommentRecord, CommentThreadDetail, CommentThreadSummary, CreateCommentInput,
    ListCommentsFilter, UpdateCommentInput,
};
use crate::entities::{comment, comment_body, comment_thread};
use crate::error::{CommentsError, CommentsResult};

pub struct CommentsService {
    db: DatabaseConnection,
}

const MODULE: &str = "comments";
const LIBRARY_PATH: &str = "library";

#[cfg(test)]
mod locale_fallback_tests {
    use super::*;

    #[test]
    fn resolve_body_uses_platform_fallback_before_first_available() {
        let now = Utc::now().into();
        let resolved = resolve_body(
            vec![
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "de".to_string(),
                    body: "Hallo".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    body: "Hello".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
            ],
            "ru",
            None,
        )
        .expect("body should resolve");

        assert_eq!(resolved.effective_locale, "en");
        assert_eq!(resolved.body, "Hello");
    }

    #[test]
    fn resolve_body_normalizes_explicit_fallback_locale() {
        let now = Utc::now().into();
        let resolved = resolve_body(
            vec![
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "en-us".to_string(),
                    body: "Hello".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "de".to_string(),
                    body: "Hallo".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
            ],
            "fr-fr",
            Some("EN_us"),
        )
        .expect("body should resolve");

        assert_eq!(resolved.effective_locale, "en-us");
        assert_eq!(resolved.body, "Hello");
    }
}

#[cfg(test)]
use rustok_core::CONTENT_FORMAT_MARKDOWN;

#[cfg(test)]
use sea_orm::Database;

impl CommentsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input), fields(tenant_id = %tenant_id, target_type = %input.target_type, target_id = %input.target_id))]
    pub async fn create_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCommentInput,
    ) -> CommentsResult<CommentRecord> {
        record_entrypoint("create_comment");
        let started = Instant::now();
        let result = async {
            let locale = input.locale.clone();
            let txn = self.db.begin().await?;
            let comment_id = self
                .create_comment_in_tx(&txn, tenant_id, security.clone(), input)
                .await?;
            txn.commit().await?;
            self.get_comment(tenant_id, security, comment_id, &locale, None)
                .await
        }
        .await;
        record_operation_result("comments.create_comment", started, &result);
        result
    }

    pub async fn create_comment_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateCommentInput,
    ) -> CommentsResult<Uuid> {
        let author_id = self.enforce_create_scope(&security)?;
        self.validate_body(&input.body, &input.body_format)?;

        let thread = self
            .find_or_create_thread_in_tx(txn, tenant_id, &input.target_type, input.target_id)
            .await?;
        self.ensure_thread_is_open(&thread)?;
        let status = self.resolve_create_status(&security, input.status)?;

        if let Some(parent_comment_id) = input.parent_comment_id {
            let parent = self
                .find_comment_in_tx(txn, tenant_id, parent_comment_id, true)
                .await?;
            if parent.thread_id != thread.id {
                return Err(CommentsError::Validation(
                    "Parent comment belongs to another thread".to_string(),
                ));
            }
        }

        let now = Utc::now();
        let position = self.next_position_in_tx(txn, thread.id).await?;
        let comment_id = Uuid::new_v4();
        let locale = normalize_locale(&input.locale)?;

        comment::ActiveModel {
            id: Set(comment_id),
            tenant_id: Set(tenant_id),
            thread_id: Set(thread.id),
            author_id: Set(author_id),
            parent_comment_id: Set(input.parent_comment_id),
            status: Set(status),
            position: Set(position),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(txn)
        .await?;

        comment_body::ActiveModel {
            id: Set(Uuid::new_v4()),
            comment_id: Set(comment_id),
            locale: Set(locale),
            body: Set(input.body),
            body_format: Set(input.body_format),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        self.update_thread_counters_in_tx(txn, &thread, 1, Some(now.into()))
            .await?;

        Ok(comment_id)
    }

    #[instrument(skip(self, security))]
    pub async fn get_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommentsResult<CommentRecord> {
        record_entrypoint("get_comment");
        let started = Instant::now();
        let result = async {
            self.enforce_read_scope(&security, Action::Read)?;
            let locale = normalize_locale(locale)?;
            let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

            let comment = self.find_comment(tenant_id, comment_id, false).await?;
            let thread = comment_thread::Entity::find_by_id(comment.thread_id)
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| CommentsError::CommentThreadNotFound {
                    target_type: "unknown".to_string(),
                    target_id: Uuid::nil(),
                })?;
            let bodies = comment_body::Entity::find()
                .filter(comment_body::Column::CommentId.eq(comment.id))
                .all(&self.db)
                .await?;

            self.build_comment_record(comment, thread, bodies, &locale, fallback_locale.as_deref())
        }
        .await;
        record_operation_result("comments.get_comment", started, &result);
        result
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        input: UpdateCommentInput,
    ) -> CommentsResult<CommentRecord> {
        record_entrypoint("update_comment");
        let started = Instant::now();
        let result = async {
            let existing = self.find_comment(tenant_id, comment_id, false).await?;
            self.enforce_owned_scope(&security, Action::Update, existing.author_id)?;

            let locale = normalize_locale(&input.locale)?;
            if input.body.is_none() && input.body_format.is_none() {
                return self
                    .get_comment(tenant_id, security, comment_id, &locale, None)
                    .await;
            }

            let body = input
                .body
                .ok_or_else(|| CommentsError::Validation("Comment body is required".to_string()))?;
            let body_format = input.body_format.unwrap_or_else(|| "markdown".to_string());
            self.validate_body(&body, &body_format)?;

            let txn = self.db.begin().await?;
            self.upsert_body_in_tx(&txn, comment_id, &locale, body, body_format)
                .await?;

            let mut active: comment::ActiveModel = existing.into();
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
            txn.commit().await?;

            self.get_comment(tenant_id, security, comment_id, &locale, None)
                .await
        }
        .await;
        record_operation_result("comments.update_comment", started, &result);
        result
    }

    #[instrument(skip(self, security))]
    pub async fn delete_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
    ) -> CommentsResult<()> {
        record_entrypoint("delete_comment");
        let started = Instant::now();
        let result = async {
            let txn = self.db.begin().await?;
            self.delete_comment_in_tx(&txn, tenant_id, security, comment_id)
                .await?;
            txn.commit().await?;
            Ok(())
        }
        .await;
        record_operation_result("comments.delete_comment", started, &result);
        result
    }

    pub async fn delete_comment_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
    ) -> CommentsResult<()> {
        let existing = self
            .find_comment_in_tx(txn, tenant_id, comment_id, false)
            .await?;
        self.enforce_owned_scope(&security, Action::Delete, existing.author_id)?;

        let thread = comment_thread::Entity::find_by_id(existing.thread_id)
            .filter(comment_thread::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| CommentsError::CommentThreadNotFound {
                target_type: "unknown".to_string(),
                target_id: Uuid::nil(),
            })?;

        let mut active: comment::ActiveModel = existing.into();
        active.deleted_at = Set(Some(Utc::now().into()));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        self.update_thread_counters_in_tx(txn, &thread, -1, None)
            .await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_comments_for_target(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        target_type: &str,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<&str>,
    ) -> CommentsResult<(Vec<CommentListItem>, u64)> {
        record_entrypoint("list_comments_for_target");
        let started = Instant::now();
        let result = async {
            self.enforce_read_scope(&security, Action::List)?;
            let locale = normalize_locale(&filter.locale)?;
            let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
            let requested_limit = Some(filter.per_page);
            let effective_limit = filter.per_page.max(1);

            let thread_lookup_started = Instant::now();
            let thread = comment_thread::Entity::find()
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .filter(comment_thread::Column::TargetType.eq(target_type))
                .filter(comment_thread::Column::TargetId.eq(target_id))
                .one(&self.db)
                .await?;
            metrics::record_read_path_query(
                LIBRARY_PATH,
                "comments.list_comments_for_target",
                "comment_threads.lookup",
                thread_lookup_started.elapsed().as_secs_f64(),
                if thread.is_some() { 1 } else { 0 },
            );

            let Some(thread) = thread else {
                metrics::record_read_path_budget(
                    LIBRARY_PATH,
                    "comments.list_comments_for_target",
                    requested_limit,
                    effective_limit,
                    0,
                );
                return Ok((Vec::new(), 0));
            };

            let paginator = comment::Entity::find()
                .filter(comment::Column::TenantId.eq(tenant_id))
                .filter(comment::Column::ThreadId.eq(thread.id))
                .filter(comment::Column::DeletedAt.is_null())
                .order_by_asc(comment::Column::Position)
                .paginate(&self.db, effective_limit);

            let page_query_started = Instant::now();
            let total = paginator.num_items().await?;
            let comments = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
            metrics::record_read_path_query(
                LIBRARY_PATH,
                "comments.list_comments_for_target",
                "comments.page",
                page_query_started.elapsed().as_secs_f64(),
                comments.len() as u64,
            );

            let comment_ids: Vec<Uuid> = comments.iter().map(|item| item.id).collect();
            let body_query_started = Instant::now();
            let bodies = comment_body::Entity::find()
                .filter(comment_body::Column::CommentId.is_in(comment_ids))
                .all(&self.db)
                .await?;
            metrics::record_read_path_query(
                LIBRARY_PATH,
                "comments.list_comments_for_target",
                "comment_bodies.batch",
                body_query_started.elapsed().as_secs_f64(),
                bodies.len() as u64,
            );

            let mut bodies_map: HashMap<Uuid, Vec<comment_body::Model>> = HashMap::new();
            for body in bodies {
                bodies_map.entry(body.comment_id).or_default().push(body);
            }

            let items = comments
                .into_iter()
                .map(|item| {
                    let resolved = resolve_body(
                        bodies_map.remove(&item.id).unwrap_or_default(),
                        &locale,
                        fallback_locale.as_deref(),
                    )?;
                    let preview: String = resolved.body.chars().take(200).collect();

                    Ok(CommentListItem {
                        id: item.id,
                        thread_id: item.thread_id,
                        target_type: thread.target_type.clone(),
                        target_id: thread.target_id,
                        requested_locale: locale.clone(),
                        effective_locale: resolved.effective_locale,
                        author_id: item.author_id,
                        parent_comment_id: item.parent_comment_id,
                        body_preview: preview,
                        status: item.status,
                        position: item.position,
                        created_at: item.created_at.to_rfc3339(),
                    })
                })
                .collect::<CommentsResult<Vec<_>>>()?;

            metrics::record_read_path_budget(
                LIBRARY_PATH,
                "comments.list_comments_for_target",
                requested_limit,
                effective_limit,
                items.len(),
            );

            Ok((items, total))
        }
        .await;
        record_operation_result("comments.list_comments_for_target", started, &result);
        result
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, target_type = %target_type, target_id = %target_id, status = ?status))]
    pub async fn set_thread_status_for_target(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        target_type: &str,
        target_id: Uuid,
        status: crate::dto::CommentThreadStatus,
    ) -> CommentsResult<()> {
        record_entrypoint("set_thread_status_for_target");
        let started = Instant::now();
        let result = async {
            self.enforce_moderation_scope(&security)?;
            let thread = comment_thread::Entity::find()
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .filter(comment_thread::Column::TargetType.eq(target_type))
                .filter(comment_thread::Column::TargetId.eq(target_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| CommentsError::CommentThreadNotFound {
                    target_type: target_type.to_string(),
                    target_id,
                })?;

            if thread.status == status {
                return Ok(());
            }

            let mut active: comment_thread::ActiveModel = thread.into();
            active.status = Set(status);
            active.updated_at = Set(Utc::now().into());
            active.update(&self.db).await?;
            Ok(())
        }
        .await;
        record_operation_result("comments.set_thread_status_for_target", started, &result);
        result
    }

    #[instrument(skip(self, security))]
    pub async fn list_threads(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page: u64,
        per_page: u64,
        target_type: Option<&str>,
        thread_status: Option<crate::dto::CommentThreadStatus>,
        comment_status: Option<crate::dto::CommentStatus>,
    ) -> CommentsResult<(Vec<CommentThreadSummary>, u64)> {
        record_entrypoint("list_threads");
        let started = Instant::now();
        let result = async {
            self.enforce_read_scope(&security, Action::List)?;

            let mut query = comment_thread::Entity::find()
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .order_by_desc(comment_thread::Column::LastCommentedAt)
                .order_by_desc(comment_thread::Column::UpdatedAt);

            if let Some(target_type) = target_type.map(str::trim).filter(|value| !value.is_empty())
            {
                query = query.filter(comment_thread::Column::TargetType.eq(target_type));
            }

            if let Some(thread_status) = thread_status {
                query = query.filter(comment_thread::Column::Status.eq(thread_status));
            }

            if let Some(comment_status) = comment_status {
                query = query
                    .join(
                        JoinType::InnerJoin,
                        comment_thread::Relation::Comments.def(),
                    )
                    .filter(comment::Column::DeletedAt.is_null())
                    .filter(comment::Column::Status.eq(comment_status))
                    .distinct();
            }

            let paginator = query.paginate(&self.db, per_page.max(1));
            let total = paginator.num_items().await?;
            let threads = paginator.fetch_page(page.saturating_sub(1)).await?;

            Ok((
                threads
                    .into_iter()
                    .map(Self::map_thread_summary)
                    .collect::<Vec<_>>(),
                total,
            ))
        }
        .await;
        record_operation_result("comments.list_threads", started, &result);
        result
    }

    #[instrument(skip(self, security))]
    pub async fn get_thread_detail(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        thread_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        page: u64,
        per_page: u64,
    ) -> CommentsResult<CommentThreadDetail> {
        record_entrypoint("get_thread_detail");
        let started = Instant::now();
        let result = async {
            self.enforce_read_scope(&security, Action::Read)?;
            let locale = normalize_locale(locale)?;
            let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
            let thread = comment_thread::Entity::find_by_id(thread_id)
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| CommentsError::CommentThreadNotFound {
                    target_type: "unknown".to_string(),
                    target_id: Uuid::nil(),
                })?;

            let paginator = comment::Entity::find()
                .filter(comment::Column::TenantId.eq(tenant_id))
                .filter(comment::Column::ThreadId.eq(thread.id))
                .filter(comment::Column::DeletedAt.is_null())
                .order_by_asc(comment::Column::Position)
                .paginate(&self.db, per_page.max(1));
            let total_comments = paginator.num_items().await?;
            let comments = paginator.fetch_page(page.saturating_sub(1)).await?;

            let comment_ids = comments.iter().map(|item| item.id).collect::<Vec<_>>();
            let mut bodies_map: HashMap<Uuid, Vec<comment_body::Model>> = HashMap::new();
            if !comment_ids.is_empty() {
                let bodies = comment_body::Entity::find()
                    .filter(comment_body::Column::CommentId.is_in(comment_ids))
                    .all(&self.db)
                    .await?;
                for body in bodies {
                    bodies_map.entry(body.comment_id).or_default().push(body);
                }
            }

            let comments = comments
                .into_iter()
                .map(|comment| {
                    let comment_id = comment.id;
                    self.build_comment_record(
                        comment,
                        thread.clone(),
                        bodies_map.remove(&comment_id).unwrap_or_default(),
                        &locale,
                        fallback_locale.as_deref(),
                    )
                })
                .collect::<CommentsResult<Vec<_>>>()?;

            Ok(CommentThreadDetail {
                thread: Self::map_thread_summary(thread),
                comments,
                total_comments,
            })
        }
        .await;
        record_operation_result("comments.get_thread_detail", started, &result);
        result
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, comment_id = %comment_id, status = ?status))]
    pub async fn set_comment_status(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        comment_id: Uuid,
        status: crate::dto::CommentStatus,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommentsResult<CommentRecord> {
        record_entrypoint("set_comment_status");
        let started = Instant::now();
        let result = async {
            self.enforce_moderation_scope(&security)?;
            let locale = normalize_locale(locale)?;
            let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

            let existing = self.find_comment(tenant_id, comment_id, false).await?;
            if existing.status != status {
                let mut active: comment::ActiveModel = existing.clone().into();
                active.status = Set(status);
                active.updated_at = Set(Utc::now().into());
                active.update(&self.db).await?;
            }

            self.get_comment(
                tenant_id,
                security,
                comment_id,
                &locale,
                fallback_locale.as_deref(),
            )
            .await
        }
        .await;
        record_operation_result("comments.set_comment_status", started, &result);
        result
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, thread_id = %thread_id, status = ?status))]
    pub async fn set_thread_status(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        thread_id: Uuid,
        status: crate::dto::CommentThreadStatus,
    ) -> CommentsResult<CommentThreadSummary> {
        record_entrypoint("set_thread_status");
        let started = Instant::now();
        let result = async {
            self.enforce_moderation_scope(&security)?;
            let thread = comment_thread::Entity::find_by_id(thread_id)
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await?
                .ok_or_else(|| CommentsError::CommentThreadNotFound {
                    target_type: "unknown".to_string(),
                    target_id: Uuid::nil(),
                })?;

            if thread.status == status {
                return Ok(Self::map_thread_summary(thread));
            }

            let mut active: comment_thread::ActiveModel = thread.clone().into();
            active.status = Set(status);
            active.updated_at = Set(Utc::now().into());
            let thread = active.update(&self.db).await?;
            Ok(Self::map_thread_summary(thread))
        }
        .await;
        record_operation_result("comments.set_thread_status", started, &result);
        result
    }

    async fn find_or_create_thread_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Uuid,
    ) -> CommentsResult<comment_thread::Model> {
        if let Some(thread) = comment_thread::Entity::find()
            .filter(comment_thread::Column::TenantId.eq(tenant_id))
            .filter(comment_thread::Column::TargetType.eq(target_type))
            .filter(comment_thread::Column::TargetId.eq(target_id))
            .one(txn)
            .await?
        {
            return Ok(thread);
        }

        let now = Utc::now();
        let thread = comment_thread::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_type: Set(target_type.to_string()),
            target_id: Set(target_id),
            status: Set(crate::dto::CommentThreadStatus::Open),
            comment_count: Set(0),
            last_commented_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        match thread.insert(txn).await {
            Ok(thread) => Ok(thread),
            Err(_) => comment_thread::Entity::find()
                .filter(comment_thread::Column::TenantId.eq(tenant_id))
                .filter(comment_thread::Column::TargetType.eq(target_type))
                .filter(comment_thread::Column::TargetId.eq(target_id))
                .one(txn)
                .await?
                .ok_or_else(|| CommentsError::CommentThreadNotFound {
                    target_type: target_type.to_string(),
                    target_id,
                }),
        }
    }

    async fn next_position_in_tx(
        &self,
        txn: &DatabaseTransaction,
        thread_id: Uuid,
    ) -> CommentsResult<i64> {
        Ok(comment::Entity::find()
            .filter(comment::Column::ThreadId.eq(thread_id))
            .order_by_desc(comment::Column::Position)
            .one(txn)
            .await?
            .map(|item| item.position + 1)
            .unwrap_or(1))
    }

    async fn find_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        include_deleted: bool,
    ) -> CommentsResult<comment::Model> {
        self.find_comment_inner(&self.db, tenant_id, comment_id, include_deleted)
            .await
    }

    async fn find_comment_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        comment_id: Uuid,
        include_deleted: bool,
    ) -> CommentsResult<comment::Model> {
        self.find_comment_inner(txn, tenant_id, comment_id, include_deleted)
            .await
    }

    async fn find_comment_inner(
        &self,
        conn: &impl sea_orm::ConnectionTrait,
        tenant_id: Uuid,
        comment_id: Uuid,
        include_deleted: bool,
    ) -> CommentsResult<comment::Model> {
        let mut query =
            comment::Entity::find_by_id(comment_id).filter(comment::Column::TenantId.eq(tenant_id));
        if !include_deleted {
            query = query.filter(comment::Column::DeletedAt.is_null());
        }
        query
            .one(conn)
            .await?
            .ok_or(CommentsError::CommentNotFound(comment_id))
    }

    async fn upsert_body_in_tx(
        &self,
        txn: &DatabaseTransaction,
        comment_id: Uuid,
        locale: &str,
        body: String,
        body_format: String,
    ) -> CommentsResult<()> {
        let existing = comment_body::Entity::find()
            .filter(comment_body::Column::CommentId.eq(comment_id))
            .filter(comment_body::Column::Locale.eq(locale))
            .one(txn)
            .await?;

        match existing {
            Some(existing) => {
                let mut active: comment_body::ActiveModel = existing.into();
                active.body = Set(body);
                active.body_format = Set(body_format);
                active.updated_at = Set(Utc::now().into());
                active.update(txn).await?;
            }
            None => {
                comment_body::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    comment_id: Set(comment_id),
                    locale: Set(locale.to_string()),
                    body: Set(body),
                    body_format: Set(body_format),
                    created_at: Set(Utc::now().into()),
                    updated_at: Set(Utc::now().into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }

    async fn update_thread_counters_in_tx(
        &self,
        txn: &DatabaseTransaction,
        thread: &comment_thread::Model,
        delta: i32,
        last_commented_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
    ) -> CommentsResult<()> {
        let mut active: comment_thread::ActiveModel = thread.clone().into();
        active.comment_count = Set((thread.comment_count + delta).max(0));
        active.updated_at = Set(Utc::now().into());
        if let Some(last_commented_at) = last_commented_at {
            active.last_commented_at = Set(Some(last_commented_at));
        }
        active.update(txn).await?;
        Ok(())
    }

    fn build_comment_record(
        &self,
        comment: comment::Model,
        thread: comment_thread::Model,
        bodies: Vec<comment_body::Model>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommentsResult<CommentRecord> {
        let resolved = resolve_body(bodies, locale, fallback_locale)?;
        Ok(CommentRecord {
            id: comment.id,
            thread_id: comment.thread_id,
            target_type: thread.target_type,
            target_id: thread.target_id,
            requested_locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            author_id: comment.author_id,
            parent_comment_id: comment.parent_comment_id,
            body: resolved.body,
            body_format: resolved.body_format,
            status: comment.status,
            position: comment.position,
            created_at: comment.created_at.to_rfc3339(),
            updated_at: comment.updated_at.to_rfc3339(),
        })
    }

    fn map_thread_summary(thread: comment_thread::Model) -> CommentThreadSummary {
        CommentThreadSummary {
            id: thread.id,
            tenant_id: thread.tenant_id,
            target_type: thread.target_type,
            target_id: thread.target_id,
            status: thread.status,
            comment_count: thread.comment_count,
            last_commented_at: thread.last_commented_at.map(|value| value.to_rfc3339()),
            created_at: thread.created_at.to_rfc3339(),
            updated_at: thread.updated_at.to_rfc3339(),
        }
    }

    fn enforce_create_scope(&self, security: &SecurityContext) -> CommentsResult<Uuid> {
        match security.get_scope(Resource::Comments, Action::Create) {
            PermissionScope::All | PermissionScope::Own => security
                .user_id
                .ok_or_else(|| CommentsError::Forbidden("Comment author is required".to_string())),
            PermissionScope::None => Err(CommentsError::Forbidden("Permission denied".to_string())),
        }
    }

    fn enforce_read_scope(&self, security: &SecurityContext, action: Action) -> CommentsResult<()> {
        if matches!(
            security.get_scope(Resource::Comments, action),
            PermissionScope::None
        ) {
            return Err(CommentsError::Forbidden("Permission denied".to_string()));
        }
        Ok(())
    }

    fn enforce_owned_scope(
        &self,
        security: &SecurityContext,
        action: Action,
        author_id: Uuid,
    ) -> CommentsResult<()> {
        match security.get_scope(Resource::Comments, action) {
            PermissionScope::All => Ok(()),
            PermissionScope::Own if security.user_id == Some(author_id) => Ok(()),
            PermissionScope::Own | PermissionScope::None => {
                Err(CommentsError::Forbidden("Permission denied".to_string()))
            }
        }
    }

    fn enforce_moderation_scope(&self, security: &SecurityContext) -> CommentsResult<()> {
        if self.can_moderate(security) {
            return Ok(());
        }
        Err(CommentsError::Forbidden("Permission denied".to_string()))
    }

    fn validate_body(&self, body: &str, body_format: &str) -> CommentsResult<()> {
        if body_format.trim().is_empty() {
            return Err(CommentsError::Validation(
                "Comment body format is required".to_string(),
            ));
        }
        if validate_body_format(body_format).is_err() {
            return Err(CommentsError::Validation(format!(
                "Unsupported comment body format: {body_format}"
            )));
        }
        if body_format != "rt_json_v1" && body_format != "rt_json" && body.trim().is_empty() {
            return Err(CommentsError::Validation(
                "Comment body cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn ensure_thread_is_open(&self, thread: &comment_thread::Model) -> CommentsResult<()> {
        if thread.status == crate::dto::CommentThreadStatus::Closed {
            return Err(CommentsError::CommentThreadClosed {
                target_type: thread.target_type.clone(),
                target_id: thread.target_id,
            });
        }
        Ok(())
    }

    fn resolve_create_status(
        &self,
        security: &SecurityContext,
        requested: crate::dto::CommentStatus,
    ) -> CommentsResult<crate::dto::CommentStatus> {
        match requested {
            crate::dto::CommentStatus::Pending | crate::dto::CommentStatus::Approved => {
                Ok(requested)
            }
            crate::dto::CommentStatus::Spam | crate::dto::CommentStatus::Trash
                if self.can_moderate(security) =>
            {
                Ok(requested)
            }
            crate::dto::CommentStatus::Spam | crate::dto::CommentStatus::Trash => {
                Err(CommentsError::Forbidden(
                    "Only moderators can create comments with spam/trash status".to_string(),
                ))
            }
        }
    }

    fn can_moderate(&self, security: &SecurityContext) -> bool {
        !matches!(
            security.get_scope(Resource::Comments, Action::Moderate),
            PermissionScope::None
        ) || !matches!(
            security.get_scope(Resource::Comments, Action::Manage),
            PermissionScope::None
        )
    }
}

#[cfg(test)]
mod format_validation_tests {
    use super::*;
    use crate::migrations;
    use rustok_core::UserRole;
    use sea_orm_migration::SchemaManager;

    #[tokio::test]
    async fn rejects_unknown_comment_body_format() {
        let db_url = format!(
            "sqlite:file:comments_format_validation_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let db = Database::connect(db_url)
            .await
            .expect("sqlite connection should succeed");
        let service = CommentsService::new(db);

        let err = service
            .validate_body("hello", "xml")
            .expect_err("unsupported format should be rejected");

        match err {
            CommentsError::Validation(message) => {
                assert!(message.contains("Unsupported comment body format"))
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn allows_rt_json_alias_with_empty_body_payload() {
        let db_url = format!(
            "sqlite:file:comments_format_validation_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let db = Database::connect(db_url)
            .await
            .expect("sqlite connection should succeed");
        let service = CommentsService::new(db);

        service
            .validate_body("", "rt_json")
            .expect("rt_json alias should follow shared rich-text contract");
        service
            .validate_body("", "rt_json_v1")
            .expect("rt_json_v1 should allow canonical rich-text payload");
        service
            .validate_body("hello", CONTENT_FORMAT_MARKDOWN)
            .expect("markdown should remain valid");
    }

    async fn setup_comments_service() -> CommentsService {
        let db_url = format!(
            "sqlite:file:comments_status_contract_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let db = Database::connect(db_url)
            .await
            .expect("sqlite connection should succeed");
        let manager = SchemaManager::new(&db);
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("comments migration should apply");
        }
        CommentsService::new(db)
    }

    #[tokio::test]
    async fn closed_thread_rejects_new_comment_creation() {
        let service = setup_comments_service().await;
        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
        let moderator = SecurityContext::system();

        service
            .create_comment(
                tenant_id,
                customer.clone(),
                CreateCommentInput {
                    target_type: "blog_post".to_string(),
                    target_id,
                    locale: "en".to_string(),
                    body: "first".to_string(),
                    body_format: "markdown".to_string(),
                    parent_comment_id: None,
                    status: crate::dto::CommentStatus::Pending,
                },
            )
            .await
            .expect("initial comment should create the thread");

        service
            .set_thread_status_for_target(
                tenant_id,
                moderator,
                "blog_post",
                target_id,
                crate::dto::CommentThreadStatus::Closed,
            )
            .await
            .expect("moderator should be able to close the thread");

        let err = service
            .create_comment(
                tenant_id,
                customer,
                CreateCommentInput {
                    target_type: "blog_post".to_string(),
                    target_id,
                    locale: "en".to_string(),
                    body: "second".to_string(),
                    body_format: "markdown".to_string(),
                    parent_comment_id: None,
                    status: crate::dto::CommentStatus::Pending,
                },
            )
            .await
            .expect_err("closed thread must reject new comments");

        assert!(matches!(
            err,
            CommentsError::CommentThreadClosed {
                target_type,
                target_id: closed_target_id
            } if target_type == "blog_post" && closed_target_id == target_id
        ));
    }

    #[tokio::test]
    async fn non_moderator_cannot_create_spam_or_trash_comment() {
        let service = setup_comments_service().await;
        let tenant_id = Uuid::new_v4();
        let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

        let err = service
            .create_comment(
                tenant_id,
                customer,
                CreateCommentInput {
                    target_type: "blog_post".to_string(),
                    target_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    body: "spam".to_string(),
                    body_format: "markdown".to_string(),
                    parent_comment_id: None,
                    status: crate::dto::CommentStatus::Spam,
                },
            )
            .await
            .expect_err("non-moderator should not create spam comments");

        assert!(matches!(err, CommentsError::Forbidden(_)));
    }
}

fn normalize_locale(locale: &str) -> CommentsResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| CommentsError::Validation("Invalid locale".to_string()))
}

fn record_entrypoint(entry_point: &str) {
    metrics::record_module_entrypoint_call(MODULE, entry_point, LIBRARY_PATH);
}

fn record_operation_result<T>(operation: &str, started: Instant, result: &CommentsResult<T>) {
    metrics::record_span_duration(operation, started.elapsed().as_secs_f64());
    if let Err(error) = result {
        metrics::record_span_error(operation, error.kind());
        metrics::record_module_error(MODULE, error.kind(), error.severity());
    }
}

struct ResolvedBody {
    effective_locale: String,
    body: String,
    body_format: String,
}

fn resolve_body(
    bodies: Vec<comment_body::Model>,
    requested_locale: &str,
    fallback_locale: Option<&str>,
) -> CommentsResult<ResolvedBody> {
    if bodies.is_empty() {
        return Err(CommentsError::Validation(
            "Comment body payload is missing".to_string(),
        ));
    }

    let requested = normalize_locale(requested_locale)?;
    let fallback = fallback_locale.map(normalize_locale).transpose()?;
    let resolved =
        resolve_by_locale_with_fallback(&bodies, &requested, fallback.as_deref(), |body| {
            body.locale.as_str()
        });
    let chosen = resolved.item.cloned().unwrap_or_else(|| bodies[0].clone());
    Ok(ResolvedBody {
        effective_locale: resolved.effective_locale,
        body: chosen.body,
        body_format: chosen.body_format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_body_prefers_requested_locale() {
        let now = Utc::now().into();
        let resolved = resolve_body(
            vec![
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "en".to_string(),
                    body: "Hello".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
                comment_body::Model {
                    id: Uuid::new_v4(),
                    comment_id: Uuid::new_v4(),
                    locale: "ru".to_string(),
                    body: "Привет".to_string(),
                    body_format: "markdown".to_string(),
                    created_at: now,
                    updated_at: now,
                },
            ],
            "ru",
            Some("en"),
        )
        .expect("body should resolve");

        assert_eq!(resolved.effective_locale, "ru");
        assert_eq!(resolved.body, "Привет");
    }
}
