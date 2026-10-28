/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::classes::*;
use crate::contracts::*;
use crate::tokens::{
    DISABLED_CONTROL_CLASSES, DISABLED_INPUT_CLASSES, FOCUS_RING_CLASSES, INPUT_FOCUS_RING_CLASSES,
    TRANSITION_COLORS_CLASSES, radius, shadow,
};
use crate::types::*;

#[test]
fn button_classes_cover_variants_sizes_and_custom_classes() {
    let default_cls = button_classes(ButtonVariant::Default, Size::Md, None);
    assert!(default_cls.contains("bg-primary"));
    assert!(default_cls.contains("h-9"));
    assert!(default_cls.contains("shadow-xs"));
    assert!(default_cls.contains("focus-visible:ring-2"));
    assert!(default_cls.contains("disabled:pointer-events-none disabled:opacity-50"));

    let destructive = button_classes(ButtonVariant::Destructive, Size::Sm, Some("custom-btn"));
    assert!(destructive.contains("bg-destructive"));
    assert!(destructive.contains("h-8"));
    assert!(destructive.contains("custom-btn"));

    let outline = button_classes(ButtonVariant::Outline, Size::Lg, None);
    assert!(outline.contains("border-input"));
    assert!(outline.contains("h-10"));

    let ghost = button_classes(ButtonVariant::Ghost, Size::Icon, None);
    assert!(ghost.contains("h-9 w-9 p-0"));
    assert!(ghost.contains("shadow-none"));

    let link = button_classes(ButtonVariant::Link, Size::Xs, None);
    assert!(link.contains("hover:underline"));
    assert!(link.contains("h-7"));
}

#[test]
fn input_textarea_and_select_classes_distinguish_valid_and_invalid() {
    let valid_input = input_classes(Size::Md, false, None);
    assert!(valid_input.contains("border-input"));
    assert!(valid_input.contains("focus-visible:ring-ring"));
    assert!(!valid_input.contains("border-destructive"));
    assert!(valid_input.contains("disabled:cursor-not-allowed"));

    let invalid_input = input_classes(Size::Sm, true, None);
    assert!(invalid_input.contains("border-destructive"));
    assert!(invalid_input.contains("focus-visible:ring-destructive"));
    assert!(invalid_input.contains("h-8"));

    let valid_textarea = textarea_classes(Size::Md, false, None);
    assert!(valid_textarea.contains("resize-y"));

    let invalid_textarea = textarea_classes(Size::Lg, true, None);
    assert!(invalid_textarea.contains("border-destructive"));
    assert!(invalid_textarea.contains("px-4 py-3"));

    let valid_select = select_classes(Size::Md, false, None);
    assert!(valid_select.contains("focus:ring-ring"));

    let invalid_select = select_classes(Size::Lg, true, None);
    assert!(invalid_select.contains("focus:ring-destructive"));
}

#[test]
fn checkbox_and_switch_classes_expose_state_and_disabled_utilities() {
    let checkbox = checkbox_classes(None);
    assert!(checkbox.contains("h-4 w-4"));
    assert!(checkbox.contains("focus-visible:ring-1"));
    assert!(checkbox.contains("disabled:cursor-not-allowed"));

    let (track_off, thumb_off) = switch_classes(false, SwitchSize::Md, None);
    assert!(track_off.contains("bg-input"));
    assert!(track_off.contains("h-6"));
    assert!(thumb_off.contains("translate-x-0"));

    let (track_on, thumb_on) = switch_classes(true, SwitchSize::Md, Some("custom-switch"));
    assert!(track_on.contains("bg-primary"));
    assert!(track_on.contains("custom-switch"));
    assert!(thumb_on.contains("translate-x-5"));

    let (small_track, small_thumb) = switch_classes(true, SwitchSize::Sm, None);
    assert!(small_track.contains("w-7"));
    assert!(small_thumb.contains("translate-x-3"));
}

#[test]
fn badge_alert_and_card_classes_cover_variants() {
    let badge_success = badge_classes(BadgeVariant::Success, Size::Sm, None);
    assert!(badge_success.contains("bg-emerald-100"));
    assert!(badge_success.contains("rounded-full"));
    assert!(badge_success.contains("shadow-none"));

    let badge_default = badge_classes(BadgeVariant::Default, Size::Md, None);
    assert!(badge_default.contains("hover:bg-primary/80"));

    let alert_destructive = alert_classes(AlertVariant::Destructive, None);
    assert!(alert_destructive.contains("border-red-300"));

    let card_default = card_classes(CardVariant::Default, None);
    assert!(card_default.contains("rounded-xl"));
    assert!(card_default.contains("shadow-sm"));

    let card_elevated = card_classes(CardVariant::Elevated, None);
    assert!(card_elevated.contains("shadow-md"));

    let header = card_header_classes(None);
    assert!(header.contains("px-6"));
    assert!(!header.ends_with(' '));
}

