//! Framework-independent discussion ordering and exact producer identity.
use crate::model::{
    BlogCommentList, BlogCommentListItem, BlogCommentsAvailability, BlogDiscussionProps,
};
use rustok_reactions_storefront::ReactionSubjectUiRef;
use std::collections::{HashMap, HashSet};

pub fn reaction_subject(props: &BlogDiscussionProps) -> Result<ReactionSubjectUiRef, String> {
    ReactionSubjectUiRef::new(
        "blog",
        "post",
        props
            .post_id
            .parse::<uuid::Uuid>()
            .map_err(|_| "Invalid Blog post identity")?,
        props.version.to_string(),
    )
}

/// Parent-first stable ordering, bounded indentation and cycle-safe orphan handling.
pub fn order_comments(items: Vec<BlogCommentListItem>) -> Vec<(BlogCommentListItem, usize)> {
    let known: HashSet<_> = items.iter().map(|item| item.id.clone()).collect();
    let mut children: HashMap<Option<String>, Vec<BlogCommentListItem>> = HashMap::new();
    for item in &items {
        let parent = item
            .parent_comment_id
            .as_ref()
            .filter(|id| known.contains(*id) && *id != &item.id)
            .cloned();
        children.entry(parent).or_default().push(item.clone());
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut pending = children
        .get(&None)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|item| (item, 0usize))
        .collect::<Vec<_>>();
    let mut remaining = items.into_iter();
    while let Some((item, depth)) = pending.pop().or_else(|| {
        remaining
            .find(|item| !seen.contains(&item.id))
            .map(|item| (item, 0))
    }) {
        if !seen.insert(item.id.clone()) {
            continue;
        }
        if let Some(replies) = children.get(&Some(item.id.clone())) {
            pending.extend(
                replies
                    .iter()
                    .rev()
                    .cloned()
                    .map(|item| (item, depth.saturating_add(1))),
            );
        }
        result.push((item, depth.min(3)));
    }
    result
}

pub fn comment_indent(depth: usize) -> &'static str {
    ["ml-0", "ml-4", "ml-8", "ml-12"][depth.min(3)]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn comment(id: &str, parent: Option<&str>) -> BlogCommentListItem {
        BlogCommentListItem {
            id: id.into(),
            parent_comment_id: parent.map(str::to_owned),
            effective_locale: "en".into(),
            author_id: None,
            content_preview: id.into(),
            created_at: "2026-11-05T00:00:00Z".into(),
        }
    }
    #[test]
    fn discussion_keeps_orphans_cycles_and_deep_threads_visible_once() {
        let ordered = order_comments(vec![
            comment("child", Some("parent")),
            comment("parent", None),
            comment("orphan", Some("missing")),
            comment("a", Some("b")),
            comment("b", Some("a")),
        ]);
        assert_eq!(
            ordered
                .iter()
                .map(|(item, depth)| (item.id.as_str(), *depth))
                .collect::<Vec<_>>(),
            [
                ("parent", 0),
                ("child", 1),
                ("orphan", 0),
                ("a", 0),
                ("b", 1)
            ]
        );
        let mut deep = vec![comment("0", None)];
        for index in 1..10000 {
            deep.push(comment(&index.to_string(), Some(&(index - 1).to_string())));
        }
        let ordered = order_comments(deep);
        assert_eq!(ordered.len(), 10000);
        assert_eq!(ordered.last().unwrap().1, 3);
    }
    #[test]
    fn reaction_identity_never_guesses_an_owner_revision() {
        let mut props = BlogDiscussionProps {
            post_id: uuid::Uuid::new_v4().to_string(),
            version: 42,
            content_locale: "ru".into(),
            ui_locale: Some("fr".into()),
            comments: crate::model::BlogCommentList {
                availability: crate::model::BlogCommentsAvailability::Available,
                cached_snapshot: false,
                items: vec![],
                total: 0,
            },
            comments_page: 1,
        };
        assert_eq!(reaction_subject(&props).unwrap().subject_revision, "42");
        props.version = 0;
        assert!(reaction_subject(&props).is_err());
        props.version = 42;
        props.post_id = uuid::Uuid::nil().to_string();
        assert!(reaction_subject(&props).is_err());
    }
}

