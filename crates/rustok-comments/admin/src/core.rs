//! Framework-agnostic helpers for the comments admin UI.
//!
//! This layer owns request/view policy that can be reused by future host adapters
//! without depending on framework runtime types.

use rustok_comments::{CommentStatus, CommentThreadStatus};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
