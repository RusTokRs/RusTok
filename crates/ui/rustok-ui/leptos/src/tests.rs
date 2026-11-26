/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::prelude::*;
use rustok_ui::*;

use crate::alert::*;
use crate::avatar::*;
use crate::badge::*;
use crate::button::*;
use crate::card::*;
use crate::checkbox::*;
use crate::dialog::*;
use crate::icon::*;
use crate::input::*;
use crate::language_toggle::*;
use crate::progress::*;
use crate::select::*;
use crate::skeleton::*;
use crate::spinner::*;
use crate::success_message::*;
use crate::switch::*;
use crate::tabs::*;

#[test]
fn test_render_button_ssr() {
    let view = view! {
        <Button variant=ButtonVariant::Destructive size=Size::Sm>
            "Delete"
        </Button>
    };
    let html = view.to_html();
    assert!(html.contains("<button"));
    assert!(html.contains("bg-destructive"));
    assert!(html.contains("Delete"));
}

#[test]
fn test_render_alert_ssr() {
    let view = view! {
        <Alert variant=AlertVariant::Destructive title="Fatal Error".to_string()>
            "System is down"
        </Alert>
    };
    let html = view.to_html();
    assert!(html.contains("role=\"alert\""));
    assert!(html.contains("Fatal Error"));
    assert!(html.contains("System is down"));
    assert!(html.contains("border-red-300"));
}

#[test]
fn test_render_badge_ssr() {
    let view = view! {
        <Badge variant=BadgeVariant::Success>
            "Active"
        </Badge>
    };
    let html = view.to_html();
    assert!(html.contains("bg-emerald-100"));
    assert!(html.contains("Active"));
}

#[test]
fn test_render_avatar_ssr() {
    let view_fallback = view! {
        <Avatar fallback="Jane Smith".to_string() size=AvatarSize::Md />
    };
    let html_fallback = view_fallback.to_html();
    assert!(html_fallback.contains("JS"));

    let view_img = view! {
        <Avatar src="https://example.com/photo.jpg".to_string() alt="Profile".to_string() />
    };
    let html_img = view_img.to_html();
    assert!(html_img.contains("<img"));
    assert!(html_img.contains("src=\"https://example.com/photo.jpg\""));
}

#[test]
fn test_render_skeleton_ssr() {
    let view = view! {
        <Skeleton variant=SkeletonVariant::Circular class="h-12 w-12".to_string() />
    };
    let html_circ = view.to_html();
    assert!(html_circ.contains("rounded-full"));

    let view_rect = view! {
        <Skeleton variant=SkeletonVariant::Rectangular />
    };
    let html = view_rect.to_html();
    assert!(html.contains("animate-pulse"));
    assert!(html.contains("rounded-md"));
}

#[test]
fn test_render_progress_ssr() {
    let value = Signal::derive(|| 37.5);
    let html = view! {
        <Progress value=value aria_label="Upload progress" class="max-w-sm" />
    }
    .to_html();

    assert!(html.contains("role=\"progressbar\""));
    assert!(html.contains("aria-label=\"Upload progress\""));
    assert!(html.contains("aria-valuemin=\"0\""));
    assert!(html.contains("aria-valuemax=\"100\""));
    assert!(html.contains("aria-valuenow=\"37.5\""));
    assert!(html.contains("width=\"37.5\""));
    assert!(html.contains("max-w-sm"));
    assert!(html.contains("aria-hidden=\"true\""));

    let overrun = Signal::derive(|| 125.0);
    let overrun_html = view! {
        <Progress value=overrun aria_label="Upload progress" />
    }
    .to_html();
    assert!(overrun_html.contains("aria-valuenow=\"100\""));
    assert!(overrun_html.contains("data-state=\"complete\""));
    assert!(overrun_html.contains("width=\"100\""));

    let custom_range = Signal::derive(|| 25.0);
    let custom_max = Signal::derive(|| 50.0);
    let custom_range_html = view! {
        <Progress
            value=custom_range
            max=custom_max
            aria_labelledby="upload-label"
            aria_value_text="Half complete"
            orientation=Orientation::Vertical
        />
    }
    .to_html();
    assert!(custom_range_html.contains("aria-labelledby=\"upload-label\""));
    assert!(custom_range_html.contains("aria-valuetext=\"Half complete\""));
    assert!(custom_range_html.contains("aria-valuemax=\"50\""));
    assert!(custom_range_html.contains("aria-valuenow=\"25\""));
    assert!(custom_range_html.contains("aria-orientation=\"vertical\""));
    assert!(custom_range_html.contains("data-state=\"loading\""));
    assert!(custom_range_html.contains("data-max=\"50\""));
    assert!(custom_range_html.contains("width=\"50\""));

    let empty_html = view! { <Progress aria_label="Pending task" /> }.to_html();
    assert!(empty_html.contains("aria-valuenow=\"0\""));
    assert!(empty_html.contains("width=\"0\""));
}

