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
use leptos_ui::{
    Alert, AlertVariant, Badge, BadgeVariant, Button, ButtonVariant, Card, CardContent,
    CardDescription, CardHeader, CardTitle, Size,
};

#[component]
pub fn DesignSystemPage() -> impl IntoView {
    // Interactive playground signals
    let (variant, set_variant) = signal(ButtonVariant::Default);
    let (size, set_size) = signal(Size::Md);
    let (disabled, set_disabled) = signal(false);
    let (loading, set_loading) = signal(false);
    let (button_text, set_button_text) = signal("Save Changes".to_string());

    // Search query signal for filter recipe
    let (search_query, set_search_query) = signal(String::new());
    let (active_filter, set_active_filter) = signal("all".to_string());

    // Save bar signal
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

            // Composite Recipes
            <section class="space-y-6">
                <h2 class="text-xl font-semibold tracking-tight text-foreground">
                    "Composite Pattern Recipes (FFA)"
                </h2>

                <div class="grid gap-6 lg:grid-cols-3">
                    // Recipe 1: Confirm Delete Dialog Mockup
                    <Card>
                        <CardHeader>
                            <CardTitle class="text-base">"Confirm Action Dialog"</CardTitle>
                            <CardDescription>"Pattern for irreversible mutations."</CardDescription>
                        </CardHeader>
                        <CardContent class="space-y-4">
                            <p class="text-sm text-muted-foreground">
                                "Composite dialog layout with cancel and destructive confirmation buttons."
                            </p>
                            <div class="rounded-lg border border-destructive/20 bg-destructive/5 p-4 space-y-3">
                                <h4 class="font-semibold text-sm text-foreground">"Are you sure?"</h4>
                                <p class="text-xs text-muted-foreground">
                                    "This will invalidate cache entries and revoke associated credentials."
                                </p>
                                <div class="flex justify-end gap-2 pt-2">
                                    <Button variant=ButtonVariant::Outline size=Size::Sm>"Cancel"</Button>
                                    <Button variant=ButtonVariant::Destructive size=Size::Sm>"Confirm Delete"</Button>
                                </div>
                            </div>
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
                                <Badge
                                    variant=if active_filter.get() == "all" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                    class="cursor-pointer"
                                    on:click=move |_| set_active_filter.set("all".to_string())
                                >
                                    "All"
                                </Badge>
                                <Badge
                                    variant=if active_filter.get() == "active" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                    class="cursor-pointer"
                                    on:click=move |_| set_active_filter.set("active".to_string())
                                >
                                    "Active"
                                </Badge>
                                <Badge
                                    variant=if active_filter.get() == "draft" { BadgeVariant::Default } else { BadgeVariant::Outline }
                                    class="cursor-pointer"
                                    on:click=move |_| set_active_filter.set("draft".to_string())
                                >
                                    "Draft"
                                </Badge>
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
                                    <Button variant=ButtonVariant::Outline size=Size::Sm>"Reset"</Button>
                                    <Button
                                        size=Size::Sm
                                        loading=is_saving.get()
                                        on_click=Box::new(move || {
                                            set_is_saving.set(true);
                                            set_save_success.set(true);
                                            set_is_saving.set(false);
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
                </div>
            </section>
        </div>
    }
}
