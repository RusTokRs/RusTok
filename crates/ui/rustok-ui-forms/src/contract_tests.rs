use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-ui-forms");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_form_lifecycle_state_machine_contract() {
    let mut state = FormState::idle();
    assert!(!state.is_submitting);
    assert!(!state.is_success());
    assert!(!state.has_errors());
    assert!(state.form_error.is_none());

    // Transition to submitting
    state.set_submitting();
    assert!(state.is_submitting);
    assert!(!state.is_success());

    // Transition to failure with error
    state.set_submitted_failure("Network timeout");
    assert!(!state.is_submitting);
    assert!(!state.is_success());
    assert!(state.has_errors());
    assert_eq!(state.form_error.as_deref(), Some("Network timeout"));

    // Add field error
    state.add_field_error("email", "Already registered");
    assert_eq!(state.field_error("email"), Some("Already registered"));

    // Clear specific field error
    state.clear_field_error("email");
    assert_eq!(state.field_error("email"), None);

    // Transition to success
    state.set_submitted_success();
    assert!(!state.is_submitting);
    assert!(state.is_success());
    assert!(!state.has_errors());
    assert!(state.form_error.is_none());
}

#[test]
fn test_schema_builder_and_json_validation_contract() {
    let schema = FormSchema::new("user_registration")
        .with_field(
            FieldDescriptor::new("username", FieldKind::Text)
                .with_required(true)
                .with_min_length(3)
                .with_max_length(20),
        )
        .with_field(FieldDescriptor::new("email", FieldKind::Email).with_required(true))
        .with_field(FieldDescriptor::new("age", FieldKind::Number).with_min(18.0));

    let invalid_json = serde_json::json!({
        "username": "al",
        "email": "not-an-email",
        "age": 16
    });

    let errors = schema.validate_json(&invalid_json);
    assert!(errors.len() >= 3);
    assert!(errors.iter().any(|e| e.field == "username"));
    assert!(errors.iter().any(|e| e.field == "email"));
    assert!(errors.iter().any(|e| e.field == "age"));

    let valid_json = serde_json::json!({
        "username": "alex_rustok",
        "email": "alex@rustok.org",
        "age": 25
    });

    let valid_errors = schema.validate_json(&valid_json);
    assert!(valid_errors.is_empty());
}

#[test]
fn test_dirty_tracker_contract() {
    let mut tracker = DirtyTracker::new();
    assert!(!tracker.is_any_dirty());
    assert!(!tracker.is_dirty("title"));

    tracker.mark("title");
    assert!(tracker.is_any_dirty());
    assert!(tracker.is_dirty("title"));
    assert!(!tracker.is_dirty("content"));

    tracker.record_change("content", true);
    assert!(tracker.is_dirty("content"));

    // Recording unchanged removes field from dirty set
    tracker.record_change("content", false);
    assert!(!tracker.is_dirty("content"));

    tracker.reset();
    assert!(!tracker.is_any_dirty());
}

#[test]
fn test_step_tracker_lifecycle_contract() {
    let mut wizard = FormStepTracker::new(vec!["account", "profile", "confirmation"]);
    assert_eq!(wizard.current_step_name(), Some("account"));
    assert_eq!(wizard.current_step_index(), 0);
    assert_eq!(wizard.total_steps(), 3);
    assert!(!wizard.is_last_step());

    assert!(wizard.next_step());
    assert_eq!(wizard.current_step_name(), Some("profile"));
    assert_eq!(wizard.current_step_index(), 1);

    assert!(wizard.next_step());
    assert_eq!(wizard.current_step_name(), Some("confirmation"));
    assert!(wizard.is_last_step());

    // Advancing past last step returns false
    assert!(!wizard.next_step());
    assert_eq!(wizard.current_step_index(), 2);

    assert!(wizard.prev_step());
    assert_eq!(wizard.current_step_name(), Some("profile"));
}

#[test]
fn test_form_sanitizer_contract() {
    assert_eq!(form_sanitizer::trim("  hello world  \n"), "hello world");
    assert_eq!(
        form_sanitizer::collapse_whitespace("hello   \t\n  world"),
        "hello world"
    );
    assert_eq!(
        form_sanitizer::slugify("  RusToK UI & Forms 2026!  "),
        "rustok-ui-forms-2026"
    );
    assert_eq!(
        form_sanitizer::normalize_email("  Alex.Doe@EXAMPLE.COM  "),
        "alex.doe@example.com"
    );
    assert_eq!(
        form_sanitizer::strip_tags("<p>Hello <b>World</b>!</p>"),
        "Hello World!"
    );
    assert!((form_sanitizer::clamp_number(150.0_f64, 0.0, 100.0) - 100.0).abs() < f64::EPSILON);
    assert!((form_sanitizer::clamp_number(-10.0_f64, 0.0, 100.0) - 0.0).abs() < f64::EPSILON);
}
