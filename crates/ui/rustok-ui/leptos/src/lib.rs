/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! SSR-first Leptos 0.8 design system adapter for RusToK UI components.

pub mod alert;
pub mod avatar;
pub mod badge;
pub mod button;
pub mod card;
pub mod checkbox;
pub mod dialog;
pub mod input;
pub mod label;
pub mod progress;
pub mod select;
pub mod separator;
pub mod skeleton;
pub mod spinner;
pub mod switch;
pub mod tabs;
pub mod textarea;
pub mod toc;

#[cfg(test)]
mod tests;

pub use alert::Alert;
pub use avatar::Avatar;
pub use badge::Badge;
pub use button::Button;
pub use card::{
    Card, CardAction, CardContent, CardDescription, CardFooter, CardHeader, CardTitle,
};
pub use checkbox::Checkbox;
pub use dialog::{
    Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader,
    DialogOverlay, DialogPortal, DialogTitle, DialogTrigger,
};
pub use input::Input;
pub use label::Label;
pub use progress::Progress;
pub use select::Select;
pub use separator::Separator;
pub use skeleton::Skeleton;
pub use spinner::Spinner;
pub use switch::Switch;
pub use tabs::{Tabs, TabsContent, TabsList, TabsTrigger};
pub use textarea::Textarea;
pub use toc::TableOfContents;

// Re-export core types and contracts
pub use rustok_ui::*;

// Canonical ui_ prefixed re-exports for module consistency
pub use alert::Alert as ui_alert;
pub use avatar::Avatar as ui_avatar;
pub use badge::Badge as ui_badge;
pub use button::Button as ui_button;
pub use checkbox::Checkbox as ui_checkbox;
pub use dialog::{
    Dialog as ui_dialog, DialogClose as ui_dialog_close, DialogContent as ui_dialog_content,
    DialogDescription as ui_dialog_description, DialogFooter as ui_dialog_footer,
    DialogHeader as ui_dialog_header, DialogOverlay as ui_dialog_overlay,
    DialogPortal as ui_dialog_portal, DialogTitle as ui_dialog_title,
    DialogTrigger as ui_dialog_trigger,
};
pub use input::Input as ui_input;
pub use label::Label as ui_label;
pub use progress::Progress as ui_progress;
pub use select::Select as ui_select;
pub use separator::Separator as ui_separator;
pub use skeleton::Skeleton as ui_skeleton;
pub use spinner::Spinner as ui_spinner;
pub use card::{
    Card as ui_card, CardAction as ui_card_action, CardContent as ui_card_content,
    CardDescription as ui_card_description, CardFooter as ui_card_footer,
    CardHeader as ui_card_header, CardTitle as ui_card_title,
};
pub use switch::Switch as ui_switch;
pub use tabs::{
    Tabs as ui_tabs, TabsContent as ui_tabs_content, TabsList as ui_tabs_list,
    TabsTrigger as ui_tabs_trigger,
};
pub use textarea::Textarea as ui_textarea;
pub use toc::TableOfContents as ui_table_of_contents;

