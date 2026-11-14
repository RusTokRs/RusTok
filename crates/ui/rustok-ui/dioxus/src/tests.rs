/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use dioxus::prelude::*;
use rustok_ui::*;

use crate::alert::*;
use crate::avatar::*;
use crate::badge::*;
use crate::button::*;
use crate::card::*;
use crate::checkbox::*;
use crate::dialog::*;
use crate::input::*;
use crate::label::*;
use crate::progress::*;
use crate::select::*;
use crate::separator::*;
use crate::skeleton::*;
use crate::spinner::*;
use crate::switch::*;
use crate::tabs::*;
use crate::textarea::*;

#[test]
fn test_dioxus_button_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Button {
                variant: ButtonVariant::Destructive,
                size: Size::Sm,
                "Delete"
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_alert_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Alert {
                variant: AlertVariant::Warning,
                title: "Warning".to_string(),
                "Watch out"
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_badge_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Badge {
                variant: BadgeVariant::Success,
                "Active"
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_avatar_and_skeleton() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Avatar {
                fallback: "John Doe".to_string(),
            }
            Skeleton {
                variant: SkeletonVariant::Circular,
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_progress_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Progress {
                value: 42.5,
                max: 50.0,
                aria_label: Some("Upload progress".to_string()),
                aria_labelledby: Some("upload-label".to_string()),
                aria_value_text: Some("Nearly complete".to_string()),
                class: Some("max-w-sm".to_string()),
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_card_and_tabs_and_dialog() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Card {
                CardHeader {
                    CardTitle { "Title" }
                    CardDescription { "Desc" }
                }
                CardContent { "Content" }
            }
            Tabs {
                TabsList {
                    TabsTrigger { active: true, "T1" }
                }
                TabsContent { active: true, "C1" }
            }
            Dialog {
                open: true,
                aria_labelledby: Some("dialog-title".to_string()),
                DialogTitle {
                    id: Some("dialog-title".to_string()),
                    "Dialog Title"
                }
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_input_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Input {
                placeholder: "Enter value...".to_string(),
            }
            Input {
                placeholder: "Search".to_string(),
                prefix: rsx! { span { "🔍" } },
                suffix: rsx! { span { "Clear" } },
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_toc_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            crate::toc::TableOfContents {
                html: Some("<h2>First Section</h2><p>Content</p><h3>Subsection</h3>".to_string()),
                locale: Some("ru".to_string()),
            }
        }
    });
    dom.rebuild_in_place();
}

#[test]
fn test_dioxus_form_controls_instantiation() {
    let mut dom = VirtualDom::new(|| {
        rsx! {
            Label { r#for: "terms".to_string(), required: true, "Accept Terms" }
            Checkbox { id: Some("terms".to_string()), checked: true }
            Switch { id: Some("sw".to_string()), checked: true }
            Select {
                id: Some("sel".to_string()),
                options: vec![SelectOption::new("1", "One")],
                placeholder: Some("Choose".to_string()),
            }
            Textarea { id: Some("ta".to_string()), rows: 4, placeholder: Some("Notes".to_string()) }
            Separator { orientation: Orientation::Horizontal }
            Spinner { size: Size::Md }
        }
    });
    dom.rebuild_in_place();
}
