/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # Leptos UI Components
//!
//! RusToK's stable Leptos component package boundary for design system components
//! and Leptos-specific composites (LanguageToggle, RichTextEditorFrame, TableOfContents).

// Re-export core FFA components, types, and utilities from rustok-ui-leptos
pub use rustok_ui_leptos::{
    Alert, AlertVariant, Avatar, AvatarSize, Badge, BadgeVariant, Button, ButtonVariant, Card,
    CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle, CardVariant,
    Checkbox, Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader,
    DialogOverlay, DialogPortal, DialogState, DialogTitle, DialogTrigger, Input, InputType,
    Label, Orientation, Progress, Select, SelectOption, Separator, Size, Skeleton,
    SkeletonVariant, Spinner, Switch, SwitchSize, TabItem, Tabs, TabsContent, TabsList,
    TabsState, TabsTrigger, Textarea, alert_classes, avatar_classes, badge_classes, button_classes,
    card_action_classes, card_classes, card_content_classes, card_description_classes,
    card_footer_classes, card_header_classes, card_title_classes, checkbox_classes,
    dialog_backdrop_classes, dialog_close_classes, dialog_content_classes,
    dialog_description_classes, dialog_footer_classes, dialog_header_classes,
    dialog_title_classes, extract_initials, input_classes, label_classes, merge_classes,
    normalize_progress_max, normalize_progress_value, normalize_progress_value_for_max,
    progress_classes, progress_value_percentage, radius, select_classes, separator_classes,
    shadow, skeleton_classes, slugify_heading, spinner_classes, switch_classes,
    tabs_content_classes, tabs_list_classes, tabs_trigger_classes, textarea_classes,
    toc_header_classes, toc_item_classes, toc_list_classes, toc_nav_classes, ui_alert, ui_avatar,
    ui_badge, ui_button, ui_card, ui_card_action, ui_card_content, ui_card_description,
    ui_card_footer, ui_card_header, ui_card_title, ui_checkbox, ui_dialog, ui_dialog_close,
    ui_dialog_content, ui_dialog_description, ui_dialog_footer, ui_dialog_header,
    ui_dialog_overlay, ui_dialog_portal, ui_dialog_title, ui_dialog_trigger, ui_input, ui_label,
    ui_progress, ui_select, ui_separator, ui_skeleton, ui_spinner, ui_switch, ui_tabs,
    ui_tabs_content, ui_tabs_list, ui_tabs_trigger, ui_textarea, DISABLED_CONTROL_CLASSES,
    DISABLED_INPUT_CLASSES, FOCUS_RING_CLASSES, INPUT_FOCUS_RING_CLASSES,
    TRANSITION_COLORS_CLASSES,
};

// Local composite components
pub mod language_toggle;
pub mod richtext;
pub mod success_message;
pub mod toc;

pub use language_toggle::{LanguageToggle as ui_language_toggle, LanguageToggleOption};
pub use richtext::{
    RichTextEditorFrame, RichTextFrameCopy, RichTextHtml, localized_richtext_frame_copy,
};
pub use success_message::SuccessMessage as ui_success_message;
pub use toc::{
    TableOfContents, TableOfContents as ui_table_of_contents, TocItem, extract_headings_from_html,
};
