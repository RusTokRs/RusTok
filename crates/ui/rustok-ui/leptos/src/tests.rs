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
use crate::input::*;
use crate::progress::*;
use crate::skeleton::*;
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

    let is_ind = Signal::derive(|| true);
    let view_reactive = view! {
        <Checkbox indeterminate=is_ind />
    };
    let html_reactive = view_reactive.to_html();
    assert!(html_reactive.contains("type=\"checkbox\""));
}

#[test]
fn test_render_input_ssr() {
    let view_plain = view! {
        <Input placeholder="Enter username" />
    };
    let html_plain = view_plain.to_html();
    assert!(html_plain.contains("placeholder=\"Enter username\""));
    assert!(!html_plain.contains("pl-9"));

    let view_adorned = view! {
        <Input
            placeholder="Search..."
            prefix=view! { <span>"🔍"</span> }.into_any()
            suffix=view! { <span>"Clear"</span> }.into_any()
        />
    };
    let html_adorned = view_adorned.to_html();
    assert!(html_adorned.contains("pl-9"));
    assert!(html_adorned.contains("pr-9"));
    assert!(html_adorned.contains("🔍"));
    assert!(html_adorned.contains("Clear"));
}
