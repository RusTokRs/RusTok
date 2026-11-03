use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "rustok-ui");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_button_and_badge_deterministic_resolvers() {
    let default_btn = button_classes(ButtonVariant::Default, Size::Md, None);
    assert!(default_btn.contains("bg-primary"));

    let destructive_btn =
        button_classes(ButtonVariant::Destructive, Size::Sm, Some("custom-action"));
    assert!(destructive_btn.contains("bg-destructive"));
    assert!(destructive_btn.contains("custom-action"));

    let badge = badge_classes(BadgeVariant::Default, Size::Md, None);
    assert!(!badge.is_empty());
    assert!(badge.contains("bg-primary"));
}

#[test]
fn test_headless_dialog_and_tabs_state() {
    let mut dialog = DialogState::new(false);
    assert!(!dialog.is_open);

    dialog.open();
    assert!(dialog.is_open);

    dialog.toggle();
    assert!(!dialog.is_open);

    let mut tabs = TabsState::new("overview");
    assert!(tabs.is_active("overview"));
    assert!(!tabs.is_active("settings"));

    tabs.select("settings");
    assert!(tabs.is_active("settings"));
    assert!(!tabs.is_active("overview"));
}

#[test]
fn test_tokens_and_utilities() {
    assert_eq!(extract_initials("Jane Doe"), "JD");
    assert_eq!(extract_initials("Admin"), "A");
    assert_eq!(
        slugify_heading("Introduction & Overview", 0),
        "introduction-overview"
    );
    assert_eq!(
        merge_classes(&["btn", "  ", "btn-primary", ""]),
        "btn btn-primary"
    );
}
