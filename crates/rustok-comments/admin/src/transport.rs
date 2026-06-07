//! Module-owned transport facade for the comments admin UI.
//!
//! The current admin surface intentionally exposes a temporary single-adapter
//! native server-function transport. Keeping this facade between `ui/leptos` and
//! `api` prevents render code from owning transport calls and leaves room for a
//! future GraphQL/headless fallback adapter without changing UI state wiring.

pub(crate) use crate::api::ApiError;
use crate::api::CommentThreadsPayload;
use rustok_comments::{
    CommentRecord, CommentStatus, CommentThreadDetail, CommentThreadStatus, CommentThreadSummary,
};

pub(crate) async fn fetch_threads(
    page: u64,
    per_page: u64,
    target_type: String,
    thread_status: Option<CommentThreadStatus>,
    comment_status: Option<CommentStatus>,
) -> Result<CommentThreadsPayload, ApiError> {
    crate::api::fetch_threads(page, per_page, target_type, thread_status, comment_status).await
}

pub(crate) async fn fetch_thread_detail(
    thread_id: String,
    locale: String,
    page: u64,
    per_page: u64,
) -> Result<CommentThreadDetail, ApiError> {
    crate::api::fetch_thread_detail(thread_id, locale, page, per_page).await
}

pub(crate) async fn set_thread_status(
    thread_id: String,
    status: CommentThreadStatus,
) -> Result<CommentThreadSummary, ApiError> {
    crate::api::set_thread_status(thread_id, status).await
}

pub(crate) async fn set_comment_status(
    comment_id: String,
    status: CommentStatus,
    locale: String,
) -> Result<CommentRecord, ApiError> {
    crate::api::set_comment_status(comment_id, status, locale).await
}
