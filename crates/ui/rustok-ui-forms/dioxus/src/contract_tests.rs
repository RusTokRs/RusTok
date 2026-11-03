use dioxus::prelude::*;
use rustok_ui_forms::{DirtyTracker, FormState};

use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-forms-dioxus");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_button_variants_and_sizes_contract() {
    assert_eq!(ButtonVariant::default(), ButtonVariant::Primary);
    assert_eq!(ButtonSize::default(), ButtonSize::Md);

    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Ghost,
    ];

    for variant in variants {
        let classes = variant.class_names();
        assert!(!classes.is_empty(), "Variant class list must not be empty");
    }
}

#[test]
fn test_form_and_field_context_integration_contract() {
    let mut dom = VirtualDom::new(|| {
        let mut initial_state = FormState::idle();
        initial_state.set_submitted_failure("Form submission failed");
        initial_state.add_field_error("email", "Invalid email address");

        let state = Signal::new(initial_state);
        let tracker = Signal::new(DirtyTracker::new());
        let form_ctx = FormContext::new(state).with_dirty_tracker(tracker);

        assert_eq!(
            form_ctx.form_error().as_deref(),
            Some("Form submission failed")
        );
        assert!(form_ctx.is_field_invalid("email"));
        assert!(!form_ctx.is_field_invalid("username"));

        let field_ctx = FieldContext::new("email", "input-email-main", form_ctx);
        assert_eq!(field_ctx.name, "email");
        assert_eq!(field_ctx.id, "input-email-main");
        assert!(field_ctx.is_invalid());
        assert_eq!(
            field_ctx.error_message().as_deref(),
            Some("Invalid email address")
        );
        assert!(!field_ctx.is_dirty());

        field_ctx.mark_dirty();
        assert!(field_ctx.is_dirty());
        assert!(form_ctx.is_field_dirty("email"));

        field_ctx.mark_clean();
        assert!(!field_ctx.is_dirty());

        rsx! {}
    });

    dom.rebuild_in_place();
}
