use rustok_api::RichTextDocument;

use crate::core::{CategoryFormSnapshot, TopicFormSnapshot};

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
    }
}
