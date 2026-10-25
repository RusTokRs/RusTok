//! Contract tests verifying the architectural invariants and public API of `rustok-forms-admin`.

use crate::core::{FormsAdminLabels, allowed_transitions, state_label, summarize_submission};
use crate::model::FormSubmission;
use serde_json::json;
use uuid::Uuid;

const CRATE_API_DOC: &str = include_str!("../CRATE_API.md");

#[test]
fn crate_api_markdown_contains_all_mandatory_contract_sections() {
    let required_sections = [
        "## Minimum Contract Set",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Public surface",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ];

    for section in required_sections {
        assert!(
            CRATE_API_DOC.contains(section),
            "CRATE_API.md must contain mandatory section: `{section}`"
        );
    }
}

#[test]
fn allowed_transitions_strictly_enforces_triage_graph() {
    // New can transition to Read, Handled, or Spam
    let new_actions = allowed_transitions("new", None);
    let new_targets: Vec<&str> = new_actions.iter().map(|a| a.target_state).collect();
    assert_eq!(new_targets, vec!["read", "handled", "spam"]);

    // Read can transition to Handled or Spam
    let read_actions = allowed_transitions("read", None);
    let read_targets: Vec<&str> = read_actions.iter().map(|a| a.target_state).collect();
    assert_eq!(read_targets, vec!["handled", "spam"]);

    // Handled can only transition to Read
    let handled_actions = allowed_transitions("handled", None);
    let handled_targets: Vec<&str> = handled_actions.iter().map(|a| a.target_state).collect();
    assert_eq!(handled_targets, vec!["read"]);

    // Spam can only transition to Read
    let spam_actions = allowed_transitions("spam", None);
    let spam_targets: Vec<&str> = spam_actions.iter().map(|a| a.target_state).collect();
    assert_eq!(spam_targets, vec!["read"]);

    // Any invalid/unknown state has no allowed transitions
    assert!(allowed_transitions("invalid", None).is_empty());
}

#[test]
fn forms_admin_labels_provide_multilingual_coverage() {
    let en = FormsAdminLabels::from_locale(Some("en"));
    assert!(!en.title.is_empty());
    assert!(!en.subtitle.is_empty());
    assert!(!en.empty.is_empty());

    let ru = FormsAdminLabels::from_locale(Some("ru"));
    assert!(!ru.title.is_empty());
    assert!(!ru.subtitle.is_empty());
    assert!(!ru.empty.is_empty());
}

#[test]
fn summarize_submission_handles_empty_and_populated_payloads() {
    let empty_row = FormSubmission {
        id: Uuid::new_v4(),
        form_id: "test".to_string(),
        locale: "en".to_string(),
        page_id: None,
        payload: json!({}),
        state: "new".to_string(),
        created_at: "2026-10-25".to_string(),
        handled_at: None,
        handled_by: None,
    };
    assert_eq!(summarize_submission(&empty_row), "(no fields)");

    let populated = FormSubmission {
        id: Uuid::new_v4(),
        form_id: "contact".to_string(),
        locale: "en".to_string(),
        page_id: Some(Uuid::new_v4()),
        payload: json!({ "name": "Antigravity", "email": "ai@google.com" }),
        state: "new".to_string(),
        created_at: "2026-10-25".to_string(),
        handled_at: None,
        handled_by: None,
    };
    let summary = summarize_submission(&populated);
    assert!(summary.contains("page"));
    assert!(summary.contains("name=Antigravity"));
    assert!(summary.contains("email=ai@google.com"));
}

#[test]
fn state_label_formatting() {
    assert_eq!(state_label("new", Some("en")), "New");
    assert_eq!(state_label("read", Some("en")), "Read");
    assert_eq!(state_label("handled", Some("en")), "Handled");
    assert_eq!(state_label("spam", Some("en")), "Spam");
}
