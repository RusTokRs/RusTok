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
pub mod select;
pub mod separator;
pub mod skeleton;
pub mod spinner;
pub mod switch;
pub mod tabs;
pub mod textarea;

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
pub use dialog::{Dialog, DialogDescription, DialogFooter, DialogHeader, DialogTitle};
pub use input::Input;
pub use label::Label;
pub use select::Select;
pub use separator::Separator;
pub use skeleton::Skeleton;
pub use spinner::Spinner;
pub use switch::Switch;
pub use tabs::{Tabs, TabsContent, TabsList, TabsTrigger};
pub use textarea::Textarea;

// Re-export core types and contracts
pub use rustok_ui::*;

// Canonical ui_ prefixed re-exports for module consistency
pub use alert::Alert as ui_alert;
pub use avatar::Avatar as ui_avatar;
pub use badge::Badge as ui_badge;
pub use button::Button as ui_button;
pub use checkbox::Checkbox as ui_checkbox;
pub use dialog::Dialog as ui_dialog;
pub use input::Input as ui_input;
pub use label::Label as ui_label;
pub use select::Select as ui_select;
pub use separator::Separator as ui_separator;
pub use skeleton::Skeleton as ui_skeleton;
pub use spinner::Spinner as ui_spinner;
pub use switch::Switch as ui_switch;
pub use textarea::Textarea as ui_textarea;
