use super::*;

#[test]
fn test_reexported_variants_and_sizes() {
    let variants = [
        ButtonVariant::Default,
        ButtonVariant::Secondary,
        ButtonVariant::Outline,
        ButtonVariant::Destructive,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ];
    assert_eq!(variants.len(), 6);

    let sizes = [Size::Sm, Size::Md, Size::Lg];
    assert_eq!(sizes.len(), 3);

    let badge_variants = [
        BadgeVariant::Default,
        BadgeVariant::Secondary,
        BadgeVariant::Outline,
        BadgeVariant::Destructive,
    ];
    assert_eq!(badge_variants.len(), 4);
}

#[test]
fn test_headless_state_and_helpers() {
    let mut dialog = DialogState::new(false);
    assert!(!dialog.is_open);
    dialog.open();
    assert!(dialog.is_open);
    dialog.close();
    assert!(!dialog.is_open);

    let initials = extract_initials("Jane Doe");
    assert_eq!(initials, "JD");

    let slug = slugify_heading("Design System", 0);
    assert_eq!(slug, "design-system");
    assert_eq!(slugify_heading("", 0), "section-1");
}

#[test]
fn test_toc_item_contract() {
    let item = TocItem {
        id: "overview".to_string(),
        text: "Overview".to_string(),
        level: 1,
    };
    assert_eq!(item.id, "overview");
    assert_eq!(item.text, "Overview");
    assert_eq!(item.level, 1);
}
