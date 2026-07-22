use std::collections::HashMap;

use rustok_content::{normalize_locale_code, resolve_by_locale_with_fallback};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::dto::{CommentListItem, CommentStatus, ListCommentsFilter};
use crate::entities::{comment, comment_body, comment_thread};
use crate::error::{CommentsError, CommentsResult};

const MAX_PUBLIC_COMMENTS_PER_PAGE: u64 = 100;

/// Public owner-side projection for a comment thread.
///
/// This path intentionally bypasses caller RBAC conversion and applies the
/// public visibility invariant in the owner query itself: only approved,
/// non-deleted comments can leave the Comments bounded context.
pub(crate) async fn list_public_comments_for_target(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    filter: ListCommentsFilter,
    fallback_locale: Option<&str>,
) -> CommentsResult<(Vec<CommentListItem>, u64)> {
    let requested_locale = normalize_locale(&filter.locale)?;
    let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

    let Some(thread) = comment_thread::Entity::find()
        .filter(comment_thread::Column::TenantId.eq(tenant_id))
        .filter(comment_thread::Column::TargetType.eq(target_type))
        .filter(comment_thread::Column::TargetId.eq(target_id))
        .one(db)
        .await?
    else {
        return Ok((Vec::new(), 0));
    };

    let per_page = filter.per_page.clamp(1, MAX_PUBLIC_COMMENTS_PER_PAGE);
    let paginator = comment::Entity::find()
        .filter(comment::Column::TenantId.eq(tenant_id))
        .filter(comment::Column::ThreadId.eq(thread.id))
        .filter(comment::Column::DeletedAt.is_null())
        .filter(comment::Column::Status.eq(CommentStatus::Approved))
        .order_by_asc(comment::Column::Position)
        .paginate(db, per_page);

    let total = paginator.num_items().await?;
    let comments = paginator.fetch_page(filter.page.saturating_sub(1)).await?;
    let comment_ids = comments.iter().map(|item| item.id).collect::<Vec<_>>();

    let mut bodies_by_comment: HashMap<Uuid, Vec<comment_body::Model>> = HashMap::new();
    if !comment_ids.is_empty() {
        for body in comment_body::Entity::find()
            .filter(comment_body::Column::CommentId.is_in(comment_ids))
            .all(db)
            .await?
        {
            bodies_by_comment
                .entry(body.comment_id)
                .or_default()
                .push(body);
        }
    }

    let items = comments
        .into_iter()
        .map(|item| {
            let resolved = resolve_body(
                bodies_by_comment.remove(&item.id).unwrap_or_default(),
                requested_locale.as_str(),
                fallback_locale.as_deref(),
            )?;
            let body_preview = resolved.body.chars().take(200).collect();

            Ok(CommentListItem {
                id: item.id,
                thread_id: item.thread_id,
                target_type: thread.target_type.clone(),
                target_id: thread.target_id,
                requested_locale: requested_locale.clone(),
                effective_locale: resolved.effective_locale,
                author_id: item.author_id,
                parent_comment_id: item.parent_comment_id,
                body_preview,
                status: item.status,
                position: item.position,
                created_at: item.created_at.to_rfc3339(),
            })
        })
        .collect::<CommentsResult<Vec<_>>>()?;

    Ok((items, total))
}

fn normalize_locale(locale: &str) -> CommentsResult<String> {
    normalize_locale_code(locale)
        .ok_or_else(|| CommentsError::Validation("Invalid locale".to_string()))
}

struct ResolvedBody {
    effective_locale: String,
    body: String,
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

    let resolved =
        resolve_by_locale_with_fallback(&bodies, requested_locale, fallback_locale, |body| {
            body.locale.as_str()
        });
    let chosen = resolved.item.cloned().unwrap_or_else(|| bodies[0].clone());

    Ok(ResolvedBody {
        effective_locale: resolved.effective_locale,
        body: chosen.body,
    })
}
