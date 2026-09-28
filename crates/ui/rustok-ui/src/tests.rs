/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

#[cfg(test)]
mod tests {
    use crate::classes::*;
    use crate::contracts::*;
    use crate::types::*;

    #[test]
    fn test_button_classes_variants() {
        let default_cls = button_classes(ButtonVariant::Default, Size::Md, false, false, None);
        assert!(default_cls.contains("bg-primary"));
        assert!(default_cls.contains("h-9"));

        let dest_cls = button_classes(ButtonVariant::Destructive, Size::Sm, false, false, Some("custom-btn"));
        assert!(dest_cls.contains("bg-destructive"));
        assert!(dest_cls.contains("h-8"));
        assert!(dest_cls.contains("custom-btn"));

        let outline_cls = button_classes(ButtonVariant::Outline, Size::Lg, false, false, None);
        assert!(outline_cls.contains("border-input"));
        assert!(outline_cls.contains("h-10"));
    }

    #[test]
    fn test_input_and_textarea_classes() {
        let valid_input = input_classes(Size::Md, false, false, None);
        assert!(valid_input.contains("border-input"));
        assert!(!valid_input.contains("border-destructive"));

        let invalid_input = input_classes(Size::Sm, true, false, None);
        assert!(invalid_input.contains("border-destructive"));
        assert!(invalid_input.contains("h-8"));

        let valid_textarea = textarea_classes(Size::Md, false, false, None);
        assert!(valid_textarea.contains("resize-y"));
    }

    #[test]
    fn test_switch_classes() {
        let (track_off, thumb_off) = switch_classes(false, SwitchSize::Md, false, None);
        assert!(track_off.contains("bg-input"));
        assert!(thumb_off.contains("translate-x-0"));

        let (track_on, thumb_on) = switch_classes(true, SwitchSize::Md, false, None);
        assert!(track_on.contains("bg-primary"));
        assert!(thumb_on.contains("translate-x-5"));
    }

    #[test]
    fn test_badge_and_alert_classes() {
        let badge_success = badge_classes(BadgeVariant::Success, Size::Sm, None);
        assert!(badge_success.contains("bg-emerald-100"));

        let alert_destructive = alert_classes(AlertVariant::Destructive, None);
        assert!(alert_destructive.contains("border-red-300"));
    }

    #[test]
    fn test_avatar_initials_extraction() {
        assert_eq!(extract_initials("John Doe"), "JD");
        assert_eq!(extract_initials("Alice"), "A");
        assert_eq!(extract_initials("   Bob   Smith  "), "BS");
        assert_eq!(extract_initials(""), "?");
        assert_eq!(extract_initials("   "), "?");
    }

    #[test]
    fn test_headless_dialog_and_tabs_state() {
        let mut dialog = DialogState::new(false);
        assert!(!dialog.is_open);
        dialog.open();
        assert!(dialog.is_open);
        dialog.close();
        assert!(!dialog.is_open);
        dialog.toggle();
        assert!(dialog.is_open);

        let mut tabs = TabsState::new("general");
        assert!(tabs.is_active("general"));
        assert!(!tabs.is_active("security"));
        tabs.select("security");
        assert!(tabs.is_active("security"));
    }

    #[test]
    fn test_select_option_contract() {
        let opt = SelectOption::new("val-1", "Option 1").disabled();
        assert_eq!(opt.value, "val-1");
        assert_eq!(opt.label, "Option 1");
        assert!(opt.disabled);

        let json = serde_json::to_string(&opt).expect("serialize select option");
        let decoded: SelectOption = serde_json::from_str(&json).expect("deserialize select option");
        assert_eq!(decoded, opt);
    }
}