pub fn prepare_comment(
    post_id: &str,
    locale: &str,
    parent: Option<String>,
    content: rustok_api::RichTextDocument,
) -> crate::model::BlogCommentCreateRequest {
    let mut request = crate::model::BlogCommentCreateRequest::for_post(
        post_id.to_string(),
        locale.to_string(),
        content,
    );
    request.parent_comment_id = parent;
    request
}

/// Availability and paging decisions belong to the pure owner policy. Cached
/// comments may be shown during an outage, but cannot enable reply commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentsView {
    pub show_section: bool,
    pub show_snapshot: bool,
    pub can_reply: bool,
    pub degraded_message: Option<(&'static str, &'static str)>,
    pub page: u64,
    pub total_pages: u64,
    pub previous_page: u64,
    pub next_page: u64,
    pub can_previous: bool,
    pub can_next: bool,
}

pub fn comments_view(comments: &BlogCommentList, requested_page: u64) -> CommentsView {
    use BlogCommentsAvailability::*;
    let degraded_message = match (comments.availability, comments.cached_snapshot) {
        (Available | Disabled, _) => None,
        (ReadOnly, _) => Some((
            "blog.comments.readOnly",
            "Comments are closed for new replies.",
        )),
        (Unavailable, true) => Some((
            "blog.comments.unavailableCached",
            "Comments are temporarily unavailable. Showing a recent cached snapshot.",
        )),
        (Unavailable, false) => Some((
            "blog.comments.unavailable",
            "Comments are temporarily unavailable. The article is still available.",
        )),
        (Timeout, true) => Some((
            "blog.comments.timeoutCached",
            "Comments took too long to load. Showing a recent cached snapshot.",
        )),
        (Timeout, false) => Some((
            "blog.comments.timeout",
            "Comments took too long to load. The article is still available.",
        )),
    };
    let total_pages = super::comments_pagination::comments_total_pages(comments.total);
    let page = super::comments_pagination::bounded_comments_page(requested_page, comments.total);
    CommentsView {
        show_section: comments.availability != Disabled,
        show_snapshot: !matches!(comments.availability, Disabled)
            && (!matches!(comments.availability, Unavailable | Timeout)
                || comments.cached_snapshot),
        can_reply: comments.availability == Available,
        degraded_message,
        page,
        total_pages,
        previous_page: page.saturating_sub(1).max(1),
        next_page: page.saturating_add(1).min(total_pages),
        can_previous: page > 1,
        can_next: page < total_pages,
    }
}

#[cfg(test)]
mod availability_tests {
    use super::*;

    #[test]
    fn outages_never_enable_writes_and_only_render_a_real_cached_snapshot() {
        for availability in [
            BlogCommentsAvailability::Unavailable,
            BlogCommentsAvailability::Timeout,
        ] {
            for cached_snapshot in [false, true] {
                let view = comments_view(
                    &BlogCommentList {
                        availability,
                        cached_snapshot,
                        items: vec![],
                        total: 0,
                    },
                    u64::MAX,
                );
                assert!(!view.can_reply);
                assert_eq!(view.show_snapshot, cached_snapshot);
                assert!(view.degraded_message.is_some());
                assert_eq!((view.page, view.previous_page, view.next_page), (1, 1, 1));
            }
        }
        let disabled = comments_view(
            &BlogCommentList {
                availability: BlogCommentsAvailability::Disabled,
                ..Default::default()
            },
            1,
        );
        assert!(!disabled.show_section);
        assert!(!disabled.show_snapshot);
        assert!(!disabled.can_reply);
        let read_only = comments_view(
            &BlogCommentList {
                availability: BlogCommentsAvailability::ReadOnly,
                ..Default::default()
            },
            1,
        );
        assert!(read_only.show_snapshot);
        assert!(!read_only.can_reply);
        assert!(comments_view(&BlogCommentList::default(), 1).can_reply);
    }

    #[test]
    fn reply_command_keeps_the_exact_post_content_locale_and_parent() {
        let content = rustok_api::RichTextDocument::single_paragraph("Reply");
        let request = prepare_comment("post", "fr-CA", Some("parent".into()), content);
        assert_eq!(request.post_id, "post");
        assert_eq!(request.locale, "fr-CA");
        assert_eq!(request.parent_comment_id.as_deref(), Some("parent"));
        assert!(uuid::Uuid::parse_str(&request.command_id).is_ok());
    }
}
