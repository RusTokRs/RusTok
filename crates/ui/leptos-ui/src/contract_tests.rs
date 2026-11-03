use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "leptos-ui");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_heading_slugification_and_extraction_contract() {
    let slug = slugify_heading("Getting Started with RusToK UI!", 0);
    assert_eq!(slug, "getting-started-with-rustok-ui");

    let html = r#"
        <h2>Introduction</h2>
        <p>Some introductory text</p>
        <h3>Architecture Overview</h3>
        <p>Details</p>
        <h2>Conclusion</h2>
    "#;

    let headings = extract_headings_from_html(html);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].level, 2);
    assert_eq!(headings[0].text, "Introduction");
    assert_eq!(headings[1].level, 3);
    assert_eq!(headings[1].text, "Architecture Overview");
    assert_eq!(headings[2].level, 2);
    assert_eq!(headings[2].text, "Conclusion");
}

#[test]
fn test_localized_richtext_frame_copy_contract() {
    let copy = localized_richtext_frame_copy(|_key, default| default.to_string());
    assert!(!copy.serialization_error.is_empty());
    assert!(!copy.invalid_payload_error.is_empty());
    assert!(!copy.frame_error.is_empty());
    assert!(copy.messages.is_object());
    assert_eq!(copy.messages["bold"], "Bold");
    assert_eq!(copy.messages["italic"], "Italic");
}

#[test]
fn test_reexported_ui_primitives_contract() {
    let btn_cls = button_classes(ButtonVariant::Default, Size::Md, None);
    assert!(!btn_cls.is_empty());

    let badge_cls = badge_classes(BadgeVariant::Default, Size::Md, None);
    assert!(!badge_cls.is_empty());

    let alert_cls = alert_classes(AlertVariant::Default, None);
    assert!(!alert_cls.is_empty());

    let initials = extract_initials("Jane Doe");
    assert_eq!(initials, "JD");
}
