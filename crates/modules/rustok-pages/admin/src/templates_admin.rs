use crate::core::{
    self, TemplateValidationError, SymbolValidationError,
};
use crate::transport;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplatesTab {
    Templates,
    Symbols,
}

#[component]
pub fn TemplatesAdmin(default_locale: String) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let active_tab = RwSignal::new(TemplatesTab::Templates);
    let selected_locale = RwSignal::new(default_locale);
    let refresh_generation = RwSignal::new(0_u64);
    let feedback = RwSignal::new(None::<(bool, String)>);

    // Template form state
    let template_key = RwSignal::new(String::new());
    let template_header_symbols = RwSignal::new(String::new());
    let template_footer_symbols = RwSignal::new(String::new());
    let template_expected_revision = RwSignal::new(None::<i64>);
    let template_is_editing = RwSignal::new(false);
    let template_busy = RwSignal::new(false);

    // Symbol form state
    let symbol_id = RwSignal::new(String::new());
    let symbol_name = RwSignal::new(String::new());
    let symbol_content_json = RwSignal::new(core::default_symbol_content_json("Header"));
    let symbol_is_editing = RwSignal::new(false);
    let symbol_busy = RwSignal::new(false);
    let delete_dialog_target = RwSignal::new(None::<(String, Option<core::SiteSymbolUsageInfo>, bool)>);
    let delete_busy = RwSignal::new(false);

    let templates_token = token;
    let templates_tenant = tenant;
    let templates_locale = selected_locale;
    let templates_resource = LocalResource::new(move || {
        let token = templates_token.get();
        let tenant = templates_tenant.get();
        let locale = templates_locale.get();
        let _generation = refresh_generation.get();
        async move { transport::fetch_page_templates(token, tenant, locale).await }
    });

    let symbols_token = token;
    let symbols_tenant = tenant;
    let symbols_locale = selected_locale;
    let symbols_resource = LocalResource::new(move || {
        let token = symbols_token.get();
        let tenant = symbols_tenant.get();
        let locale = symbols_locale.get();
        let _generation = refresh_generation.get();
        async move { transport::fetch_site_symbols(token, tenant, locale).await }
    });

    let reset_template_form = move || {
        template_key.set(String::new());
        template_header_symbols.set(String::new());
        template_footer_symbols.set(String::new());
        template_expected_revision.set(None);
        template_is_editing.set(false);
    };

    let reset_symbol_form = move || {
        symbol_id.set(String::new());
        symbol_name.set(String::new());
        symbol_content_json.set(core::default_symbol_content_json("Header"));
        symbol_is_editing.set(false);
    };

    let save_template_action = move |event: SubmitEvent| {
        event.prevent_default();
        if template_busy.get_untracked() {
            return;
        }

        let raw_key = template_key.get_untracked();
        let validated_key = match core::validate_template_key(&raw_key) {
            Ok(key) => key,
            Err(TemplateValidationError::EmptyKey) => {
                feedback.set(Some((true, "Template key cannot be empty".to_string())));
                return;
            }
            Err(TemplateValidationError::InvalidCharacters) => {
                feedback.set(Some((
                    true,
                    "Template key must contain only alphanumeric characters, dashes, and underscores".to_string(),
                )));
                return;
            }
            Err(TemplateValidationError::KeyTooLong) => {
                feedback.set(Some((true, "Template key must not exceed 64 characters".to_string())));
                return;
            }
        };

        let loc = selected_locale.get_untracked();
        let header_ids = core::parse_symbol_id_list(&template_header_symbols.get_untracked());
        let footer_ids = core::parse_symbol_id_list(&template_footer_symbols.get_untracked());
        let exp_rev = template_expected_revision.get_untracked();
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();

        template_busy.set(true);
        feedback.set(None);

        spawn_local(async move {
            match transport::save_page_template(
                tok,
                ten,
                validated_key.clone(),
                loc,
                header_ids,
                footer_ids,
                exp_rev,
            )
            .await
            {
                Ok(saved) => {
                    feedback.set(Some((
                        false,
                        format!("Template `{}` saved successfully (revision {})", saved.key, saved.revision),
                    )));
                    reset_template_form();
                    refresh_generation.update(|g| *g = g.wrapping_add(1));
                }
                Err(err) => {
                    feedback.set(Some((true, format!("Failed to save template: {err}"))));
                }
            }
            template_busy.set(false);
        });
    };

    let save_symbol_action = move |event: SubmitEvent| {
        event.prevent_default();
        if symbol_busy.get_untracked() {
            return;
        }

        let raw_id = symbol_id.get_untracked();
        let validated_id = match core::validate_symbol_id(&raw_id) {
            Ok(id) => id,
            Err(SymbolValidationError::EmptyId) => {
                feedback.set(Some((true, "Symbol ID cannot be empty".to_string())));
                return;
            }
            Err(SymbolValidationError::InvalidCharacters) => {
                feedback.set(Some((
                    true,
                    "Symbol ID must contain only alphanumeric characters, dashes, and underscores".to_string(),
                )));
                return;
            }
            Err(SymbolValidationError::IdTooLong) => {
                feedback.set(Some((true, "Symbol ID must not exceed 64 characters".to_string())));
                return;
            }
            Err(_) => {
                feedback.set(Some((true, "Invalid symbol ID".to_string())));
                return;
            }
        };

        let raw_json = symbol_content_json.get_untracked();
        if let Err(err) = core::validate_symbol_content_json(&raw_json) {
            let msg = match err {
                SymbolValidationError::InvalidJson(msg) => format!("Invalid JSON: {msg}"),
                SymbolValidationError::RootNotObject => "Symbol content JSON root must be an object".to_string(),
                _ => "Invalid symbol content".to_string(),
            };
            feedback.set(Some((true, msg)));
            return;
        }

        let name_val = {
            let n = symbol_name.get_untracked().trim().to_string();
            if n.is_empty() { None } else { Some(n) }
        };
        let loc = selected_locale.get_untracked();
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();

        symbol_busy.set(true);
        feedback.set(None);

        spawn_local(async move {
            match transport::save_site_symbol(
                tok,
                ten,
                validated_id.clone(),
                loc,
                name_val,
                raw_json,
            )
            .await
            {
                Ok(saved) => {
                    feedback.set(Some((
                        false,
                        format!("Symbol `{}` saved successfully", saved.symbol_id),
                    )));
                    reset_symbol_form();
                    refresh_generation.update(|g| *g = g.wrapping_add(1));
                }
                Err(err) => {
                    feedback.set(Some((true, format!("Failed to save symbol: {err}"))));
                }
            }
            symbol_busy.set(false);
        });
    };

    view! {
        <div class="space-y-6">
            // Feedback notification
            {move || {
                feedback.get().map(|(is_error, msg)| {
                    let border_class = if is_error { "border-destructive/30 bg-destructive/10 text-destructive" } else { "border-emerald-500/30 bg-emerald-500/10 text-emerald-800 dark:text-emerald-300" };
                    view! {
                        <div class=format!("flex items-center justify-between rounded-xl border p-4 text-sm {border_class}") role="alert">
                            <span>{msg}</span>
                            <button
                                type="button"
                                class="text-xs font-semibold underline hover:opacity-75"
                                on:click=move |_| feedback.set(None)
                            >
                                "Dismiss"
                            </button>
                        </div>
                    }
                })
            }}

            // Navigation bar & locale selector
            <div class="flex flex-wrap items-center justify-between gap-4 border-b border-border pb-4">
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class=move || {
                            let is_active = active_tab.get() == TemplatesTab::Templates;
                            if is_active {
                                "rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm"
                            } else {
                                "rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-muted text-muted-foreground"
                            }
                        }
                        on:click=move |_| {
                            active_tab.set(TemplatesTab::Templates);
                            feedback.set(None);
                        }
                    >
                        "Layout Templates"
                    </button>
                    <button
                        type="button"
                        class=move || {
                            let is_active = active_tab.get() == TemplatesTab::Symbols;
                            if is_active {
                                "rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-sm"
                            } else {
                                "rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-muted text-muted-foreground"
                            }
                        }
                        on:click=move |_| {
                            active_tab.set(TemplatesTab::Symbols);
                            feedback.set(None);
                        }
                    >
                        "Site Symbols"
                    </button>
                </div>

                <div class="flex items-center gap-2">
                    <label class="text-xs font-medium text-muted-foreground">"Locale:"</label>
                    <input
                        type="text"
                        class="w-20 rounded-lg border border-input bg-background px-2 py-1 text-sm text-center"
                        prop:value=move || selected_locale.get()
                        on:change=move |event| {
                            let val = event_target_value(&event).trim().to_ascii_lowercase();
                            if !val.is_empty() {
                                selected_locale.set(val);
                                refresh_generation.update(|g| *g = g.wrapping_add(1));
                            }
                        }
                    />
                </div>
            </div>

            // Active Tab Content
            {move || match active_tab.get() {
                TemplatesTab::Templates => view! {
                    <div class="grid gap-6 lg:grid-cols-[1fr_22rem]">
                        // Templates catalog list
                        <section class="space-y-4">
                            <h2 class="text-lg font-semibold text-card-foreground">"Active Layout Templates"</h2>
                            <Suspense fallback=|| view! {
                                <div class="space-y-3">
                                    <div class="h-24 animate-pulse rounded-2xl border border-border bg-muted"></div>
                                    <div class="h-24 animate-pulse rounded-2xl border border-border bg-muted"></div>
                                </div>
                            }>
                                {move || {
                                    templates_resource.get().map(|result| match result {
                                        Ok(templates) if templates.is_empty() => view! {
                                            <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                                "No custom layout templates found for this locale. The default canvas layout is available."
                                            </div>
                                        }.into_any(),
                                        Ok(templates) => view! {
                                            <div class="space-y-3">
                                                {templates.into_iter().map(|template| {
                                                    let is_default = template.key == "default";
                                                    let edit_key = template.key.clone();
                                                    let edit_headers = template.header_symbol_ids.join(", ");
                                                    let edit_footers = template.footer_symbol_ids.join(", ");
                                                    let edit_rev = template.revision;
                                                    let delete_key = template.key.clone();
                                                    let delete_rev = template.revision;
                                                    let tok = token;
                                                    let ten = tenant;
                                                    let loc = selected_locale;

                                                    view! {
                                                        <div class="flex flex-col justify-between gap-4 rounded-2xl border border-border bg-card p-5 shadow-sm sm:flex-row sm:items-center">
                                                            <div class="space-y-2">
                                                                <div class="flex items-center gap-2">
                                                                    <span class="font-mono text-base font-semibold text-card-foreground">
                                                                        {template.key.clone()}
                                                                    </span>
                                                                    {if is_default {
                                                                        view! {
                                                                            <span class="rounded bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground">
                                                                                "default virtual"
                                                                            </span>
                                                                        }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <span class="rounded bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                                                                                {format!("rev {}", template.revision)}
                                                                            </span>
                                                                        }.into_any()
                                                                    }}
                                                                </div>
                                                                <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                                                                    <span>"Headers: "</span>
                                                                    {if template.header_symbol_ids.is_empty() {
                                                                        view! { <span class="italic text-muted-foreground/60">"none"</span> }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <div class="flex flex-wrap gap-1">
                                                                                {template.header_symbol_ids.into_iter().map(|id| view! {
                                                                                    <span class="rounded bg-secondary px-1.5 py-0.5 font-mono text-[11px] text-secondary-foreground">{id}</span>
                                                                                }).collect_view()}
                                                                            </div>
                                                                        }.into_any()
                                                                    }}
                                                                    <span class="ml-2">"Footers: "</span>
                                                                    {if template.footer_symbol_ids.is_empty() {
                                                                        view! { <span class="italic text-muted-foreground/60">"none"</span> }.into_any()
                                                                    } else {
                                                                        view! {
                                                                            <div class="flex flex-wrap gap-1">
                                                                                {template.footer_symbol_ids.into_iter().map(|id| view! {
                                                                                    <span class="rounded bg-secondary px-1.5 py-0.5 font-mono text-[11px] text-secondary-foreground">{id}</span>
                                                                                }).collect_view()}
                                                                            </div>
                                                                        }.into_any()
                                                                    }}
                                                                </div>
                                                            </div>
                                                            <div class="flex items-center gap-2 self-end sm:self-center">
                                                                <button
                                                                    type="button"
                                                                    class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium hover:bg-muted"
                                                                    on:click=move |_| {
                                                                        template_key.set(edit_key.clone());
                                                                        template_header_symbols.set(edit_headers.clone());
                                                                        template_footer_symbols.set(edit_footers.clone());
                                                                        template_expected_revision.set(Some(edit_rev));
                                                                        template_is_editing.set(true);
                                                                    }
                                                                >
                                                                    "Edit"
                                                                </button>
                                                                {if !is_default {
                                                                    view! {
                                                                        <button
                                                                            type="button"
                                                                            class="rounded-lg border border-destructive/40 px-3 py-1.5 text-xs font-medium text-destructive hover:bg-destructive/10"
                                                                            on:click=move |_| {
                                                                                let k = delete_key.clone();
                                                                                let t = tok.get_untracked();
                                                                                let tn = ten.get_untracked();
                                                                                let l = loc.get_untracked();
                                                                                spawn_local(async move {
                                                                                    match transport::delete_page_template(t, tn, k.clone(), l, delete_rev).await {
                                                                                        Ok(_) => {
                                                                                            feedback.set(Some((false, format!("Template `{k}` deleted"))));
                                                                                            refresh_generation.update(|g| *g = g.wrapping_add(1));
                                                                                        }
                                                                                        Err(err) => {
                                                                                            feedback.set(Some((true, format!("Failed to delete template: {err}"))));
                                                                                        }
                                                                                    }
                                                                                });
                                                                            }
                                                                        >
                                                                            "Delete"
                                                                        </button>
                                                                    }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }.into_any(),
                                        Err(err) => view! {
                                            <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                                                {format!("Error loading templates: {err}")}
                                            </div>
                                        }.into_any(),
                                    })
                                }}
                            </Suspense>
                        </section>

                        // Template Editor Card
                        <aside class="h-fit rounded-2xl border border-border bg-card p-5 shadow-sm">
                            <h3 class="mb-3 font-semibold text-card-foreground">
                                {move || if template_is_editing.get() { "Edit Template" } else { "Create Template" }}
                            </h3>
                            <form on:submit=save_template_action class="space-y-4">
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "Template Key"
                                    </label>
                                    <input
                                        type="text"
                                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono"
                                        placeholder="e.g. landing, sidebar"
                                        prop:value=move || template_key.get()
                                        prop:disabled=move || template_is_editing.get() || template_busy.get()
                                        on:input=move |event| template_key.set(event_target_value(&event))
                                        required
                                    />
                                    <p class="mt-1 text-[11px] text-muted-foreground">
                                        "Alphanumeric key identifying the layout frame."
                                    </p>
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "Header Symbol IDs"
                                    </label>
                                    <input
                                        type="text"
                                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono"
                                        placeholder="site-header, promo-bar"
                                        prop:value=move || template_header_symbols.get()
                                        prop:disabled=move || template_busy.get()
                                        on:input=move |event| template_header_symbols.set(event_target_value(&event))
                                    />
                                    <p class="mt-1 text-[11px] text-muted-foreground">
                                        "Comma-separated symbol IDs rendered above the page slot."
                                    </p>
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "Footer Symbol IDs"
                                    </label>
                                    <input
                                        type="text"
                                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono"
                                        placeholder="site-footer"
                                        prop:value=move || template_footer_symbols.get()
                                        prop:disabled=move || template_busy.get()
                                        on:input=move |event| template_footer_symbols.set(event_target_value(&event))
                                    />
                                    <p class="mt-1 text-[11px] text-muted-foreground">
                                        "Comma-separated symbol IDs rendered below the page slot."
                                    </p>
                                </div>
                                <div class="flex items-center gap-2 pt-2">
                                    <button
                                        type="submit"
                                        class="flex-1 rounded-lg bg-primary py-2 text-center text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
                                        prop:disabled=move || template_busy.get()
                                    >
                                        {move || if template_busy.get() { "Saving..." } else { "Save Template" }}
                                    </button>
                                    {move || if template_is_editing.get() {
                                        view! {
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
                                                on:click=move |_| reset_template_form()
                                            >
                                                "Cancel"
                                            </button>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>
                            </form>
                        </aside>
                    </div>
                }.into_any(),

                TemplatesTab::Symbols => view! {
                    <div class="grid gap-6 lg:grid-cols-[1fr_24rem]">
                        // Site Symbols catalog list
                        <section class="space-y-4">
                            <h2 class="text-lg font-semibold text-card-foreground">"Active Site Symbols"</h2>
                            <Suspense fallback=|| view! {
                                <div class="space-y-3">
                                    <div class="h-24 animate-pulse rounded-2xl border border-border bg-muted"></div>
                                    <div class="h-24 animate-pulse rounded-2xl border border-border bg-muted"></div>
                                </div>
                            }>
                                {move || {
                                    symbols_resource.get().map(|result| match result {
                                        Ok(symbols) if symbols.is_empty() => view! {
                                            <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                                "No site symbols found for this locale. Define headers, footers, or promo banners to attach to layout templates."
                                            </div>
                                        }.into_any(),
                                        Ok(symbols) => {
                                            let current_templates = templates_resource.get().and_then(Result::ok).unwrap_or_default();
                                            view! {
                                                <div class="space-y-3">
                                                    {symbols.into_iter().map(|symbol| {
                                                        let edit_id = symbol.symbol_id.clone();
                                                        let edit_name = symbol.name.clone().unwrap_or_default();
                                                        let edit_json = symbol.content_json.clone();
                                                        let delete_id = symbol.symbol_id.clone();
                                                        let sym_id_check = symbol.symbol_id.clone();
                                                        let tok = token;
                                                        let ten = tenant;
                                                        let loc = selected_locale;
                                                        let templates_res = templates_resource;
                                                        let target_sig = delete_dialog_target;

                                                        let used_in_templates: Vec<String> = current_templates
                                                            .iter()
                                                            .filter(|tpl| {
                                                                tpl.header_symbol_ids.iter().any(|id| id.eq_ignore_ascii_case(&sym_id_check))
                                                                    || tpl.footer_symbol_ids.iter().any(|id| id.eq_ignore_ascii_case(&sym_id_check))
                                                            })
                                                            .map(|tpl| tpl.key.clone())
                                                            .collect();
                                                        let has_template_refs = !used_in_templates.is_empty();
                                                        let tpl_count = used_in_templates.len();
                                                        let tpl_preview = used_in_templates.join(", ");

                                                        view! {
                                                            <div class="flex flex-col justify-between gap-4 rounded-2xl border border-border bg-card p-5 shadow-sm sm:flex-row sm:items-center">
                                                                <div class="space-y-1.5">
                                                                    <div class="flex items-center gap-2">
                                                                        <span class="font-mono text-base font-semibold text-card-foreground">
                                                                            {symbol.symbol_id.clone()}
                                                                        </span>
                                                                        {symbol.name.map(|n| view! {
                                                                            <span class="text-sm text-muted-foreground">
                                                                                {format!("({n})")}
                                                                            </span>
                                                                        })}
                                                                    </div>
                                                                    <div class="flex flex-wrap items-center gap-2">
                                                                        {if has_template_refs {
                                                                            view! {
                                                                                <span class="inline-flex items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/10 px-2.5 py-0.5 text-[11px] font-medium text-amber-800 dark:text-amber-300">
                                                                                    <span class="inline-block h-1.5 w-1.5 rounded-full bg-amber-500"></span>
                                                                                    {format!("In {tpl_count} template(s): {tpl_preview}")}
                                                                                </span>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! {
                                                                                <span class="inline-flex items-center gap-1 rounded-full border border-border bg-muted/60 px-2.5 py-0.5 text-[11px] font-medium text-muted-foreground">
                                                                                    <span class="inline-block h-1.5 w-1.5 rounded-full bg-muted-foreground/50"></span>
                                                                                    "No template references"
                                                                                </span>
                                                                            }.into_any()
                                                                        }}
                                                                        <span class="text-xs text-muted-foreground">
                                                                            {format!("AST: {} bytes", symbol.content_json.len())}
                                                                        </span>
                                                                    </div>
                                                                </div>
                                                                <div class="flex items-center gap-2 self-end sm:self-center">
                                                                    <button
                                                                        type="button"
                                                                        class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium hover:bg-muted"
                                                                        on:click=move |_| {
                                                                            symbol_id.set(edit_id.clone());
                                                                            symbol_name.set(edit_name.clone());
                                                                            symbol_content_json.set(edit_json.clone());
                                                                            symbol_is_editing.set(true);
                                                                        }
                                                                    >
                                                                        "Edit"
                                                                    </button>
                                                                    <button
                                                                        type="button"
                                                                        class="rounded-lg border border-destructive/40 px-3 py-1.5 text-xs font-medium text-destructive hover:bg-destructive/10"
                                                                        on:click=move |_| {
                                                                            let id = delete_id.clone();
                                                                            target_sig.set(Some((id.clone(), None, true)));
                                                                            let t = tok.get_untracked();
                                                                            let tn = ten.get_untracked();
                                                                            let l = loc.get_untracked();
                                                                            spawn_local(async move {
                                                                                let pages = transport::fetch_site_symbol_usage(t, tn, id.clone(), l)
                                                                                    .await
                                                                                    .unwrap_or_default();
                                                                                let templates = templates_res.get_untracked().and_then(Result::ok).unwrap_or_default();
                                                                                let usage = core::compute_symbol_usage(&id, &templates, pages);
                                                                                target_sig.set(Some((id, Some(usage), false)));
                                                                            });
                                                                        }
                                                                    >
                                                                        "Delete"
                                                                    </button>
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            }.into_any()
                                        }
                                        Err(err) => view! {
                                            <div class="rounded-xl border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
                                                {format!("Error loading site symbols: {err}")}
                                            </div>
                                        }.into_any(),
                                    })
                                }}
                            </Suspense>
                        </section>

                        // Site Symbol Editor Card
                        <aside class="h-fit rounded-2xl border border-border bg-card p-5 shadow-sm">
                            <h3 class="mb-3 font-semibold text-card-foreground">
                                {move || if symbol_is_editing.get() { "Edit Site Symbol" } else { "Create Site Symbol" }}
                            </h3>
                            <form on:submit=save_symbol_action class="space-y-4">
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "Symbol ID"
                                    </label>
                                    <input
                                        type="text"
                                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm font-mono"
                                        placeholder="e.g. site-header, footer-main"
                                        prop:value=move || symbol_id.get()
                                        prop:disabled=move || symbol_is_editing.get() || symbol_busy.get()
                                        on:input=move |event| symbol_id.set(event_target_value(&event))
                                        required
                                    />
                                    <p class="mt-1 text-[11px] text-muted-foreground">
                                        "Unique cross-page identifier for this symbol."
                                    </p>
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "Display Name (Optional)"
                                    </label>
                                    <input
                                        type="text"
                                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                        placeholder="Main Site Header"
                                        prop:value=move || symbol_name.get()
                                        prop:disabled=move || symbol_busy.get()
                                        on:input=move |event| symbol_name.set(event_target_value(&event))
                                    />
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-card-foreground">
                                        "AST Content JSON"
                                    </label>
                                    <textarea
                                        class="mt-1 h-44 w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-xs"
                                        prop:value=move || symbol_content_json.get()
                                        prop:disabled=move || symbol_busy.get()
                                        on:input=move |event| symbol_content_json.set(event_target_value(&event))
                                        required
                                    />
                                    <p class="mt-1 text-[11px] text-muted-foreground">
                                        "Serialized Fly component tree or section definition."
                                    </p>
                                </div>
                                <div class="flex items-center gap-2 pt-2">
                                    <button
                                        type="submit"
                                        class="flex-1 rounded-lg bg-primary py-2 text-center text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
                                        prop:disabled=move || symbol_busy.get()
                                    >
                                        {move || if symbol_busy.get() { "Saving..." } else { "Save Symbol" }}
                                    </button>
                                    {move || if symbol_is_editing.get() {
                                        view! {
                                            <button
                                                type="button"
                                                class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
                                                on:click=move |_| reset_symbol_form()
                                            >
                                                "Cancel"
                                            </button>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </div>
                            </form>
                        </aside>
                    </div>
                }.into_any(),
            }}

            // Safe Deletion Modal Dialog
            {move || {
                delete_dialog_target.get().map(|(sym_id, usage_opt, is_loading)| {
                    let sym_id_for_delete = sym_id.clone();
                    let is_deleting = delete_busy.get();

                    view! {
                        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4" role="dialog" aria-modal="true">
                            <div class="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-xl space-y-4">
                                <div class="flex items-start justify-between gap-3">
                                    <div>
                                        <h3 class="text-lg font-semibold text-card-foreground">
                                            "Safe Symbol Deletion"
                                        </h3>
                                        <p class="text-xs font-mono text-muted-foreground mt-0.5">
                                            {format!("Symbol ID: {sym_id}")}
                                        </p>
                                    </div>
                                    <button
                                        type="button"
                                        class="rounded-lg p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                                        on:click=move |_| delete_dialog_target.set(None)
                                    >
                                        <span class="sr-only">"Close"</span>
                                        "✕"
                                    </button>
                                </div>

                                {if is_loading {
                                    view! {
                                        <div class="flex items-center gap-3 py-6 text-sm text-muted-foreground">
                                            <div class="h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent shrink-0"></div>
                                            <span>"Analyzing site-wide symbol references across templates and pages..."</span>
                                        </div>
                                    }.into_any()
                                } else if let Some(usage) = usage_opt {
                                    if !usage.can_delete_safely() {
                                        let dep_count = usage.total_dependents();
                                        view! {
                                            <div class="space-y-4">
                                                <div class="rounded-xl border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-900 dark:text-amber-200 space-y-2" role="alert">
                                                    <div class="font-semibold flex items-center gap-1.5">
                                                        <span>{format!("⚠️ Symbol is in active use ({dep_count} reference(s))")}</span>
                                                    </div>
                                                    <p class="text-xs leading-relaxed">
                                                        {usage.deletion_warning().unwrap_or_default()}
                                                    </p>
                                                </div>

                                                <div class="rounded-xl border border-border bg-muted/40 p-3 space-y-2 text-xs">
                                                    <div class="font-medium text-foreground">"Dependency Breakdown:"</div>
                                                    {(!usage.dependent_templates.is_empty()).then(|| view! {
                                                        <div>
                                                            <span class="text-muted-foreground">"Layout Templates (" {usage.dependent_templates.len()} "): "</span>
                                                            <span class="font-mono font-medium">{usage.dependent_templates.join(", ")}</span>
                                                        </div>
                                                    })}
                                                    {(!usage.dependent_page_ids.is_empty()).then(|| view! {
                                                        <div>
                                                            <span class="text-muted-foreground">"Page Documents: "</span>
                                                            <span class="font-mono font-medium">{format!("{} page(s)", usage.dependent_page_ids.len())}</span>
                                                        </div>
                                                    })}
                                                </div>

                                                <p class="text-xs text-muted-foreground">
                                                    "To safely delete this symbol, detach it from the layout templates above and re-publish referencing pages."
                                                </p>

                                                <div class="flex items-center justify-end gap-2 pt-2 border-t border-border">
                                                    <button
                                                        type="button"
                                                        class="rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-muted"
                                                        on:click=move |_| delete_dialog_target.set(None)
                                                    >
                                                        "Cancel"
                                                    </button>
                                                    <button
                                                        type="button"
                                                        class="rounded-lg border border-destructive/40 bg-destructive/10 px-4 py-2 text-sm font-medium text-destructive hover:bg-destructive hover:text-destructive-foreground disabled:opacity-50"
                                                        disabled=is_deleting
                                                        on:click={
                                                            let id = sym_id_for_delete.clone();
                                                            let tok = token;
                                                            let ten = tenant;
                                                            let loc = selected_locale;
                                                            move |_| {
                                                                delete_busy.set(true);
                                                                let sym = id.clone();
                                                                let t = tok.get_untracked();
                                                                let tn = ten.get_untracked();
                                                                let l = loc.get_untracked();
                                                                spawn_local(async move {
                                                                    match transport::delete_site_symbol(t, tn, sym.clone(), l).await {
                                                                        Ok(_) => {
                                                                            feedback.set(Some((false, format!("Symbol `{sym}` deleted"))));
                                                                            delete_dialog_target.set(None);
                                                                            refresh_generation.update(|g| *g = g.wrapping_add(1));
                                                                        }
                                                                        Err(err) => {
                                                                            feedback.set(Some((true, format!("Server rejected deletion: {err}"))));
                                                                            delete_dialog_target.set(None);
                                                                        }
                                                                    }
                                                                    delete_busy.set(false);
                                                                });
                                                            }
                                                        }
                                                    >
                                                        {if is_deleting { "Deleting..." } else { "Force Delete Attempt" }}
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="space-y-4">
                                                <div class="rounded-xl border border-emerald-500/30 bg-emerald-500/10 p-4 text-sm text-emerald-800 dark:text-emerald-300">
                                                    <p class="text-xs leading-relaxed">
                                                        "No dependencies found. This symbol is not referenced by any layout templates or page documents and can be safely deleted."
                                                    </p>
                                                </div>

                                                <p class="text-sm text-foreground">
                                                    "Are you sure you want to permanently delete site symbol " <code class="font-mono font-semibold">{sym_id.clone()}</code> "?"
                                                </p>

                                                <div class="flex items-center justify-end gap-2 pt-2 border-t border-border">
                                                    <button
                                                        type="button"
                                                        class="rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-muted"
                                                        on:click=move |_| delete_dialog_target.set(None)
                                                    >
                                                        "Cancel"
                                                    </button>
                                                    <button
                                                        type="button"
                                                        class="rounded-lg bg-destructive px-4 py-2 text-sm font-medium text-destructive-foreground hover:opacity-90 disabled:opacity-50"
                                                        disabled=is_deleting
                                                        on:click={
                                                            let id = sym_id_for_delete.clone();
                                                            let tok = token;
                                                            let ten = tenant;
                                                            let loc = selected_locale;
                                                            move |_| {
                                                                delete_busy.set(true);
                                                                let sym = id.clone();
                                                                let t = tok.get_untracked();
                                                                let tn = ten.get_untracked();
                                                                let l = loc.get_untracked();
                                                                spawn_local(async move {
                                                                    match transport::delete_site_symbol(t, tn, sym.clone(), l).await {
                                                                        Ok(_) => {
                                                                            feedback.set(Some((false, format!("Symbol `{sym}` deleted successfully"))));
                                                                            delete_dialog_target.set(None);
                                                                            refresh_generation.update(|g| *g = g.wrapping_add(1));
                                                                        }
                                                                        Err(err) => {
                                                                            feedback.set(Some((true, format!("Failed to delete symbol: {err}"))));
                                                                            delete_dialog_target.set(None);
                                                                        }
                                                                    }
                                                                    delete_busy.set(false);
                                                                });
                                                            }
                                                        }
                                                    >
                                                        {if is_deleting { "Deleting..." } else { "Confirm Delete" }}
                                                    </button>
                                                </div>
                                            </div>
                                        }.into_any()
                                    }
                                } else {
                                    view! { <div /> }.into_any()
                                }}
                            </div>
                        </div>
                    }
                })
            }}
        </div>
    }
}
