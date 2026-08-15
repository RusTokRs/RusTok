use rustok_api::RichTextDocument;

use crate::core::{CategoryFormSnapshot, TopicFormSnapshot};
use crate::model::{CategoryDetail, TopicDetail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForumAdminLocaleSwitchDecision {
    Noop { locale: String },
    Reload { locale: String },
    BlockedDirty,
    Invalid,
}

pub fn category_locale_switch_decision(
    current: &CategoryFormSnapshot,
    persisted: Option<&CategoryFormSnapshot>,
    requested_locale: &str,
) -> ForumAdminLocaleSwitchDecision {
    locale_switch_decision(
        current.locale.as_str(),
        requested_locale,
        match persisted {
            Some(persisted) => category_editable_state_matches(current, persisted),
            None => category_localized_fields_are_pristine(current),
        },
    )
}

pub fn topic_locale_switch_decision(
    current: &TopicFormSnapshot,
    persisted: Option<&TopicFormSnapshot>,
    requested_locale: &str,
) -> ForumAdminLocaleSwitchDecision {
    locale_switch_decision(
        current.locale.as_str(),
        requested_locale,
        match persisted {
            Some(persisted) => topic_editable_state_matches(current, persisted),
            None => topic_localized_fields_are_pristine(current),
        },
    )
}

pub fn category_detail_for_editor(mut detail: CategoryDetail) -> CategoryDetail {
    if detail_is_fallback(detail.requested_locale.as_str(), detail.effective_locale.as_str()) {
        detail.name.clear();
        detail.slug.clear();
        detail.description = None;
    }
    detail
}

pub fn topic_detail_for_editor(mut detail: TopicDetail) -> TopicDetail {
    if detail_is_fallback(detail.requested_locale.as_str(), detail.effective_locale.as_str()) {
        detail.title.clear();
        detail.slug.clear();
        detail.body.document = RichTextDocument::empty();
        detail.body.html.clear();
        detail.body_plain_text.clear();
        detail.tags.clear();
    }
    detail
}

pub fn topic_tags_for_update(
    current: &TopicDetail,
    candidate_tags: Vec<String>,
) -> Option<Vec<String>> {
    let candidate_tags = normalize_tag_labels(candidate_tags);
    let current_tags = normalize_tag_labels(current.tags.clone());
    if candidate_tags == current_tags
        || (detail_is_fallback(
            current.requested_locale.as_str(),
            current.effective_locale.as_str(),
        ) && candidate_tags.is_empty())
    {
        None
    } else {
        Some(candidate_tags)
    }
}

pub fn category_target_form(detail: &CategoryDetail) -> CategoryFormSnapshot {
    CategoryFormSnapshot::from_detail(&category_detail_for_editor(detail.clone()))
}

pub fn topic_target_form(detail: &TopicDetail) -> TopicFormSnapshot {
    TopicFormSnapshot::from_detail(&topic_detail_for_editor(detail.clone()))
}

pub fn locale_candidate_matches_active(candidate: &str, active: &str) -> bool {
    candidate.trim().eq_ignore_ascii_case(active.trim())
}

fn detail_is_fallback(requested_locale: &str, effective_locale: &str) -> bool {
    !effective_locale.eq_ignore_ascii_case(requested_locale)
}

fn normalize_tag_labels(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn locale_switch_decision(
    current_locale: &str,
    requested_locale: &str,
    is_clean: bool,
) -> ForumAdminLocaleSwitchDecision {
    let requested_locale = requested_locale.trim();
    if requested_locale.is_empty() {
        return ForumAdminLocaleSwitchDecision::Invalid;
    }
    if requested_locale.eq_ignore_ascii_case(current_locale.trim()) {
        return ForumAdminLocaleSwitchDecision::Noop {
            locale: current_locale.trim().to_string(),
        };
    }
    if !is_clean {
        return ForumAdminLocaleSwitchDecision::BlockedDirty;
    }
    ForumAdminLocaleSwitchDecision::Reload {
        locale: requested_locale.to_string(),
    }
}

fn category_editable_state_matches(
    current: &CategoryFormSnapshot,
    persisted: &CategoryFormSnapshot,
) -> bool {
    current.editing_id == persisted.editing_id
        && current.name == persisted.name
        && current.slug == persisted.slug
        && current.description == persisted.description
        && current.icon == persisted.icon
        && current.color == persisted.color
        && current.position == persisted.position
        && current.moderated == persisted.moderated
}

fn topic_editable_state_matches(current: &TopicFormSnapshot, persisted: &TopicFormSnapshot) -> bool {
    current.editing_id == persisted.editing_id
        && current.category_id == persisted.category_id
        && current.title == persisted.title
        && current.slug == persisted.slug
        && current.body == persisted.body
        && current.tags_raw == persisted.tags_raw
}

fn category_localized_fields_are_pristine(current: &CategoryFormSnapshot) -> bool {
    current.editing_id.is_none()
        && current.name.trim().is_empty()
        && current.slug.trim().is_empty()
        && current.description.trim().is_empty()
}

fn topic_localized_fields_are_pristine(current: &TopicFormSnapshot) -> bool {
    current.editing_id.is_none()
        && current.title.trim().is_empty()
        && current.slug.trim().is_empty()
        && current.tags_raw.trim().is_empty()
        && current.body == RichTextDocument::empty()
}

#[cfg(test)]
mod tests {
    use rustok_api::RichTextView;

    use super::*;

    fn category(locale: &str) -> CategoryFormSnapshot {
        CategoryFormSnapshot {
            editing_id: Some("category-1".to_string()),
            locale: locale.to_string(),
            name: "General".to_string(),
            slug: "general".to_string(),
            description: "General discussion".to_string(),
            icon: "chat".to_string(),
            color: "#fff".to_string(),
            position: 1,
            moderated: false,
        }
    }

    fn topic(locale: &str) -> TopicFormSnapshot {
        TopicFormSnapshot {
            editing_id: Some("topic-1".to_string()),
            locale: locale.to_string(),
            category_id: "category-1".to_string(),
            title: "Welcome".to_string(),
            slug: "welcome".to_string(),
            body: RichTextDocument::single_paragraph("Hello"),
            tags_raw: "intro".to_string(),
        }
    }

    fn topic_detail(requested_locale: &str, effective_locale: &str) -> TopicDetail {
        TopicDetail {
            id: "topic-1".to_string(),
            requested_locale: requested_locale.to_string(),
            locale: requested_locale.to_string(),
            effective_locale: effective_locale.to_string(),
            available_locales: vec![effective_locale.to_string()],
            category_id: "category-1".to_string(),
            author_id: None,
            title: "Welcome".to_string(),
            slug: "welcome".to_string(),
            body: RichTextView {
                document: RichTextDocument::single_paragraph("Hello"),
                html: "<p>Hello</p>".to_string(),
            },
            body_plain_text: "Hello".to_string(),
            status: "published".to_string(),
            tags: vec!["intro".to_string(), "news".to_string()],
            is_pinned: false,
            is_locked: false,
            reply_count: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn clean_existing_category_reloads_target_locale() {
        let current = category("en");
        assert_eq!(
            category_locale_switch_decision(&current, Some(&current), " ru "),
            ForumAdminLocaleSwitchDecision::Reload {
                locale: "ru".to_string()
            }
        );
    }

    #[test]
    fn dirty_existing_category_blocks_locale_switch() {
        let persisted = category("en");
        let mut current = persisted.clone();
        current.name = "Unsaved title".to_string();
        assert_eq!(
            category_locale_switch_decision(&current, Some(&persisted), "ru"),
            ForumAdminLocaleSwitchDecision::BlockedDirty
        );
    }

    #[test]
    fn new_category_blocks_switch_after_localized_content_is_entered() {
        let mut current = CategoryFormSnapshot {
            editing_id: None,
            locale: "en".to_string(),
            name: String::new(),
            slug: String::new(),
            description: String::new(),
            icon: "chat".to_string(),
            color: "#fff".to_string(),
            position: 3,
            moderated: true,
        };
        assert!(matches!(
            category_locale_switch_decision(&current, None, "ru"),
            ForumAdminLocaleSwitchDecision::Reload { .. }
        ));
        current.name = "Draft".to_string();
        assert_eq!(
            category_locale_switch_decision(&current, None, "ru"),
            ForumAdminLocaleSwitchDecision::BlockedDirty
        );
    }

    #[test]
    fn dirty_existing_topic_blocks_locale_switch() {
        let persisted = topic("en");
        let mut current = persisted.clone();
        current.body = RichTextDocument::single_paragraph("Unsaved body");
        assert_eq!(
            topic_locale_switch_decision(&current, Some(&persisted), "ar"),
            ForumAdminLocaleSwitchDecision::BlockedDirty
        );
    }

    #[test]
    fn new_topic_blocks_switch_after_localized_content_is_entered() {
        let mut current = TopicFormSnapshot {
            editing_id: None,
            locale: "en".to_string(),
            category_id: "category-1".to_string(),
            title: String::new(),
            slug: String::new(),
            body: RichTextDocument::empty(),
            tags_raw: String::new(),
        };
        assert!(matches!(
            topic_locale_switch_decision(&current, None, "ar"),
            ForumAdminLocaleSwitchDecision::Reload { .. }
        ));
        current.title = "Draft".to_string();
        assert_eq!(
            topic_locale_switch_decision(&current, None, "ar"),
            ForumAdminLocaleSwitchDecision::BlockedDirty
        );
    }

    #[test]
    fn locale_candidate_is_trimmed_and_same_locale_is_a_noop() {
        let current = category("en");
        assert_eq!(
            category_locale_switch_decision(&current, Some(&current), " EN "),
            ForumAdminLocaleSwitchDecision::Noop {
                locale: "en".to_string()
            }
        );
        assert_eq!(
            category_locale_switch_decision(&current, Some(&current), "   "),
            ForumAdminLocaleSwitchDecision::Invalid
        );
        assert!(locale_candidate_matches_active(" EN ", "en"));
        assert!(!locale_candidate_matches_active("de", "en"));
    }

    #[test]
    fn fallback_category_detail_is_safe_for_initial_editor_load() {
        let detail = CategoryDetail {
            id: "category-1".to_string(),
            requested_locale: "ru".to_string(),
            locale: "ru".to_string(),
            effective_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            name: "General".to_string(),
            slug: "general".to_string(),
            description: Some("English fallback".to_string()),
            icon: Some("chat".to_string()),
            color: Some("#FFFFFF".to_string()),
            parent_id: None,
            position: 4,
            topic_count: 2,
            reply_count: 3,
            moderated: true,
        };
        let detail = category_detail_for_editor(detail);
        assert_eq!(detail.locale, "ru");
        assert!(detail.name.is_empty());
        assert!(detail.slug.is_empty());
        assert_eq!(detail.description, None);
        assert_eq!(detail.icon.as_deref(), Some("chat"));
        assert_eq!(detail.position, 4);
        assert!(detail.moderated);
    }

    #[test]
    fn fallback_topic_detail_clears_all_locale_labels_before_any_editor_write() {
        let detail = topic_detail("ar", "en");
        let detail = topic_detail_for_editor(detail);
        assert_eq!(detail.locale, "ar");
        assert_eq!(detail.category_id, "category-1");
        assert!(detail.title.is_empty());
        assert!(detail.slug.is_empty());
        assert_eq!(detail.body.document, RichTextDocument::empty());
        assert!(detail.body.html.is_empty());
        assert!(detail.body_plain_text.is_empty());
        assert!(detail.tags.is_empty());
    }

    #[test]
    fn exact_locale_detail_is_not_scrubbed() {
        let detail = CategoryDetail {
            id: "category-1".to_string(),
            requested_locale: "en".to_string(),
            locale: "en".to_string(),
            effective_locale: "en".to_string(),
            available_locales: vec!["en".to_string()],
            name: "General".to_string(),
            slug: "general".to_string(),
            description: None,
            icon: None,
            color: None,
            parent_id: None,
            position: 0,
            topic_count: 0,
            reply_count: 0,
            moderated: false,
        };
        assert_eq!(category_detail_for_editor(detail.clone()).name, detail.name);
    }

    #[test]
    fn unchanged_exact_locale_tags_do_not_trigger_attachment_resync() {
        let detail = topic_detail("en", "en");
        assert_eq!(
            topic_tags_for_update(
                &detail,
                vec![" intro ".to_string(), "news".to_string()]
            ),
            None
        );
    }

    #[test]
    fn scrubbed_fallback_tags_preserve_existing_attachment_identity() {
        let detail = topic_detail("ar", "en");
        assert_eq!(topic_tags_for_update(&detail, Vec::new()), None);
    }

    #[test]
    fn explicit_tag_change_is_forwarded_for_owner_sync() {
        let detail = topic_detail("en", "en");
        assert_eq!(
            topic_tags_for_update(&detail, vec!["arabic".to_string()]),
            Some(vec!["arabic".to_string()])
        );
        assert_eq!(
            topic_tags_for_update(&detail, Vec::new()),
            Some(Vec::new())
        );
    }
}