#[test]
fn avatar_spinner_skeleton_label_and_separator_classes() {
    let (container, fallback) = avatar_classes(AvatarSize::Sm, Some("avatar-x"));
    assert!(container.contains("h-8 w-8"));
    assert!(container.contains("avatar-x"));
    assert!(fallback.contains("text-xs"));

    let spinner = spinner_classes(Size::Lg, None);
    assert!(spinner.contains("border-[3px]"));
    assert!(spinner.contains("animate-spin"));

    let skeleton = skeleton_classes(SkeletonVariant::Text, None);
    assert!(skeleton.contains("h-4 w-full"));
    assert!(skeleton.contains("rounded-sm"));

    let circular = skeleton_classes(SkeletonVariant::Circular, None);
    assert!(circular.contains("rounded-full"));

    let label = label_classes(true, None);
    assert!(label.contains("cursor-not-allowed"));

    let disabled_label = label_classes(false, None);
    assert!(disabled_label.contains("peer-disabled:opacity-70"));

    let vertical = separator_classes(Orientation::Vertical, None);
    assert!(vertical.contains("h-full w-px"));
    assert!(separator_classes(Orientation::Horizontal, None).contains("w-full h-px"));
}

#[test]
fn progress_class_and_percentage_normalization_are_consistent() {
    let classes = progress_classes(Some("max-w-sm"));
    assert!(classes.contains("h-2 w-full"));
    assert!(classes.contains("bg-primary/20"));
    assert!(classes.contains("max-w-sm"));
    assert!(!classes.contains("  "));

    assert_eq!(normalize_progress_value(-1.0), 0.0);
    assert_eq!(normalize_progress_value(-0.0).to_bits(), 0.0_f64.to_bits());
    assert_eq!(normalize_progress_value(37.5), 37.5);
    assert_eq!(normalize_progress_value(101.0), 100.0);
    assert_eq!(normalize_progress_value(f64::NAN), 0.0);
    assert_eq!(normalize_progress_value(f64::INFINITY), 0.0);
    assert_eq!(normalize_progress_value(f64::NEG_INFINITY), 0.0);

    assert_eq!(normalize_progress_max(5.0), 5.0);
    assert_eq!(normalize_progress_max(0.0), 100.0);
    assert_eq!(normalize_progress_max(f64::NAN), 100.0);
    assert_eq!(normalize_progress_value_for_max(-1.0, 5.0), 0.0);
    assert_eq!(normalize_progress_value_for_max(2.5, 5.0), 2.5);
    assert_eq!(normalize_progress_value_for_max(8.0, 5.0), 5.0);
    assert_eq!(progress_value_percentage(2.5, 5.0), 50.0);
}

#[test]
fn dialog_and_tabs_classes_track_open_and_active_state() {
    let open_backdrop = dialog_backdrop_classes(true);
    assert!(open_backdrop.contains("opacity-100"));
    assert!(dialog_backdrop_classes(false).contains("pointer-events-none"));

    let open_content = dialog_content_classes(true, Some("modal-x"));
    assert!(open_content.contains("scale-100"));
    assert!(open_content.contains("modal-x"));
    assert!(dialog_content_classes(false, None).contains("scale-95"));

    let close_control = dialog_close_classes(None);
    assert!(close_control.contains("absolute right-4 top-4"));
    assert!(close_control.contains("hover:opacity-100"));

    let vertical_list = tabs_list_classes(Orientation::Vertical, None);
    assert!(vertical_list.contains("flex-col"));
    assert!(tabs_list_classes(Orientation::Horizontal, None).contains("h-9"));

    let active_trigger = tabs_trigger_classes(true, false, None);
    assert!(active_trigger.contains("bg-background"));
    assert!(active_trigger.contains("shadow-xs"));
    assert!(!active_trigger.contains("  "));

    let disabled_trigger = tabs_trigger_classes(false, true, None);
    assert!(disabled_trigger.contains("pointer-events-none"));

    assert!(tabs_content_classes(true, None).contains("block"));
    assert!(tabs_content_classes(false, None).contains("hidden"));
}

#[test]
fn merge_classes_trims_skips_and_joins_fragments() {
    assert_eq!(merge_classes(&[]), "");
    assert_eq!(merge_classes(&["a", "", "   ", "b"]), "a b");
    assert_eq!(merge_classes(&["a ", " b"]), "a b");
    assert_eq!(merge_classes(&["single"]), "single");
}

