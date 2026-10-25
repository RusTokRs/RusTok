use crate::i18n;
use crate::model::FormSubmission;

/// Localized labels for the forms admin surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormsAdminLabels {
    pub title: String,
    pub subtitle: String,
    pub empty: String,
}

impl FormsAdminLabels {
    pub fn from_locale(locale: Option<&str>) -> Self {
        Self {
            title: i18n::t(locale, "forms_title", "Form submissions"),
            subtitle: i18n::t(
                locale,
                "forms_subtitle",
                "newest first; honeypot captures are stored as spam",
            ),
            empty: i18n::t(locale, "forms_empty", "No submissions yet"),
        }
    }
}

/// One actionable triage transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionAction {
    pub target_state: &'static str,
    pub label: String,
}

/// Resolves allowed next transitions according to the triage state machine.
pub fn allowed_transitions(state: &str, locale: Option<&str>) -> Vec<SubmissionAction> {
    match state {
        "new" => vec![
            SubmissionAction {
                target_state: "read",
                label: i18n::t(locale, "forms_action_mark_read", "Mark read"),
            },
            SubmissionAction {
                target_state: "handled",
                label: i18n::t(locale, "forms_action_mark_handled", "Mark handled"),
            },
            SubmissionAction {
                target_state: "spam",
                label: i18n::t(locale, "forms_action_mark_spam", "Mark spam"),
            },
        ],
        "read" => vec![
            SubmissionAction {
                target_state: "handled",
                label: i18n::t(locale, "forms_action_mark_handled", "Mark handled"),
            },
            SubmissionAction {
                target_state: "spam",
                label: i18n::t(locale, "forms_action_mark_spam", "Mark spam"),
            },
        ],
        "handled" => vec![SubmissionAction {
            target_state: "read",
            label: i18n::t(locale, "forms_action_mark_read", "Mark read"),
        }],
        "spam" => vec![SubmissionAction {
            target_state: "read",
            label: i18n::t(locale, "forms_action_mark_read", "Mark read"),
        }],
        _ => Vec::new(),
    }
}

/// Resolves human-readable, localized state label.
pub fn state_label(state: &str, locale: Option<&str>) -> String {
    match state {
        "new" => i18n::t(locale, "forms_state_new", "New"),
        "read" => i18n::t(locale, "forms_state_read", "Read"),
        "handled" => i18n::t(locale, "forms_state_handled", "Handled"),
        "spam" => i18n::t(locale, "forms_state_spam", "Spam"),
        other => other.to_string(),
    }
}

/// Formats a human-readable summary of the submission payload.
pub fn summarize_submission(row: &FormSubmission) -> String {
    let mut parts = Vec::new();
    if let Some(page_id) = row.page_id {
        parts.push(format!("page {page_id}"));
    }
    if let Some(value) = row.payload.as_object() {
        for (name, field) in value.iter().take(6) {
            let text = match field {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            parts.push(format!(
                "{name}={}",
                text.chars().take(40).collect::<String>()
            ));
        }
    }
    if parts.is_empty() {
        "(no fields)".to_string()
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn labels_load_en_and_ru() {
        let en = FormsAdminLabels::from_locale(Some("en"));
        assert_eq!(en.title, "Form submissions");
        assert_eq!(en.empty, "No submissions yet");

        let ru = FormsAdminLabels::from_locale(Some("ru"));
        assert_eq!(ru.title, "Отправленные формы");
        assert_eq!(ru.empty, "Нет отправленных форм");
    }

    #[test]
    fn state_labels_localized() {
        assert_eq!(state_label("new", Some("en")), "New");
        assert_eq!(state_label("new", Some("ru")), "Новая");
        assert_eq!(state_label("read", Some("en")), "Read");
        assert_eq!(state_label("read", Some("ru")), "Прочитано");
        assert_eq!(state_label("handled", Some("en")), "Handled");
        assert_eq!(state_label("handled", Some("ru")), "Обработано");
        assert_eq!(state_label("spam", Some("en")), "Spam");
        assert_eq!(state_label("spam", Some("ru")), "Спам");
        assert_eq!(state_label("custom", Some("en")), "custom");
    }

    #[test]
    fn allowed_transitions_match_state_machine() {
        let new_actions = allowed_transitions("new", Some("en"));
        assert_eq!(new_actions.len(), 3);
        assert_eq!(new_actions[0].target_state, "read");
        assert_eq!(new_actions[1].target_state, "handled");
        assert_eq!(new_actions[2].target_state, "spam");

        let read_actions = allowed_transitions("read", Some("en"));
        assert_eq!(read_actions.len(), 2);
        assert_eq!(read_actions[0].target_state, "handled");
        assert_eq!(read_actions[1].target_state, "spam");

        let handled_actions = allowed_transitions("handled", Some("en"));
        assert_eq!(handled_actions.len(), 1);
        assert_eq!(handled_actions[0].target_state, "read");

        let spam_actions = allowed_transitions("spam", Some("en"));
        assert_eq!(spam_actions.len(), 1);
        assert_eq!(spam_actions[0].target_state, "read");

        let unknown_actions = allowed_transitions("unknown", Some("en"));
        assert!(unknown_actions.is_empty());
    }

    #[test]
    fn summarize_submission_formatting() {
        let submission = FormSubmission {
            id: Uuid::new_v4(),
            form_id: "contact".to_string(),
            locale: "en".to_string(),
            page_id: None,
            payload: json!({ "email": "test@example.com", "name": "Alice" }),
            state: "new".to_string(),
            created_at: "2026-10-23".to_string(),
            handled_at: None,
            handled_by: None,
        };
        let summary = summarize_submission(&submission);
        assert!(summary.contains("email=test@example.com"));
        assert!(summary.contains("name=Alice"));

        let empty = FormSubmission {
            id: Uuid::new_v4(),
            form_id: "feedback".to_string(),
            locale: "en".to_string(),
            page_id: None,
            payload: json!({}),
            state: "read".to_string(),
            created_at: "2026-10-23".to_string(),
            handled_at: None,
            handled_by: None,
        };
        assert_eq!(summarize_submission(&empty), "(no fields)");
    }
}
