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
use rustok_api::RichTextDocument;
use rustok_grid::{CellValue, ColumnAlign, GridColumnDef};
use rustok_grid_leptos::prelude::DataGrid;
use rustok_ui_leptos::{
    Alert, AlertVariant, Avatar, AvatarSize, Badge, BadgeVariant, Button, ButtonVariant, Card,
    CardContent, CardDescription, CardFooter, CardHeader, CardTitle, Checkbox, Dialog, DialogClose,
    DialogContent, DialogDescription, DialogTitle, DialogTrigger, Input, Orientation, Progress,
    RichTextEditorFrame, Select, SelectOption, Separator, Size, Skeleton, SkeletonVariant, Spinner,
    Switch, TableOfContents, Tabs, TabsContent, TabsList, TabsTrigger, Textarea, TocItem,
    localized_richtext_frame_copy,
};

/// One demo row of the workbench data grid.
///
/// Declared outside the component so the cell renderer and the sort key can share
/// it, and so the grid shows a real (`Clone`) row type instead of strings.
#[derive(Clone, Debug)]
struct ShowcaseGridRow {
    id: String,
    name: String,
    seats: i32,
    status: String,
}

#[component]
pub fn DesignSystemPage() -> impl IntoView {
    // RichText Sandbox signals
    let (rt_profile, set_rt_profile) = signal("article".to_string());
    let (rt_locale, set_rt_locale) = signal("en".to_string());
    let (rt_disabled, set_rt_disabled) = signal(false);
    let (rt_doc, set_rt_doc) = signal(RichTextDocument::empty());
    let rt_copy = localized_richtext_frame_copy(|_, default| default.to_string());

    // Interactive playground signals
    let (variant, set_variant) = signal(ButtonVariant::Default);
    let (size, set_size) = signal(Size::Md);
    let (disabled, set_disabled) = signal(false);
    let (loading, set_loading) = signal(false);
    let (button_text, set_button_text) = signal("Save Changes".to_string());

    // Sections that were missing from this page: selection, overlays, status,
    // identity, navigation and the data grid.
    let showcase_choice = RwSignal::new("starter".to_string());
    let showcase_choice_with_placeholder = RwSignal::new("".to_string());
    let showcase_tab = RwSignal::new("overview".to_string());
    let showcase_dialog_open = RwSignal::new(false);
    let showcase_rows = RwSignal::new(vec![
        ShowcaseGridRow {
            id: "core".to_string(),
            name: "Core workspace".to_string(),
            seats: 24,
            status: "active".to_string(),
        },
        ShowcaseGridRow {
            id: "commerce".to_string(),
            name: "Commerce".to_string(),
            seats: 12,
            status: "active".to_string(),
        },
        ShowcaseGridRow {
            id: "archive".to_string(),
            name: "Archive".to_string(),
            seats: 3,
            status: "read-only".to_string(),
        },
    ]);

    // Form controls interactive signals
    let (form_input, set_form_input) = signal("admin@rustok.dev".to_string());
    let (form_input_invalid, set_form_input_invalid) = signal(false);
    let (form_switch, set_form_switch) = signal(true);
    let (form_checkbox, set_form_checkbox) = signal(true);
    let (form_notes, set_form_notes) =
        signal("Initial telemetry and module configuration notes.".to_string());

    // Recipe signals
    let (confirm_open, set_confirm_open) = signal(false);
    let (deleted_status, set_deleted_status) = signal(false);
    let (search_query, set_search_query) = signal(String::new());
    let (active_filter, set_active_filter) = signal("all".to_string());

    // Save bar signals
    let (is_saving, set_is_saving) = signal(false);
    let (save_success, set_save_success) = signal(false);

    let snippet = move || {
        let v_str = match variant.get() {
            ButtonVariant::Default => "Default",
            ButtonVariant::Destructive => "Destructive",
            ButtonVariant::Outline => "Outline",
            ButtonVariant::Secondary => "Secondary",
            ButtonVariant::Ghost => "Ghost",
            ButtonVariant::Link => "Link",
        };
        let s_str = match size.get() {
            Size::Sm => "Sm",
            Size::Md => "Md",
            Size::Lg => "Lg",
            Size::Icon => "Icon",
            Size::Xs => "Xs",
            Size::Xl => "Xl",
        };
        format!(
            "<Button variant=ButtonVariant::{v_str} size=Size::{s_str} disabled={} loading={}>\n    \"{}\"\n</Button>",
            disabled.get(),
            loading.get(),
            button_text.get()
        )
    };

    view! {
        <div class="flex flex-1 flex-col gap-8 p-6 md:p-10">
            // Header
            <div class="flex flex-col gap-2 border-b border-border pb-6 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <div class="flex items-center gap-2">
                        <h1 class="text-3xl font-bold tracking-tight text-foreground">
                            "UI Workbench"
                        </h1>
                        <Badge variant=BadgeVariant::Outline class="ml-2 font-mono text-xs">
                            "Rust / WASM (FFA)"
                        </Badge>
                    </div>
                    <p class="text-muted-foreground mt-1 text-sm">
                        "Autonomous Leptos component gallery for RusToK. Running natively with zero JS/React dependencies."
                    </p>
                </div>
            </div>

            // Interactive Playground
            <section class="space-y-4">
                <h2 class="text-xl font-semibold tracking-tight text-foreground">
                    "Interactive Button Inspector"
                </h2>
                <div class="grid gap-6 lg:grid-cols-12">
                    // Controls
                    <Card class="lg:col-span-5">
                        <CardHeader>
                            <CardTitle class="text-base">"Props Controls"</CardTitle>
                            <CardDescription>
                                "Modify properties to verify Leptos WASM reactive state rendering."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4 text-sm">
                            <div class="space-y-1.5">
                                <label class="font-medium text-xs text-muted-foreground">"Variant"</label>
                                <div class="flex flex-wrap gap-1.5">
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_variant.set(ButtonVariant::Default))
                                    >
                                        "Default"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_variant.set(ButtonVariant::Destructive))
                                    >
                                        "Destructive"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_variant.set(ButtonVariant::Outline))
                                    >
                                        "Outline"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_variant.set(ButtonVariant::Secondary))
                                    >
                                        "Secondary"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_variant.set(ButtonVariant::Ghost))
                                    >
                                        "Ghost"
                                    </Button>
                                </div>
                            </div>

                            <div class="space-y-1.5">
                                <label class="font-medium text-xs text-muted-foreground">"Size"</label>
                                <div class="flex gap-1.5">
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_size.set(Size::Sm))
                                    >
                                        "Sm"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_size.set(Size::Md))
                                    >
                                        "Md (default)"
                                    </Button>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        on_click=Box::new(move || set_size.set(Size::Lg))
                                    >
                                        "Lg"
                                    </Button>
                                </div>
                            </div>

                            <div class="space-y-1.5">
                                <label class="font-medium text-xs text-muted-foreground">"Button Label"</label>
                                <input
                                    type="text"
                                    prop:value=button_text
                                    on:input=move |ev| set_button_text.set(event_target_value(&ev))
                                    class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                                />
                            </div>

                            <div class="flex items-center justify-between border-t border-border pt-3">
                                <span class="font-medium text-xs text-foreground">"Disabled State"</span>
                                <Button
                                    variant=ButtonVariant::Outline
                                    size=Size::Sm
                                    on_click=Box::new(move || set_disabled.update(|d| *d = !*d))
                                >
                                    {move || if disabled.get() { "Enabled: OFF" } else { "Enabled: ON" }}
                                </Button>
                            </div>

                            <div class="flex items-center justify-between">
                                <span class="font-medium text-xs text-foreground">"Loading Spinner"</span>
                                <Button
                                    variant=ButtonVariant::Outline
                                    size=Size::Sm
                                    on_click=Box::new(move || set_loading.update(|l| *l = !*l))
                                >
                                    {move || if loading.get() { "Loading: ON" } else { "Loading: OFF" }}
                                </Button>
                            </div>
                        </CardContent>
                    </Card>

                    // Live Preview
                    <Card class="flex flex-col lg:col-span-7">
                        <CardHeader>
                            <CardTitle class="text-base">"Live Preview"</CardTitle>
                            <CardDescription>
                                "Native Leptos rendering with shared RusToK Tailwind tokens."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="flex min-h-[160px] flex-1 items-center justify-center rounded-lg border border-dashed border-border bg-muted/20 p-6">
                            {move || {
                                view! {
                                    <Button
                                        variant=variant.get()
                                        size=size.get()
                                        disabled=disabled.get()
                                        loading=loading.get()
                                    >
                                        {button_text.get()}
                                    </Button>
                                }
                            }}
                        </CardContent>
                        <div class="flex flex-col gap-2 border-t border-border bg-muted/10 p-4">
                            <span class="text-xs font-semibold text-muted-foreground uppercase">
                                "Leptos Code Snippet"
                            </span>
                            <pre class="overflow-x-auto rounded-md bg-slate-950 p-3 text-xs text-slate-100 dark:bg-black font-mono">
                                <code>{snippet}</code>
                            </pre>
                        </div>
                    </Card>
                </div>
            </section>

            // Component Baseline Matrix
            <section class="space-y-6">
                <h2 class="text-xl font-semibold tracking-tight text-foreground">
                    "Component Baseline Matrix"
                </h2>

                // Buttons
                <Card>
                    <CardHeader>
                        <CardTitle class="text-base">"Button Variants & Sizes"</CardTitle>
                        <CardDescription>
                            "Visual parity verified against shadcn CSS custom property specifications."
                        </CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-6">
                        <div class="flex flex-wrap items-center gap-3">
                            <Button variant=ButtonVariant::Default>"Default"</Button>
                            <Button variant=ButtonVariant::Secondary>"Secondary"</Button>
                            <Button variant=ButtonVariant::Destructive>"Destructive"</Button>
                            <Button variant=ButtonVariant::Outline>"Outline"</Button>
                            <Button variant=ButtonVariant::Ghost>"Ghost"</Button>
                            <Button variant=ButtonVariant::Link>"Link"</Button>
                        </div>
                        <div class="flex flex-wrap items-center gap-3">
                            <Button size=Size::Sm>"Small (sm)"</Button>
                            <Button size=Size::Md>"Medium (default)"</Button>
                            <Button size=Size::Lg>"Large (lg)"</Button>
                            <Button disabled=true>"Disabled"</Button>
                        </div>
                    </CardContent>
                </Card>

                // Badges
                <Card>
                    <CardHeader>
                        <CardTitle class="text-base">"Badge Variants"</CardTitle>
                        <CardDescription>"Semantic tags and status pills."</CardDescription>
                    </CardHeader>
                    <CardContent class="flex flex-wrap gap-3">
                        <Badge variant=BadgeVariant::Default>"Default"</Badge>
                        <Badge variant=BadgeVariant::Secondary>"Secondary"</Badge>
                        <Badge variant=BadgeVariant::Destructive>"Destructive"</Badge>
                        <Badge variant=BadgeVariant::Outline>"Outline"</Badge>
                        <Badge variant=BadgeVariant::Success>"Success"</Badge>
                        <Badge variant=BadgeVariant::Warning>"Warning"</Badge>
                    </CardContent>
                </Card>

                // Alerts
                <div class="grid gap-6 md:grid-cols-2">
                    <Alert variant=AlertVariant::Default title="System Information">
                        "Standard information notification across admin workflows."
                    </Alert>
                    <Alert variant=AlertVariant::Destructive title="Destructive Action Warning">
                        "Warning message for critical actions and domain guardrails."
                    </Alert>
                </div>
            </section>

            <Separator />

            // Form Controls & Inputs
            <section class="space-y-6">
                <h2 class="text-xl font-semibold tracking-tight text-foreground">
                    "Form Controls & Inputs"
                </h2>
                <div class="grid gap-6 md:grid-cols-2">
                    // Text Inputs & Textarea
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Text Fields"</CardTitle>
                            <CardDescription>
                                "Single-line Input and multi-line Textarea controls with live reactive signals."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <div class="space-y-1.5">
                                <div class="flex items-center justify-between">
                                    <label class="font-medium text-xs text-muted-foreground">"Primary Email"</label>
                                    <button
                                        type="button"
                                        class="text-xs text-primary hover:underline cursor-pointer"
                                        on:click=move |_| set_form_input_invalid.update(|v| *v = !*v)
                                    >
                                        {move || if form_input_invalid.get() { "Clear Error" } else { "Simulate Error" }}
                                    </button>
                                </div>
                                {move || view! {
                                    <Input
                                        placeholder="admin@rustok.dev"
                                        value=form_input
                                        set_value=set_form_input
                                        invalid=form_input_invalid.get()
                                    />
                                }}
                                {move || form_input_invalid.get().then(|| view! {
                                    <p class="text-xs text-destructive">"Invalid email format: must belong to authorized tenant domain."</p>
                                })}
                            </div>

                            <div class="space-y-1.5">
                                <label class="font-medium text-xs text-muted-foreground">"Configuration Notes"</label>
                                <Textarea
                                    rows=3
                                    placeholder="Enter operational notes..."
                                    value=form_notes
                                    set_value=set_form_notes
                                />
                            </div>

                            <div class="space-y-1.5">
                                <label class="font-medium text-xs text-muted-foreground">"Disabled Identifier"</label>
                                <Input
                                    disabled=true
                                    placeholder="System managed identifier"
                                    value=signal("mod_core_01jx".to_string()).0
                                />
                            </div>
                        </CardContent>
                    </Card>

                    // Switches & Checkboxes
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Toggles & Checkboxes"</CardTitle>
                            <CardDescription>
                                "Binary selection controls connected to reactive WASM signals."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-6">
                            <div class="flex items-center justify-between rounded-lg border border-border p-3">
                                <div class="space-y-0.5">
                                    <span class="text-sm font-medium text-foreground">"Telemetry Streaming"</span>
                                    <p class="text-xs text-muted-foreground">
                                        "Stream real-time trace events to OpenTelemetry endpoint."
                                    </p>
                                </div>
                                <div class="flex items-center gap-3">
                                    <span class="text-xs font-mono text-muted-foreground">
                                        {move || if form_switch.get() { "ACTIVE" } else { "OFF" }}
                                    </span>
                                    <Switch
                                        checked=form_switch
                                        set_checked=set_form_switch
                                    />
                                </div>
                            </div>

                            <div class="flex items-center justify-between rounded-lg border border-border p-3">
                                <div class="space-y-0.5">
                                    <span class="text-sm font-medium text-foreground">"Automatic Re-indexing"</span>
                                    <p class="text-xs text-muted-foreground">
                                        "Update full-text search projection on entity mutations."
                                    </p>
                                </div>
                                <div class="flex items-center gap-3">
                                    <span class="text-xs font-mono text-muted-foreground">
                                        {move || if form_checkbox.get() { "CHECKED" } else { "UNCHECKED" }}
                                    </span>
                                    <Checkbox
                                        checked=form_checkbox
                                        set_checked=set_form_checkbox
                                    />
                                </div>
                            </div>

                            <div class="flex items-center gap-2 pt-2 text-xs text-muted-foreground">
                                <Badge variant=BadgeVariant::Outline>"FFA State"</Badge>
                                <span>"Both controls use native Rust signals with zero external JS runtime."</span>
                            </div>
                        </CardContent>
                    </Card>
                </div>
            </section>

            <Separator />

            // Composite Recipes
            <section class="space-y-6">
                <h2 class="text-xl font-semibold tracking-tight text-foreground">
                    "Composite Pattern Recipes (FFA)"
                </h2>

                <div class="grid gap-6 lg:grid-cols-2 xl:grid-cols-4">
                    // Recipe 1: Confirm Delete Dialog
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Confirm Action Dialog"</CardTitle>
                            <CardDescription>"Pattern for irreversible mutations."</CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <p class="text-sm text-muted-foreground">
                                "Composite dialog layout with cancel and destructive confirmation buttons."
                            </p>
                            {move || {
                                if confirm_open.get() {
                                    view! {
                                        <div class="rounded-lg border border-destructive/20 bg-destructive/5 p-4 space-y-3">
                                            <h4 class="font-semibold text-sm text-foreground">"Are you sure?"</h4>
                                            <p class="text-xs text-muted-foreground">
                                                "This will invalidate cache entries and revoke associated credentials."
                                            </p>
                                            <div class="flex justify-end gap-2 pt-2">
                                                <Button
                                                    variant=ButtonVariant::Outline
                                                    size=Size::Sm
                                                    on_click=Box::new(move || set_confirm_open.set(false))
                                                >
                                                    "Cancel"
                                                </Button>
                                                <Button
                                                    variant=ButtonVariant::Destructive
                                                    size=Size::Sm
                                                    on_click=Box::new(move || {
                                                        set_confirm_open.set(false);
                                                        set_deleted_status.set(true);
                                                    })
                                                >
                                                    "Confirm Delete"
                                                </Button>
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-2">
                                            <Button
                                                variant=ButtonVariant::Destructive
                                                size=Size::Sm
                                                on_click=Box::new(move || {
                                                    set_deleted_status.set(false);
                                                    set_confirm_open.set(true);
                                                })
                                            >
                                                "Delete Module"
                                            </Button>
                                            {move || {
                                                deleted_status.get().then(|| {
                                                    view! {
                                                        <div class="rounded-md bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-400">
                                                            "Module deleted successfully."
                                                        </div>
                                                    }
                                                })
                                            }}
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </CardContent>
                    </Card>

                    // Recipe 2: Search & Filter Toolbar
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Resource Filter Toolbar"</CardTitle>
                            <CardDescription>"Compact search and status filters for grids."</CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-3">
                            <input
                                type="text"
                                placeholder="Filter resources..."
                                prop:value=search_query
                                on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                            />
                            <div class="flex gap-1.5">
                                <button
                                    type="button"
                                    class="focus:outline-none"
                                    on:click=move |_| set_active_filter.set("all".to_string())
                                >
                                    <Badge
                                        variant=if active_filter.get() == "all" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                        class="cursor-pointer"
                                    >
                                        "All"
                                    </Badge>
                                </button>
                                <button
                                    type="button"
                                    class="focus:outline-none"
                                    on:click=move |_| set_active_filter.set("active".to_string())
                                >
                                    <Badge
                                        variant=if active_filter.get() == "active" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                        class="cursor-pointer"
                                    >
                                        "Active"
                                    </Badge>
                                </button>
                                <button
                                    type="button"
                                    class="focus:outline-none"
                                    on:click=move |_| set_active_filter.set("draft".to_string())
                                >
                                    <Badge
                                        variant=if active_filter.get() == "draft" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                        class="cursor-pointer"
                                    >
                                        "Draft"
                                    </Badge>
                                </button>
                            </div>
                        </CardContent>
                    </Card>

                    // Recipe 3: Save Action Toolbar
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Save Action Toolbar"</CardTitle>
                            <CardDescription>"Form mutation bar with dirty/saved state feedback."</CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <div class="flex items-center justify-between rounded-lg border border-border bg-muted/40 p-3">
                                <div class="flex items-center gap-2">
                                    <span class="h-2 w-2 rounded-full bg-amber-500"></span>
                                    <span class="text-xs font-medium text-foreground">"Unsaved changes"</span>
                                </div>
                                <div class="flex gap-2">
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=Size::Sm
                                        disabled=is_saving.get()
                                        on_click=Box::new(move || {
                                            set_save_success.set(false);
                                        })
                                    >
                                        "Reset"
                                    </Button>
                                    <Button
                                        size=Size::Sm
                                        loading=is_saving.get()
                                        on_click=Box::new(move || {
                                            set_is_saving.set(true);
                                            set_save_success.set(false);
                                            leptos::prelude::set_timeout(
                                                move || {
                                                    set_is_saving.set(false);
                                                    set_save_success.set(true);
                                                },
                                                std::time::Duration::from_millis(800),
                                            );
                                        })
                                    >
                                        "Save"
                                    </Button>
                                </div>
                            </div>
                            {move || {
                                save_success.get().then(|| {
                                    view! {
                                        <div class="rounded-md bg-green-500/10 p-2 text-xs text-green-700 dark:text-green-400">
                                            "Changes saved successfully!"
                                        </div>
                                    }
                                })
                            }}
                        </CardContent>
                    </Card>

                    // Recipe 4: Entity Summary Card
                    <Card>
                        <CardHeader>
                            <div class="flex items-start justify-between">
                                <div class="space-y-1">
                                    <CardTitle class="text-base">"RusToK Core"</CardTitle>
                                    <CardDescription>"E-Commerce Engine"</CardDescription>
                                </div>
                                <Badge variant=BadgeVariant::Success class="font-mono text-xs">
                                    "Healthy"
                                </Badge>
                            </div>
                        </CardHeader>
                        <CardContent class="space-y-3">
                            <p class="text-xs text-muted-foreground">
                                "High-throughput transactional core managing inventory, checkout, and tenant boundaries."
                            </p>
                            <div class="grid grid-cols-2 gap-2 rounded-md bg-muted/40 p-2 text-xs">
                                <div>
                                    <span class="text-muted-foreground block">"Version"</span>
                                    <span class="font-semibold text-foreground">"v0.8.21"</span>
                                </div>
                                <div>
                                    <span class="text-muted-foreground block">"Uptime"</span>
                                    <span class="font-semibold text-foreground">"99.98%"</span>
                                </div>
                            </div>
                        </CardContent>
                        <CardFooter class="flex justify-between border-t border-border pt-3">
                            <Button variant=ButtonVariant::Outline size=Size::Sm>
                                "Logs"
                            </Button>
                            <Button size=Size::Sm>
                                "Manage"
                            </Button>
                        </CardFooter>
                    </Card>
                </div>
            </section>

            // RichText Editor Sandbox
            <section class="space-y-4">
                <div class="flex items-center gap-2">
                    <h2 class="text-xl font-semibold tracking-tight text-foreground">
                        "RichText Editor Sandbox"
                    </h2>
                    <Badge variant=BadgeVariant::Outline class="ml-2 font-mono text-xs">
                        "Isolated Frame"
                    </Badge>
                </div>
                <Card>
                    <CardHeader>
                        <div class="flex flex-wrap items-center justify-between gap-4">
                            <div>
                                <CardTitle class="text-base">"Interactive Editor"</CardTitle>
                                <CardDescription>
                                    "Sandboxed iframe editor communicating via MessageChannel with heartbeat and autosave."
                                </CardDescription>
                            </div>
                            <div class="flex flex-wrap items-center gap-3">
                                <div class="flex items-center gap-1.5">
                                    <span class="text-xs font-medium text-muted-foreground">"Profile:"</span>
                                    {move || {
                                        let current = rt_profile.get();
                                        ["article", "discussion", "comment"].into_iter().map(|p| {
                                            let p_str = p.to_string();
                                            let is_active = current == p;
                                            let variant = if is_active { ButtonVariant::Default } else { ButtonVariant::Outline };
                                            view! {
                                                <Button
                                                    variant=variant
                                                    size=Size::Sm
                                                    on_click=Box::new(move || set_rt_profile.set(p_str.clone()))
                                                    class="capitalize h-7 px-2.5 text-xs"
                                                >
                                                    {p}
                                                </Button>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                                <Separator orientation=Orientation::Vertical class="h-4" />
                                <div class="flex items-center gap-1.5">
                                    <span class="text-xs font-medium text-muted-foreground">"Locale:"</span>
                                    {move || {
                                        let current = rt_locale.get();
                                        ["en", "ru"].into_iter().map(|loc| {
                                            let loc_str = loc.to_string();
                                            let is_active = current == loc;
                                            let variant = if is_active { ButtonVariant::Default } else { ButtonVariant::Outline };
                                            view! {
                                                <Button
                                                    variant=variant
                                                    size=Size::Sm
                                                    on_click=Box::new(move || set_rt_locale.set(loc_str.clone()))
                                                    class="uppercase h-7 px-2.5 text-xs"
                                                >
                                                    {loc}
                                                </Button>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                                <Separator orientation=Orientation::Vertical class="h-4" />
                                <label class="flex items-center gap-1.5 text-xs font-medium cursor-pointer text-foreground">
                                    <Switch
                                        checked=rt_disabled
                                        set_checked=set_rt_disabled
                                    />
                                    <span>"Disabled"</span>
                                </label>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        <div class="rounded-md border border-border p-4 bg-background">
                            {move || {
                                let p = rt_profile.get();
                                view! {
                                    <RichTextEditorFrame
                                        document=rt_doc
                                        set_document=set_rt_doc
                                        content_locale=rt_locale.into()
                                        label=format!("RichText Document ({p})")
                                        profile=p
                                        copy=rt_copy.clone()
                                        disabled=rt_disabled.into()
                                    />
                                }
                            }}
                        </div>
                        <div class="rounded-md bg-muted/50 p-3">
                            <div class="text-xs font-medium text-muted-foreground mb-1">"Document JSON State:"</div>
                            <pre class="text-[10px] font-mono overflow-auto max-h-36 p-2 bg-background rounded border border-border text-foreground">
                                {move || serde_json::to_string_pretty(&rt_doc.get()).unwrap_or_default()}
                            </pre>
                        </div>
                    </CardContent>
                </Card>
            </section>

            // Selection, overlays and controlled state
            <section class="space-y-4">
                <div class="flex items-center gap-2">
                    <h2 class="text-xl font-semibold tracking-tight text-foreground">
                        "Selection & Overlays"
                    </h2>
                    <Badge variant=BadgeVariant::Outline class="ml-2 font-mono text-xs">
                        "Select · Tabs · Dialog"
                    </Badge>
                </div>
                <div class="grid gap-4 lg:grid-cols-3">
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Select"</CardTitle>
                            <CardDescription>
                                "Controlled listbox: the selected value lives in the host signal."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-3">
                            <Select
                                aria_label="Showcase workspace"
                                options=vec![
                                    SelectOption::new("starter", "Starter"),
                                    SelectOption::new("growth", "Growth"),
                                    SelectOption::new("enterprise", "Enterprise"),
                                ]
                                value=showcase_choice.read_only()
                                set_value=showcase_choice.write_only()
                            />
                            <Select
                                aria_label="Showcase plan with placeholder"
                                placeholder="Choose a plan"
                                options=vec![
                                    SelectOption::new("monthly", "Monthly"),
                                    SelectOption::new("yearly", "Yearly"),
                                ]
                                value=showcase_choice_with_placeholder.read_only()
                                set_value=showcase_choice_with_placeholder.write_only()
                            />
                            <p class="text-xs text-muted-foreground">
                                "Selected: "
                                <span class="font-mono">{move || showcase_choice.get()}</span>
                            </p>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Tabs"</CardTitle>
                            <CardDescription>
                                "Roving tablist; each panel is a real tabpanel for assistive tech."
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            {move || {
                                let active = showcase_tab.get();
                                let is_overview = active == "overview";
                                let is_activity = active == "activity";
                                view! {
                                    <Tabs default_value=active.clone()>
                                        <TabsList aria_label="Showcase tabs">
                                            <TabsTrigger
                                                value="overview".to_string()
                                                active=is_overview
                                                on_select=Callback::new(move |()| showcase_tab.set("overview".to_string()))
                                            >
                                                "Overview"
                                            </TabsTrigger>
                                            <TabsTrigger
                                                value="activity".to_string()
                                                active=is_activity
                                                on_select=Callback::new(move |()| showcase_tab.set("activity".to_string()))
                                            >
                                                "Activity"
                                            </TabsTrigger>
                                        </TabsList>
                                        <TabsContent value="overview".to_string() active=is_overview class="pt-3 text-sm text-muted-foreground">
                                            "Workspace summary, plan limits and the current billing period."
                                        </TabsContent>
                                        <TabsContent value="activity".to_string() active=is_activity class="pt-3 text-sm text-muted-foreground">
                                            "Audit trail of the last 30 days, newest first."
                                        </TabsContent>
                                    </Tabs>
                                }
                            }}
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Dialog"</CardTitle>
                            <CardDescription>
                                "Modal dialog with focus trap, overlay and an explicit close action."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-3">
                            <Dialog
                                open=Signal::derive(move || showcase_dialog_open.get())
                                on_open_change=Callback::new(move |open: bool| showcase_dialog_open.set(open))
                            >
                                <DialogTrigger
                                    aria_controls="ds-showcase-dialog"
                                    class="inline-flex h-10 items-center justify-center rounded-lg border border-border bg-background px-4 text-sm font-medium text-foreground transition hover:bg-muted"
                                >
                                    "Open dialog"
                                </DialogTrigger>
                                <DialogContent id="ds-showcase-dialog" aria_labelledby="ds-showcase-dialog-title">
                                    <DialogTitle id="ds-showcase-dialog-title">
                                        "Delete the archive workspace?"
                                    </DialogTitle>
                                    <DialogDescription>
                                        "Every page, target and binding in this workspace goes away. The action cannot be undone."
                                    </DialogDescription>
                                    <div class="mt-4 flex justify-end gap-2">
                                        <DialogClose class="inline-flex h-10 items-center justify-center rounded-lg border border-border px-4 text-sm font-medium text-foreground transition hover:bg-muted">
                                            "Cancel"
                                        </DialogClose>
                                        <Button
                                            variant=ButtonVariant::Destructive
                                            on_click=Box::new(move || showcase_dialog_open.set(false))
                                        >
                                            "Delete workspace"
                                        </Button>
                                    </div>
                                </DialogContent>
                            </Dialog>
                            <p class="text-xs text-muted-foreground">
                                "State: "
                                <span class="font-mono">
                                    {move || if showcase_dialog_open.get() { "open" } else { "closed" }}
                                </span>
                            </p>
                        </CardContent>
                    </Card>
                </div>
            </section>

            // Status, identity and navigation
            <section class="space-y-4">
                <div class="flex items-center gap-2">
                    <h2 class="text-xl font-semibold tracking-tight text-foreground">
                        "Status, Identity & Navigation"
                    </h2>
                    <Badge variant=BadgeVariant::Outline class="ml-2 font-mono text-xs">
                        "Spinner · Progress · Skeleton · Avatar · Toc"
                    </Badge>
                </div>
                <div class="grid gap-4 lg:grid-cols-3">
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Loading"</CardTitle>
                            <CardDescription>
                                "Spinner sizes, determinate and indeterminate progress."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <div class="flex items-end gap-4">
                                <Spinner size=Size::Sm aria_label="Small spinner" />
                                <Spinner size=Size::Md aria_label="Medium spinner" />
                                <Spinner size=Size::Lg aria_label="Large spinner" />
                            </div>
                            <Progress aria_label="Upload progress" value=Some(62.0) max=Some(100.0) />
                            <Progress aria_label="Indeterminate progress" />
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Skeleton & Avatar"</CardTitle>
                            <CardDescription>
                                "Placeholders keep the layout from jumping while data loads."
                            </CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <Skeleton />
                            <Skeleton variant=SkeletonVariant::Rectangular class="h-16 w-full" />
                            <div class="flex items-center gap-3">
                                <Skeleton variant=SkeletonVariant::Circular class="h-10 w-10" />
                                <Skeleton class="h-4 w-32" />
                            </div>
                            <div class="flex items-center gap-3">
                                <Avatar fallback="RT".to_string() alt="RusToK" size=AvatarSize::Xs />
                                <Avatar fallback="RT".to_string() alt="RusToK" size=AvatarSize::Sm />
                                <Avatar fallback="RT".to_string() alt="RusToK" size=AvatarSize::Md />
                                <Avatar fallback="RT".to_string() alt="RusToK" size=AvatarSize::Lg />
                            </div>
                        </CardContent>
                    </Card>
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Table of contents"</CardTitle>
                            <CardDescription>
                                "Generated from headings, with the depth the caller asks for."
                            </CardDescription>
                        </CardHeader>
                        <CardContent>
                            <TableOfContents
                                title="On this page".to_string()
                                sticky=false
                                items=vec![
                                    TocItem::new("showcase-usage", "Usage", 2),
                                    TocItem::new("showcase-api", "API", 2),
                                    TocItem::new("showcase-props", "Props", 3),
                                ]
                            />
                        </CardContent>
                    </Card>
                </div>
            </section>

            // Data grid: the surface that every module page reuses
            <section class="space-y-4">
                <div class="flex items-center gap-2">
                    <h2 class="text-xl font-semibold tracking-tight text-foreground">
                        "Data Grid"
                    </h2>
                    <Badge variant=BadgeVariant::Outline class="ml-2 font-mono text-xs">
                        "DataGrid · sort · empty state"
                    </Badge>
                </div>
                <Card>
                    <CardHeader>
                        <CardTitle class="text-base">"Sortable grid with a typed cell renderer"</CardTitle>
                        <CardDescription>
                            "Client-side ordering comes from `sort_cell`, so a header click sorts the rows the host already loaded."
                        </CardDescription>
                    </CardHeader>
                    <CardContent class="space-y-4">
                        {
                            let columns = vec![
                                GridColumnDef::new("name", "Workspace").align(ColumnAlign::Left),
                                GridColumnDef::new("seats", "Seats").align(ColumnAlign::Right),
                                GridColumnDef::new("status", "Status").align(ColumnAlign::Left),
                            ];
                            let cell_renderer = Callback::new(
                                move |(row, column): (ShowcaseGridRow, String)| match column.as_str() {
                                    "seats" => view! { <span class="tabular-nums">{row.seats}</span> }.into_any(),
                                    "status" => view! {
                                        <Badge variant=BadgeVariant::Outline class="font-mono text-xs">
                                            {row.status}
                                        </Badge>
                                    }
                                    .into_any(),
                                    _ => view! { <span class="font-medium">{row.name}</span> }.into_any(),
                                },
                            );
                            let sort_cell = Callback::new(
                                move |(row, column): (ShowcaseGridRow, String)| match column.as_str() {
                                    "seats" => CellValue::number(f64::from(row.seats)),
                                    "status" => CellValue::text(row.status),
                                    _ => CellValue::text(row.name),
                                },
                            );
                            view! {
                                <DataGrid
                                    aria_label="Showcase workspace grid".to_string()
                                    columns=columns
                                    data=Signal::derive(move || showcase_rows.get())
                                    key_fn=|row: &ShowcaseGridRow| row.id.clone()
                                    cell_renderer=cell_renderer
                                    sort_cell=sort_cell
                                />
                            }
                        }
                        <div class="grid gap-2">
                            <div class="flex items-center gap-2">
                                <Button
                                    variant=ButtonVariant::Outline
                                    size=Size::Sm
                                    on_click=Box::new(move || showcase_rows.update(|rows| rows.clear()))
                                >
                                    "Show the empty state"
                                </Button>
                                <Button
                                    variant=ButtonVariant::Ghost
                                    size=Size::Sm
                                    on_click=Box::new(move || {
                                        showcase_rows.set(vec![ShowcaseGridRow {
                                            id: "restored".to_string(),
                                            name: "Restored workspace".to_string(),
                                            seats: 1,
                                            status: "active".to_string(),
                                        }])
                                    })
                                >
                                    "Restore a row"
                                </Button>
                            </div>
                            <DataGrid
                                aria_label="Showcase empty grid".to_string()
                                columns=vec![
                                    GridColumnDef::new("name", "Workspace"),
                                    GridColumnDef::new("seats", "Seats"),
                                ]
                                data=Signal::derive(Vec::<ShowcaseGridRow>::new)
                                key_fn=|row: &ShowcaseGridRow| row.id.clone()
                                cell_renderer=Callback::new(move |(row, column): (ShowcaseGridRow, String)| {
                                    match column.as_str() {
                                        "seats" => view! { <span class="tabular-nums">{row.seats}</span> }.into_any(),
                                        _ => view! { <span class="font-medium">{row.name}</span> }.into_any(),
                                    }
                                })
                                empty_message="No workspaces yet".to_string()
                            />
                        </div>
                    </CardContent>
                </Card>
            </section>
        </div>
    }
}