#[test]
fn type_helpers_return_attribute_values() {
    assert_eq!(InputType::Text.as_str(), "text");
    assert_eq!(InputType::Password.as_str(), "password");
    assert_eq!(InputType::Email.as_str(), "email");
    assert_eq!(InputType::Number.as_str(), "number");
    assert_eq!(InputType::Search.as_str(), "search");
    assert_eq!(InputType::Tel.as_str(), "tel");
    assert_eq!(InputType::Url.as_str(), "url");
    assert_eq!(InputType::Date.as_str(), "date");

    assert_eq!(Orientation::Horizontal.as_str(), "horizontal");
    assert_eq!(Orientation::Vertical.as_str(), "vertical");
}

#[test]
fn extract_initials_handles_names_punctuation_and_expanding_uppercase() {
    assert_eq!(extract_initials("John Doe"), "JD");
    assert_eq!(extract_initials("Alice"), "A");
    assert_eq!(extract_initials("   Bob   Smith  "), "BS");
    assert_eq!(extract_initials(""), "?");
    assert_eq!(extract_initials("   "), "?");
    assert_eq!(extract_initials("***"), "?");
    assert_eq!(extract_initials("\"Quoted\" Name"), "QN");
    assert_eq!(extract_initials("Straße"), "S");
    assert_eq!(extract_initials("иван петров"), "ИП");
    assert_eq!(extract_initials("one two three"), "OT");
}

#[test]
fn headless_dialog_and_tabs_state() {
    let mut dialog = DialogState::new(false);
    assert!(!dialog.is_open);
    dialog.open();
    assert!(dialog.is_open);
    dialog.close();
    assert!(!dialog.is_open);
    dialog.toggle();
    assert!(dialog.is_open);
    assert!(!DialogState::default().is_open);

    let mut tabs = TabsState::new("general");
    assert!(tabs.is_active("general"));
    assert!(!tabs.is_active("security"));
    tabs.select("security");
    assert!(tabs.is_active("security"));
}

#[test]
fn select_option_and_tab_item_contracts_round_trip_through_serde() {
    let opt = SelectOption::new("val-1", "Option 1").disabled();
    assert_eq!(opt.value, "val-1");
    assert_eq!(opt.label, "Option 1");
    assert!(opt.disabled);
    assert!(!SelectOption::new("val-2", "Option 2").disabled);

    let json = serde_json::to_string(&opt).expect("serialize select option");
    let decoded: SelectOption = serde_json::from_str(&json).expect("deserialize select option");
    assert_eq!(decoded, opt);

    let tab = TabItem::new("general", "General").with_badge("3");
    assert_eq!(tab.id, "general");
    assert_eq!(tab.badge.as_deref(), Some("3"));
    assert!(!tab.disabled);
    assert!(TabItem::new("danger", "Danger").disabled().disabled);

    let (container, _) = avatar_classes(AvatarSize::Md, None);
    assert!(!container.is_empty());
}

#[test]
fn every_resolver_returns_a_well_formed_class_list() {
    let (track_off, thumb_off) = switch_classes(false, SwitchSize::Md, None);
    let (track_on, thumb_on) = switch_classes(true, SwitchSize::Lg, Some("custom-switch"));
    let (avatar, avatar_fallback) = avatar_classes(AvatarSize::Xs, Some("avatar-x"));

    let lists = vec![
        button_classes(ButtonVariant::Default, Size::Md, Some("custom")),
        input_classes(Size::Md, true, Some("custom")),
        textarea_classes(Size::Sm, false, None),
        select_classes(Size::Lg, true, None),
        checkbox_classes(Some("custom")),
        badge_classes(BadgeVariant::Warning, Size::Xs, None),
        alert_classes(AlertVariant::Info, Some("custom")),
        card_classes(CardVariant::Bordered, None),
        card_header_classes(None),
        card_title_classes(None),
        card_description_classes(None),
        card_action_classes(None),
        card_content_classes(None),
        card_footer_classes(None),
        skeleton_classes(SkeletonVariant::Rectangular, Some("h-10 w-10")),
        spinner_classes(Size::Xl, None),
        label_classes(true, None),
        separator_classes(Orientation::Vertical, None),
        dialog_backdrop_classes(true),
        dialog_content_classes(true, None),
        dialog_header_classes(None),
        dialog_title_classes(None),
        dialog_description_classes(None),
        dialog_footer_classes(None),
        tabs_list_classes(Orientation::Vertical, None),
        tabs_trigger_classes(true, false, None),
        tabs_content_classes(false, None),
    ];

    let mut all = lists;
    all.extend([
        track_off,
        thumb_off,
        track_on,
        thumb_on,
        avatar,
        avatar_fallback,
    ]);

    for class_list in all {
        assert!(!class_list.is_empty());
        assert_eq!(
            class_list,
            class_list.trim(),
            "untrimmed class list: {class_list:?}"
        );
        assert!(
            !class_list.contains("  "),
            "doubled separator in: {class_list:?}"
        );
    }
}

