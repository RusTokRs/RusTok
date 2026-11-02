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

    let initials = extract_initials("John Doe");
    assert_eq!(initials, "JD");

    let slug = slugify_heading("Hello World!", 1);
    assert_eq!(slug, "hello-world");
    assert_eq!(slugify_heading("", 0), "section-1");
}

#[test]
fn test_toc_item_contract() {
    let item = TocItem {
        id: "intro".to_string(),
        text: "Introduction".to_string(),
        level: 2,
    };
    assert_eq!(item.id, "intro");
    assert_eq!(item.text, "Introduction");
    assert_eq!(item.level, 2);
}
