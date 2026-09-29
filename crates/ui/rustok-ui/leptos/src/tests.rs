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
use crate::dialog::*;
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
fn test_render_dialog_ssr() {
    let view_closed = view! {
        <Dialog open=false>
            <DialogTitle>"Closed"</DialogTitle>
        </Dialog>
    };
    assert!(!view_closed.to_html().contains("role=\"dialog\""));

    let view_open = view! {
        <Dialog open=true>
            <DialogTitle>"Title Open"</DialogTitle>
        </Dialog>
    };
    let html_open = view_open.to_html();
    assert!(html_open.contains("role=\"dialog\""));
    assert!(html_open.contains("Title Open"));
}