#[test]
fn test_render_card_ssr() {
    let view = view! {
        <Card>
            <CardHeader>
                <CardTitle>"Settings"</CardTitle>
                <CardDescription>"Manage account"</CardDescription>
            </CardHeader>
            <CardContent>
                <p>"Body"</p>
            </CardContent>
        </Card>
    };
    let html = view.to_html();
    assert!(html.contains("Settings"));
    assert!(html.contains("Manage account"));
    assert!(html.contains("Body"));
}

#[test]
fn test_render_tabs_ssr() {
    let view = view! {
        <Tabs>
            <TabsList>
                <TabsTrigger active=true>"Tab 1"</TabsTrigger>
                <TabsTrigger active=false>"Tab 2"</TabsTrigger>
            </TabsList>
            <TabsContent active=true>"Content 1"</TabsContent>
        </Tabs>
    };
    let html = view.to_html();
    assert!(html.contains("role=\"tablist\""));
    assert!(html.contains("role=\"tab\""));
    assert!(html.contains("role=\"tabpanel\""));
    assert!(html.contains("Tab 1"));
}

#[test]
fn test_dialog_root_and_trigger_render_ssr_state() {
    let html_closed = Owner::new().with(|| {
        let view_closed = view! {
            <Dialog default_open=false>
                <DialogTrigger aria_controls="dialog-panel">"Open dialog"</DialogTrigger>
                <DialogContent id="dialog-panel" aria_labelledby="dialog-title">
                    <DialogTitle id="dialog-title">"Title"</DialogTitle>
                    <DialogDescription id="dialog-description">"Details"</DialogDescription>
                    <DialogClose>"Done"</DialogClose>
                </DialogContent>
            </Dialog>
        };
        view_closed.to_html()
    });
    assert!(html_closed.contains("data-slot=\"dialog-trigger\""));
    assert!(html_closed.contains("aria-haspopup=\"dialog\""));
    assert!(html_closed.contains("aria-controls=\"dialog-panel\""));
    assert!(html_closed.contains("aria-expanded=\"false\""));
    assert!(html_closed.contains("data-state=\"closed\""));

    let html_open = Owner::new().with(|| {
        let view_open = view! {
            <Dialog default_open=true>
                <DialogTrigger>"Open dialog"</DialogTrigger>
                <DialogContent aria_label="Accessible dialog">
                    <DialogTitle>"Title"</DialogTitle>
                </DialogContent>
            </Dialog>
        };
        view_open.to_html()
    });
    assert!(html_open.contains("aria-expanded=\"true\""));
    assert!(html_open.contains("data-state=\"open\""));
    assert!(html_open.contains("aria-controls=\"dialog-"));

    // Leptos Portal, like Radix's client portal, mounts overlay/content only in
    // the browser; the server output retains the trigger but no dialog panel.
    assert!(!html_open.contains("role=\"dialog\""));
}

#[test]
fn test_render_checkbox_ssr() {
    let view_static = view! {
        <Checkbox id="terms" indeterminate=true />
    };
    let html_static = view_static.to_html();
    assert!(html_static.contains("type=\"checkbox\""));
    assert!(html_static.contains("id=\"terms\""));
    assert!(html_static.contains("data-slot=\"checkbox\""));
    assert!(html_static.contains("data-state=\"indeterminate\""));
    assert!(html_static.contains("aria-checked=\"mixed\""));

    let is_ind = Signal::derive(|| true);
    let view_reactive = view! {
        <Checkbox indeterminate=is_ind />
    };
    let html_reactive = view_reactive.to_html();
    assert!(html_reactive.contains("type=\"checkbox\""));
    assert!(html_reactive.contains("data-slot=\"checkbox\""));
    assert!(html_reactive.contains("data-state=\"indeterminate\""));
    assert!(html_reactive.contains("aria-checked=\"mixed\""));
}