#[test]
fn variant_and_size_matrices_are_non_empty_and_distinct() {
    use std::collections::BTreeSet;

    let mut button_variants = BTreeSet::new();
    for variant in [
        ButtonVariant::Default,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ] {
        let resolved = button_classes(variant, Size::Md, None);
        assert!(!resolved.is_empty());
        assert!(
            button_variants.insert(resolved),
            "duplicate button variant styles"
        );
    }

    let mut button_sizes = BTreeSet::new();
    for size in [Size::Xs, Size::Sm, Size::Md, Size::Lg, Size::Xl, Size::Icon] {
        let resolved = button_classes(ButtonVariant::Default, size, None);
        assert!(
            button_sizes.insert(resolved),
            "duplicate button size styles"
        );
    }

    let mut badge_variants = BTreeSet::new();
    for variant in [
        BadgeVariant::Default,
        BadgeVariant::Secondary,
        BadgeVariant::Destructive,
        BadgeVariant::Outline,
        BadgeVariant::Success,
        BadgeVariant::Warning,
        BadgeVariant::Info,
    ] {
        assert!(badge_variants.insert(badge_classes(variant, Size::Md, None)));
    }

    let mut alert_variants = BTreeSet::new();
    for variant in [
        AlertVariant::Default,
        AlertVariant::Info,
        AlertVariant::Warning,
        AlertVariant::Destructive,
        AlertVariant::Success,
    ] {
        assert!(alert_variants.insert(alert_classes(variant, None)));
    }

    let mut card_variants = BTreeSet::new();
    for variant in [
        CardVariant::Default,
        CardVariant::Bordered,
        CardVariant::Elevated,
        CardVariant::Ghost,
    ] {
        assert!(card_variants.insert(card_classes(variant, None)));
    }

    let mut avatar_sizes = BTreeSet::new();
    for size in [
        AvatarSize::Xs,
        AvatarSize::Sm,
        AvatarSize::Md,
        AvatarSize::Lg,
        AvatarSize::Xl,
    ] {
        assert!(avatar_sizes.insert(avatar_classes(size, None).0));
    }

    let mut skeleton_variants = BTreeSet::new();
    for variant in [
        SkeletonVariant::Text,
        SkeletonVariant::Circular,
        SkeletonVariant::Rectangular,
    ] {
        assert!(skeleton_variants.insert(skeleton_classes(variant, None)));
    }

    let mut switch_sizes = BTreeSet::new();
    for size in [SwitchSize::Sm, SwitchSize::Md, SwitchSize::Lg] {
        assert!(switch_sizes.insert(switch_classes(false, size, None).0));
    }
}

#[test]
fn design_tokens_are_non_empty_utility_fragments() {
    let tokens = [
        FOCUS_RING_CLASSES,
        INPUT_FOCUS_RING_CLASSES,
        TRANSITION_COLORS_CLASSES,
        DISABLED_CONTROL_CLASSES,
        DISABLED_INPUT_CLASSES,
        radius::FULL,
        radius::XL,
        radius::LG,
        radius::MD,
        radius::SM,
        shadow::NONE,
        shadow::XS,
        shadow::SM,
        shadow::MD,
        shadow::LG,
    ];

    for token in tokens {
        assert!(!token.is_empty());
        assert_eq!(token, token.trim());
        assert!(!token.contains("  "));
    }
}

#[test]
fn toc_item_and_heading_extraction() {
    let html = r#"
        <article>
            <h2 id="intro">Introduction</h2>
            <p>Intro text</p>
            <h3>Nested <strong>Subheading</strong></h3>
            <h2 id="custom-slug">Summary</h2>
        </article>
    "#;

    let headings = extract_headings_from_html(html);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].id, "intro");
    assert_eq!(headings[0].text, "Introduction");
    assert_eq!(headings[0].level, 2);

    assert_eq!(headings[1].id, "nested-subheading");
    assert_eq!(headings[1].text, "Nested Subheading");
    assert_eq!(headings[1].level, 3);

    assert_eq!(headings[2].id, "custom-slug");
    assert_eq!(headings[2].text, "Summary");
    assert_eq!(headings[2].level, 2);

    let nav_cls = toc_nav_classes(true, Some("my-custom-toc"));
    assert!(nav_cls.contains("sticky top-24"));
    assert!(nav_cls.contains("my-custom-toc"));

    let item_active_cls = toc_item_classes(3, true);
    assert!(item_active_cls.contains("pl-3"));
    assert!(item_active_cls.contains("border-primary"));

    let item_inactive_cls = toc_item_classes(2, false);
    assert!(!item_inactive_cls.contains("pl-3"));
    assert!(item_inactive_cls.contains("text-muted-foreground"));
}
