//! Framework-agnostic helpers for the comments admin UI.
//!
//! This layer owns request/view policy that can be reused by future host adapters
//! without depending on framework runtime types.

use rustok_comments::{
    CommentRecord, CommentStatus, CommentThreadDetail, CommentThreadStatus, CommentThreadSummary,
};
use rustok_ui_core::{AdminQueryKey, UiRouteQueryIntent, normalize_ui_text};

pub(crate) const COMMENTS_ADMIN_THREAD_QUERY_KEY: &str = AdminQueryKey::ThreadId.as_str();
pub(crate) const COMMENTS_ADMIN_LOCALE_QUERY_KEY: &str = AdminQueryKey::Locale.as_str();

pub(crate) fn comments_admin_select_thread_query_intent(
    thread_id: &str,
) -> Option<UiRouteQueryIntent> {
    let thread_id = normalize_ui_text(thread_id)?;
    Some(UiRouteQueryIntent::push(
        COMMENTS_ADMIN_THREAD_QUERY_KEY,
        thread_id,
    ))
}

pub(crate) fn comments_admin_locale_query_intent(locale: &str) -> UiRouteQueryIntent {
    match normalize_ui_text(locale) {
        Some(locale) => UiRouteQueryIntent::replace(COMMENTS_ADMIN_LOCALE_QUERY_KEY, locale),
        None => UiRouteQueryIntent::clear(COMMENTS_ADMIN_LOCALE_QUERY_KEY),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentThreadsRequest {
    pub page: u64,
    pub per_page: u64,
    pub target_type: String,
    pub thread_status: Option<CommentThreadStatus>,
    pub comment_status: Option<CommentStatus>,
}

impl CommentThreadsRequest {
    pub(crate) fn from_filters(
        page: u64,
        per_page: u64,
        target_type: String,
        thread_status_filter: &str,
        comment_status_filter: &str,
    ) -> Self {
        Self {
            page: page.max(1),
            per_page: per_page.max(1),
            target_type: target_type.trim().to_string(),
            thread_status: parse_thread_status(thread_status_filter),
            comment_status: parse_comment_status(comment_status_filter),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentThreadDetailRequest {
    pub thread_id: String,
    pub locale: String,
    pub page: u64,
    pub per_page: u64,
}

impl CommentThreadDetailRequest {
    pub(crate) fn new(thread_id: String, locale: String, page: u64, per_page: u64) -> Self {
        Self {
            thread_id,
            locale: locale.trim().to_string(),
            page: page.max(1),
            per_page: per_page.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetThreadStatusCommand {
    pub thread_id: String,
    pub status: CommentThreadStatus,
}

impl SetThreadStatusCommand {
    pub(crate) fn new(thread_id: String, status: CommentThreadStatus) -> Self {
        Self { thread_id, status }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetCommentStatusCommand {
    pub comment_id: String,
    pub status: CommentStatus,
    pub locale: String,
}

impl SetCommentStatusCommand {
    pub(crate) fn new(comment_id: String, status: CommentStatus, locale: String) -> Self {
        Self {
            comment_id,
            status,
            locale: locale.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentThreadListItemViewModel {
    pub id: String,
    pub target_label: String,
    pub status_label: &'static str,
    pub comment_count: i32,
}

impl CommentThreadListItemViewModel {
    pub(crate) fn from_summary(thread: &CommentThreadSummary) -> Self {
        Self {
            id: thread.id.to_string(),
            target_label: format_target_label(&thread.target_type, &thread.target_id.to_string()),
            status_label: thread_status_label(thread.status),
            comment_count: thread.comment_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentThreadDetailViewModel {
    pub target_label: String,
    pub status_label: &'static str,
    pub comment_count: i32,
}

impl CommentThreadDetailViewModel {
    pub(crate) fn from_detail(detail: &CommentThreadDetail) -> Self {
        Self {
            target_label: format_target_label(
                &detail.thread.target_type,
                &detail.thread.target_id.to_string(),
            ),
            status_label: thread_status_label(detail.thread.status),
            comment_count: detail.thread.comment_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentRowViewModel {
    pub id: String,
    pub author_id: String,
    pub created_at: String,
    pub body: String,
    pub requested_locale: String,
    pub effective_locale: String,
}

impl CommentRowViewModel {
    pub(crate) fn from_record(comment: &CommentRecord) -> Self {
        Self {
            id: comment.id.to_string(),
            author_id: comment.author_id.to_string(),
            created_at: comment.created_at.clone(),
            body: comment.body.clone(),
            requested_locale: comment.requested_locale.clone(),
            effective_locale: comment.effective_locale.clone(),
        }
    }
}

pub(crate) fn parse_thread_status(value: &str) -> Option<CommentThreadStatus> {
    match value.trim() {
        "open" => Some(CommentThreadStatus::Open),
        "closed" => Some(CommentThreadStatus::Closed),
        _ => None,
    }
}

pub(crate) fn parse_comment_status(value: &str) -> Option<CommentStatus> {
    match value.trim() {
        "pending" => Some(CommentStatus::Pending),
        "approved" => Some(CommentStatus::Approved),
        "spam" => Some(CommentStatus::Spam),
        "trash" => Some(CommentStatus::Trash),
        _ => None,
    }
}

fn thread_status_label(status: CommentThreadStatus) -> &'static str {
    match status {
        CommentThreadStatus::Open => "open",
        CommentThreadStatus::Closed => "closed",
    }
}

fn format_target_label(target_type: &str, target_id: &str) -> String {
    format!("{}:{}", target_type.trim(), target_id.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn builds_thread_route_query_push_intent() {
        let intent = comments_admin_select_thread_query_intent(" thread-1 ").unwrap();

        assert_eq!(
            intent,
            UiRouteQueryIntent::Push {
                key: COMMENTS_ADMIN_THREAD_QUERY_KEY,
                value: "thread-1".to_string(),
            }
        );
        assert_eq!(
            intent.into_write(),
            rustok_ui_core::UiRouteQueryWrite {
                updates: vec![(
                    COMMENTS_ADMIN_THREAD_QUERY_KEY,
                    Some("thread-1".to_string())
                )],
                replace: false,
            }
        );
        assert_eq!(comments_admin_select_thread_query_intent("   "), None);
    }

    #[test]
    fn builds_locale_route_query_replace_or_clear_intent() {
        let intent = comments_admin_locale_query_intent(" ru ");
        assert_eq!(
            intent,
            UiRouteQueryIntent::Replace {
                key: COMMENTS_ADMIN_LOCALE_QUERY_KEY,
                value: "ru".to_string(),
            }
        );
        assert_eq!(
            intent.into_write(),
            rustok_ui_core::UiRouteQueryWrite {
                updates: vec![(COMMENTS_ADMIN_LOCALE_QUERY_KEY, Some("ru".to_string()))],
                replace: true,
            }
        );

        let intent = comments_admin_locale_query_intent("   ");
        assert_eq!(
            intent,
            UiRouteQueryIntent::Clear {
                key: COMMENTS_ADMIN_LOCALE_QUERY_KEY,
            }
        );
        assert_eq!(
            intent.into_write(),
            rustok_ui_core::UiRouteQueryWrite {
                updates: vec![(COMMENTS_ADMIN_LOCALE_QUERY_KEY, None)],
                replace: true,
            }
        );
    }

    #[test]
    fn parses_thread_status_filters() {
        assert_eq!(parse_thread_status("open"), Some(CommentThreadStatus::Open));
        assert_eq!(
            parse_thread_status("closed"),
            Some(CommentThreadStatus::Closed)
        );
        assert_eq!(parse_thread_status(" all "), None);
        assert_eq!(parse_thread_status(""), None);
    }

    #[test]
    fn parses_comment_status_filters() {
        assert_eq!(
            parse_comment_status("pending"),
            Some(CommentStatus::Pending)
        );
        assert_eq!(
            parse_comment_status("approved"),
            Some(CommentStatus::Approved)
        );
        assert_eq!(parse_comment_status("spam"), Some(CommentStatus::Spam));
        assert_eq!(parse_comment_status("trash"), Some(CommentStatus::Trash));
        assert_eq!(parse_comment_status("all"), None);
    }

    #[test]
    fn builds_thread_list_request_from_ui_filters() {
        let request = CommentThreadsRequest::from_filters(
            0,
            0,
            " blog_post ".to_string(),
            "closed",
            "approved",
        );

        assert_eq!(request.page, 1);
        assert_eq!(request.per_page, 1);
        assert_eq!(request.target_type, "blog_post");
        assert_eq!(request.thread_status, Some(CommentThreadStatus::Closed));
        assert_eq!(request.comment_status, Some(CommentStatus::Approved));
    }

    #[test]
    fn normalizes_detail_request_pagination_and_locale() {
        let request =
            CommentThreadDetailRequest::new("thread-1".to_string(), " ru ".to_string(), 0, 0);

        assert_eq!(request.thread_id, "thread-1");
        assert_eq!(request.locale, "ru");
        assert_eq!(request.page, 1);
        assert_eq!(request.per_page, 1);
    }

    #[test]
    fn normalizes_comment_status_command_locale() {
        let command = SetCommentStatusCommand::new(
            "comment-1".to_string(),
            CommentStatus::Spam,
            " en ".to_string(),
        );

        assert_eq!(command.comment_id, "comment-1");
        assert_eq!(command.status, CommentStatus::Spam);
        assert_eq!(command.locale, "en");
    }

    #[test]
    fn maps_thread_summary_to_list_view_model() {
        let target_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let thread = thread_summary(target_id, CommentThreadStatus::Closed, 7);

        let view_model = CommentThreadListItemViewModel::from_summary(&thread);

        assert_eq!(view_model.id, thread.id.to_string());
        assert_eq!(
            view_model.target_label,
            "blog_post:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
        );
        assert_eq!(view_model.status_label, "closed");
        assert_eq!(view_model.comment_count, 7);
    }

    #[test]
    fn maps_detail_summary_to_view_model() {
        let target_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
        let detail = CommentThreadDetail {
            thread: thread_summary(target_id, CommentThreadStatus::Open, 2),
            comments: vec![],
            total_comments: 2,
        };

        let view_model = CommentThreadDetailViewModel::from_detail(&detail);

        assert_eq!(
            view_model.target_label,
            "blog_post:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
        );
        assert_eq!(view_model.status_label, "open");
        assert_eq!(view_model.comment_count, 2);
    }

    #[test]
    fn maps_comment_record_to_row_view_model() {
        let comment = comment_record();

        let view_model = CommentRowViewModel::from_record(&comment);

        assert_eq!(view_model.id, comment.id.to_string());
        assert_eq!(view_model.author_id, comment.author_id.to_string());
        assert_eq!(view_model.created_at, "2026-06-07T00:00:00Z");
        assert_eq!(view_model.body, "Moderation body");
        assert_eq!(view_model.requested_locale, "ru");
        assert_eq!(view_model.effective_locale, "ru");
    }

    fn thread_summary(
        target_id: Uuid,
        status: CommentThreadStatus,
        comment_count: i32,
    ) -> CommentThreadSummary {
        CommentThreadSummary {
            id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            tenant_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            target_type: "blog_post".to_string(),
            target_id,
            status,
            comment_count,
            last_commented_at: Some("2026-06-07T00:00:00Z".to_string()),
            created_at: "2026-06-07T00:00:00Z".to_string(),
            updated_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }

    fn comment_record() -> CommentRecord {
        CommentRecord {
            id: Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            thread_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            target_type: "blog_post".to_string(),
            target_id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            requested_locale: "ru".to_string(),
            effective_locale: "ru".to_string(),
            author_id: Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
            parent_comment_id: None,
            body: "Moderation body".to_string(),
            body_format: "plain_text".to_string(),
            status: CommentStatus::Pending,
            position: 1,
            created_at: "2026-06-07T00:00:00Z".to_string(),
            updated_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }
}