#[test]
fn test_render_input_ssr() {
    let view_plain = view! {
        <Input placeholder="Enter username" />
    };
    let html_plain = view_plain.to_html();
    assert!(html_plain.contains("placeholder=\"Enter username\""));
    assert!(html_plain.contains("data-slot=\"input\""));
    assert!(!html_plain.contains("pl-9"));

    let view_adorned = view! {
        <Input
            placeholder="Search..."
            prefix=view! { <span>"🔍"</span> }.into_any()
            suffix=view! { <span>"Clear"</span> }.into_any()
        />
    };
    let html_adorned = view_adorned.to_html();
    assert!(html_adorned.contains("data-slot=\"input-wrapper\""));
    assert!(html_adorned.contains("data-slot=\"input\""));
    assert!(html_adorned.contains("pl-9"));
    assert!(html_adorned.contains("pr-9"));
    assert!(html_adorned.contains("🔍"));
    assert!(html_adorned.contains("Clear"));
}

#[test]
fn test_render_toc_ssr() {
    let view = view! {
        <crate::toc::TableOfContents
            html="<h2>Overview</h2><p>text</p><h3>Details</h3><p>more</p>".to_string()
            locale="ru".to_string()
        />
    };
    let html = view.to_html();
    assert!(html.contains("<nav"));
    assert!(html.contains("data-slot=\"toc\""));
    assert!(html.contains("data-slot=\"toc-header\""));
    assert!(html.contains("data-slot=\"toc-list\""));
    assert!(html.contains("Содержание"));
    assert!(html.contains("Overview"));
    assert!(html.contains("Details"));
}

#[test]
fn test_render_declarative_tabs_ssr() {
    let html = Owner::new().with(|| {
        let view = view! {
            <Tabs default_value="tab2".to_string()>
                <TabsList>
                    <TabsTrigger value="tab1".to_string() id="trigger-1" panel_id="panel-1">"Tab 1"</TabsTrigger>
                    <TabsTrigger value="tab2".to_string() id="trigger-2" panel_id="panel-2">"Tab 2"</TabsTrigger>
                </TabsList>
                <TabsContent value="tab1".to_string() id="panel-1" tab_id="trigger-1">"Content 1"</TabsContent>
                <TabsContent value="tab2".to_string() id="panel-2" tab_id="trigger-2">"Content 2"</TabsContent>
            </Tabs>
        };
        view.to_html()
    });

    assert!(html.contains("role=\"tablist\""));
    assert!(html.contains("data-slot=\"tabs\""));
    assert!(html.contains("data-slot=\"tabs-list\""));
    assert!(html.contains("data-slot=\"tabs-trigger\""));
    assert!(html.contains("data-slot=\"tabs-content\""));
    assert!(html.contains("data-state=\"inactive\""));
    assert!(html.contains("data-state=\"active\""));
    assert!(html.contains("data-orientation=\"horizontal\""));
    assert!(html.contains("role=\"tab\""));
    assert!(html.contains("aria-selected=\"false\""));
    assert!(html.contains("aria-selected=\"true\""));
    assert!(html.contains("tabindex=\"-1\""));
    assert!(html.contains("tabindex=\"0\""));
    assert!(html.contains("role=\"tabpanel\""));
    assert!(html.contains("Content 2"));
}

#[test]
fn test_render_switch_ssr() {
    // Uncontrolled default (off)
    let view_default = view! {
        <Switch id="sw-default" aria_label="Default toggle" />
    };
    let html_default = view_default.to_html();
    assert!(html_default.contains("role=\"switch\""));
    assert!(html_default.contains("aria-checked=\"false\""));
    assert!(html_default.contains("data-state=\"unchecked\""));
    assert!(html_default.contains("aria-label=\"Default toggle\""));
    assert!(html_default.contains("data-slot=\"switch\""));
    assert!(html_default.contains("data-slot=\"switch-thumb\""));

    // Uncontrolled default_checked (on) with form name
    let view_checked = view! {
        <Switch
            id="sw-notifications"
            default_checked=true
            name="notifications"
            value="enabled".to_string()
        />
    };
    let html_checked = view_checked.to_html();
    assert!(html_checked.contains("role=\"switch\""));
    assert!(html_checked.contains("aria-checked=\"true\""));
    assert!(html_checked.contains("data-state=\"checked\""));
    assert!(html_checked.contains("type=\"checkbox\""));
    assert!(html_checked.contains("name=\"notifications\""));
    assert!(html_checked.contains("value=\"enabled\""));

    // Controlled switch
    let html_controlled = Owner::new().with(|| {
        let (checked, set_checked) = signal(true);
        let view_controlled = view! {
            <Switch
                id="sw-ctrl"
                checked=checked
                set_checked=set_checked
            />
        };
        view_controlled.to_html()
    });
    assert!(html_controlled.contains("aria-checked=\"true\""));
    assert!(html_controlled.contains("data-state=\"checked\""));
}

#[test]
fn test_render_select_ssr() {
    let options = Signal::derive(|| {
        vec![
            SelectOption::new("opt1", "Option 1"),
            SelectOption::new("opt2", "Option 2"),
        ]
    });
    let view = view! {
        <Select
            id="test-select"
            name="test-name"
            options=options
            placeholder="Select an item".to_string()
            aria_label="Test Select"
        />
    };
    let html = view.to_html();
    assert!(html.contains("data-slot=\"select\""));
    assert!(html.contains("id=\"test-select\""));
    assert!(html.contains("name=\"test-name\""));
    assert!(html.contains("aria-label=\"Test Select\""));
    assert!(html.contains("Select an item"));
    assert!(html.contains("Option 1"));
    assert!(html.contains("Option 2"));
}

#[test]
fn test_render_spinner_ssr() {
    let view = view! {
        <Spinner size=Size::Lg aria_label="Please wait" />
    };
    let html = view.to_html();
    assert!(html.contains("data-slot=\"spinner\""));
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("aria-label=\"Please wait\""));
}

#[test]
fn test_render_success_message_ssr() {
    let view = view! {
        <SuccessMessage message="Item saved successfully!".to_string() />
    };
    let html = view.to_html();
    assert!(html.contains("data-slot=\"success-message\""));
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("Item saved successfully!"));
}

#[test]
fn test_render_language_toggle_ssr() {
    let current = Signal::derive(|| "en".to_string());
    let options = vec![
        LanguageToggleOption::new("en", "English"),
        LanguageToggleOption::new("ru", "Русский"),
    ];
    let view = view! {
        <LanguageToggle
            current_locale=current
            options=options
            on_set_locale=|_| {}
        />
    };
    let html = view.to_html();
    assert!(html.contains("data-slot=\"language-toggle\""));
    assert!(html.contains("data-slot=\"language-toggle-item\""));
    assert!(html.contains("data-state=\"active\""));
    assert!(html.contains("data-state=\"inactive\""));
    assert!(html.contains("aria-pressed=\"true\""));
    assert!(html.contains("aria-pressed=\"false\""));
    assert!(html.contains("English"));
    assert!(html.contains("Русский"));
}

#[test]
fn test_render_icon_is_decorative_by_default() {
    let view = view! { <Icon name=IconName::Sun /> };
    let html = view.to_html();
    assert!(html.contains("data-slot=\"icon\""));
    assert!(html.contains("viewBox=\"0 0 24 24\""));
    assert!(html.contains("aria-hidden=\"true\""));
    assert!(!html.contains("role=\"img\""));
    assert!(!html.contains("aria-label"));
    assert!(html.contains("<path"));
    // Colour comes from the surrounding text class, not from the icon itself.
    assert!(html.contains("stroke=\"currentColor\""));
    assert!(html.contains("fill=\"none\""));
}

#[test]
fn test_render_icon_with_a_label_becomes_an_image() {
    let view = view! { <Icon name=IconName::Users size=20 aria_label="Members" /> };
    let html = view.to_html();
    assert!(html.contains("role=\"img\""));
    assert!(html.contains("aria-label=\"Members\""));
    assert!(!html.contains("aria-hidden"));
    assert!(html.contains("width=\"20\""));
    assert!(html.contains("height=\"20\""));
}

#[test]
fn test_icon_set_is_unique_and_non_empty() {
    // Two icons sharing path data would mean one of them was copied by mistake; an empty path list
    // renders a blank square that no test would otherwise notice.
    let names = [
        IconName::Package,
        IconName::Shield,
        IconName::Check,
        IconName::ChevronDown,
        IconName::ChevronLeft,
        IconName::ChevronRight,
        IconName::ChevronUp,
        IconName::Eye,
        IconName::EyeOff,
        IconName::Key,
        IconName::Lock,
        IconName::Moon,
        IconName::PanelLeft,
        IconName::Plus,
        IconName::Search,
        IconName::ShieldCheck,
        IconName::Sun,
        IconName::Terminal,
        IconName::User,
        IconName::Users,
        IconName::X,
    ];
    let mut seen: Vec<&'static [&'static str]> = Vec::new();
    for name in names {
        let paths = name.paths();
        assert!(!paths.is_empty(), "{name:?} has no path data");
        assert!(!seen.contains(&paths), "{name:?} duplicates another icon");
        seen.push(paths);
    }
}
