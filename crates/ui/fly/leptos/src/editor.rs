//! A working Leptos editor surface built on the shared Fly contracts.
//!
//! Everything semantic comes from `fly` and `fly-ui`: the document lives in a
//! [`FlyEditor`], presentation state in a [`FlyUiStateMachine`], and the palette/layer
//! view-models are the shared `fly_ui` derivations. This module only binds those to
//! Leptos signals and DOM. It renders identically on the server (SSR) and in the browser;
//! pointer geometry and drag & drop stay in `fly-web` behind the `wasm-client` feature.
//!
//! The host owns persistence: it receives every document change through `on_change` and
//! decides how to store it, exactly like the SSR intent endpoint does.

use fly::{
    blank_page, normalize_slug, render_page, validate_token_name, validate_token_value,
    AssetCatalog, AssetCommand, DesignToken, EditorCommand, FlyEditor, PageCommand, PageLocator,
    PageMetadata, PagePatch, PageSelection, PageSummary, ProjectDocument, RegistrySet,
    RenderPolicy, ThemeCommand,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{
    builtin_responsive_breakpoints, builtin_style_properties, builtin_style_states,
    component_property_view, editor_layer_items, editor_layer_items_for_page,
    palette_block_items_with_access, preview_viewport_for_breakpoint, responsive_breakpoint,
    scoped_style_value, style_patch, theme_token_items, CapabilityState, ComponentPropertyChange,
    ComponentPropertyKind, ComponentPropertyView, ContributionAssemblyResult, EditorMessages,
    FlyUiStateMachine, PagePropertiesUpdate, PaletteBlockAccess, PaletteBlockItem, Presentation,
    ScopedStyleChange, StyleEntry, ThemeTokenItem, UiIntent,
};
use leptos::prelude::*;

/// Summary of a contribution assembly: how many contributions registered, how
/// many were skipped, and every assembly diagnostic with an accessible role
/// matching its severity. Rendered above the palette so provider problems are
/// visible where authors look for their blocks.
#[component]
pub fn FlyContributionSummary(
    assembly: ContributionAssemblyResult,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let template = messages.unwrap_or_default().contributions_summary;
    let headline = template
        .replace(
            "{registered}",
            &assembly.registered_contributions.to_string(),
        )
        .replace("{skipped}", &assembly.skipped_contributions.to_string());
    let registered = assembly.registered_contributions;
    let skipped = assembly.skipped_contributions;
    view! {
        <div
            class="fly-contributions"
            data-fly-contribution-registry="true"
            data-fly-contributions-registered=registered
            data-fly-contributions-skipped=skipped
        >
            <p class="fly-contributions__headline">{headline}</p>
            {assembly
                .diagnostics
                .into_iter()
                .map(|diagnostic| {
                    let role = match diagnostic.severity {
                        fly_ui::ContributionAssemblySeverity::Error => {
                            "alert"
                        }
                        _ => "status",
                    };
                    let code = diagnostic.code.clone();
                    view! {
                        <p
                            class="fly-contributions__diagnostic"
                            role=role
                            data-fly-contribution-diagnostic=code.clone()
                        >
                            <code>{code}</code>
                            {format!(": {}", diagnostic.message)}
                        </p>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Palette of insertable blocks, grouped by category in collapsible sections
/// (`landing` starts open, mirroring the host admin panels). Selecting one
/// notifies the host through `on_insert` with the registry block id; the host
/// (or [`FlyEditorWorkspace`]) turns that into an `EditorCommand::Insert`
/// against its own document. When `capabilities` denies editing, blocks render
/// disabled so capability gating lives in this one surface instead of every host.
#[component]
pub fn FlyPalette(
    blocks: Vec<PaletteBlockItem>,
    on_insert: Callback<String>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let palette_label = messages.unwrap_or_default().palette;
    let can_insert = capabilities.map(|state| state.edit).unwrap_or(true);
    let mut groups = std::collections::BTreeMap::<String, Vec<PaletteBlockItem>>::new();
    for block in blocks {
        groups
            .entry(block.category.clone())
            .or_default()
            .push(block);
    }
    view! {
        <div class="fly-palette" role="group" aria-label=palette_label>
            {groups
                .into_iter()
                .map(|(category, blocks)| {
                    let start_open = category == "landing";
                    view! {
                        <details class="fly-palette__category" open=start_open>
                            <summary>{category}</summary>
                            <div role="list">
                                {blocks
                                    .into_iter()
                                    .map(|block| {
                                        let block_id = block.id.clone();
                                        let contribution_ids =
                                            block.contribution_ids.join(",");
                                        view! {
                                            <button
                                                type="button"
                                                class="fly-palette__block"
                                                role="listitem"
                                                data-fly-block=block.id.clone()
                                                data-fly-category=block.category.clone()
                                                data-fly-contribution-ids=contribution_ids
                                                disabled=!can_insert
                                                on:click=move |_| {
                                                    on_insert.run(block_id.clone())
                                                }
                                            >
                                                {block.label.clone()}
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </details>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// Layer tree for a document. Rows are buttons so selection works with keyboard and
/// assistive technology, not only pointer hit-testing.
#[component]
pub fn FlyLayerTree(
    document: ProjectDocument,
    selected: Option<String>,
    on_select: Callback<Option<String>>,
    #[prop(optional)] page_index: Option<usize>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let items = match page_index {
        Some(index) => editor_layer_items_for_page(&document, index),
        None => editor_layer_items(&document),
    };
    let layers_label = messages.unwrap_or_default().layers;
    view! {
        <ol class="fly-layers" aria-label=layers_label>
            {items
                .into_iter()
                .map(|item| {
                    let is_selected = !item.id.is_empty() && selected.as_deref() == Some(item.id.as_str());
                    let item_id = item.id.clone();
                    let label = item.label.clone();
                    let indent = format!("padding-left:{}rem", 0.5 + item.depth as f32 * 1.25);
                    view! {
                        <li>
                            <button
                                type="button"
                                class="fly-layers__row"
                                style=indent
                                aria-current=is_selected.then_some("true")
                                disabled=item.opaque || item_id.is_empty()
                                on:click=move |_| {
                                    if !item_id.is_empty() {
                                        on_select.run(Some(item_id.clone()));
                                    }
                                }
                            >
                                {label}
                            </button>
                        </li>
                    }
                })
                .collect_view()}
        </ol>
    }
}

/// Read-only design-token listing for theme panels: each token's custom property name
/// and value. Authoring tokens stays with the host (form or JSON); adapters display the
/// single validated projection from `fly-ui`.
#[component]
pub fn FlyThemeTokens(
    tokens: Vec<ThemeTokenItem>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let tokens_label = messages.theme_tokens;
    view! {
        <dl class="fly-theme-tokens" aria-label=tokens_label>
            {tokens
                .into_iter()
                .map(|token| {
                    view! {
                        <div class="fly-theme-tokens__row">
                            <dt><code>{token.css_variable.clone()}</code></dt>
                            <dd>{token.value.clone()}</dd>
                        </div>
                    }
                })
                .collect_view()}
        </dl>
    }
}

/// Editable design-token panel: lists the validated token projection, removes tokens,
/// and upserts one token by name. Input is checked with the same `fly` validators the
/// command path applies, so an invalid name/value is reported inline instead of being
/// silently dropped; accepted edits go out as callbacks and the host/workspace turns
/// them into `EditorCommand::Theme` (the only mutation path for `flyTheme`).
#[component]
pub fn FlyThemeTokenEditor(
    tokens: Vec<ThemeTokenItem>,
    on_upsert: Callback<DesignToken>,
    on_remove: Callback<String>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let can_styles = capabilities.map(|state| state.styles).unwrap_or(true);
    let tokens_label = messages.theme_tokens.clone();
    let tokens_empty_label = messages.tokens_empty.clone();
    let empty_tokens = tokens.is_empty();
    let remove_label = messages.remove;
    let token_name_label = messages.token_name;
    let token_value_label = messages.token_value;
    let save_token_label = messages.save_token;
    let token_name = RwSignal::new(String::new());
    let token_value = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);

    view! {
        <div class="fly-theme-token-editor">
            {empty_tokens.then(|| {
                view! { <p class="fly-theme-tokens__empty">{tokens_empty_label}</p> }
            })}
            <dl class="fly-theme-tokens" aria-label=tokens_label>
                {tokens
                    .into_iter()
                    .map(|token| {
                        let name_for_remove = token.name.clone();
                        let remove_label = remove_label.clone();
                        view! {
                            <div class="fly-theme-tokens__row">
                                <dt><code>{token.css_variable.clone()}</code></dt>
                                <dd>{token.value.clone()}</dd>
                                <dd>
                                    <button
                                        type="button"
                                        class="fly-theme-tokens__remove"
                                        disabled=!can_styles
                                        on:click=move |_| on_remove.run(name_for_remove.clone())
                                    >
                                        {remove_label}
                                    </button>
                                </dd>
                            </div>
                        }
                    })
                    .collect_view()}
            </dl>
            <div class="fly-theme-token-editor__form">
                <label>
                    {token_name_label}
                    <input
                        type="text"
                        disabled=!can_styles
                        placeholder="brand"
                        prop:value=move || token_name.get()
                        on:input=move |event| token_name.set(event_target_value(&event))
                    />
                </label>
                <label>
                    {token_value_label}
                    <input
                        type="text"
                        disabled=!can_styles
                        placeholder="#0b5fff"
                        prop:value=move || token_value.get()
                        on:input=move |event| token_value.set(event_target_value(&event))
                    />
                </label>
                <button
                    type="button"
                    disabled=!can_styles
                    on:click=move |_| {
                        let name = token_name.get_untracked().trim().to_string();
                        let value = token_value.get_untracked().trim().to_string();
                        match validate_token_name(&name)
                            .and_then(|_| validate_token_value(&name, &value))
                        {
                            Ok(()) => {
                                error.set(None);
                                on_upsert.run(DesignToken { name, value });
                            }
                            Err(message) => error.set(Some(message)),
                        }
                    }
                >
                    {save_token_label}
                </button>
                {move || {
                    error
                        .get()
                        .map(|message| {
                            view! {
                                <p class="fly-theme-token-editor__error" role="alert">{message}</p>
                            }
                        })
                }}
            </div>
        </div>
    }
}

/// Read-only rendered preview of a document (first page), with the Fly project styles.
/// The HTML comes from the Fly SSR renderer, which escapes every author-controlled sink.
#[component]
pub fn FlyCanvasPreview(
    document: ProjectDocument,
    #[prop(optional)] page_index: Option<usize>,
    #[prop(optional)] viewport_width: Option<u32>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let canvas_style =
        viewport_width.map(|width| format!("max-width: {width}px; margin-inline: auto"));
    let preview_failed_label = messages.unwrap_or_default().preview_failed;
    let selection = match page_index {
        Some(index) => PageSelection::Index(index),
        None => PageSelection::First,
    };
    let rendered = render_page(&document, &selection, &RenderPolicy::default());
    match rendered {
        Ok(page) => view! {
            <div class="fly-canvas" data-fly-canvas="true" style=canvas_style>
                <style>{page.css}</style>
                <div inner_html=page.html></div>
            </div>
        }
        .into_any(),
        Err(error) => view! {
            <div class="fly-canvas fly-canvas--error" role="alert">
                {format!("{preview_failed_label}: {error}")}
            </div>
        }
        .into_any(),
    }
}

/// Surfaces an editor error the workspace would otherwise swallow: every
/// `FlyEditor::apply` result is routed here so hosts see failures in one place
/// (error banner, toast, log) instead of silent no-ops.
fn report_workspace_error(
    on_error: Option<Callback<String>>,
    context: &str,
    error: &fly::FlyError,
) {
    if let Some(on_error) = on_error {
        on_error.run(format!("{context}: {error}"));
    }
}

fn report_workspace_error_raw(on_error: Option<Callback<String>>, message: &str) {
    if let Some(on_error) = on_error {
        on_error.run(message.to_string());
    }
}

fn current_media_scope(breakpoint_id: String) -> fly::StyleRuleScope {
    responsive_breakpoint(breakpoint_id.as_str())
        .map(|breakpoint| fly::StyleRuleScope::Media {
            query: breakpoint.media_query,
        })
        .unwrap_or(fly::StyleRuleScope::Base)
}

fn unique_workspace_page_id(pages: &[PageSummary], name: &str) -> String {
    let existing: std::collections::BTreeSet<&str> =
        pages.iter().filter_map(|page| page.id.as_deref()).collect();
    let normalized = normalize_slug(name.to_string());
    let base = if normalized.is_empty() {
        "page".to_string()
    } else {
        normalized
    };
    if !existing.contains(base.as_str()) {
        return base;
    }
    for suffix in 2..=10_000 {
        let candidate = format!("{base}-{suffix}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    format!("{base}-{}", existing.len().saturating_add(1))
}

/// Page switcher: one tab per document page, in document order. Activation is a
/// plain callback — the workspace turns it into a local page switch or, in intent
/// mode, an `activate_page` envelope.
#[component]
pub fn FlyPageTabs(
    pages: Vec<PageSummary>,
    active: usize,
    on_activate: Callback<usize>,
    #[prop(optional)] on_add: Option<Callback<String>>,
    #[prop(optional)] on_remove: Option<Callback<usize>>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let can_edit = capabilities.map(|state| state.edit).unwrap_or(true);
    let pages_label = messages.pages.clone();
    let new_page_name_label = messages.new_page_name.clone();
    let add_page_label = messages.add_page.clone();
    let remove_page_label = messages.remove_page.clone();
    let new_page_name = RwSignal::new(String::new());
    let remove_disabled = pages.len() <= 1;
    view! {
        <nav class="fly-pages" aria-label=pages_label>
            {pages
                .into_iter()
                .map(|page| {
                    let is_active = page.index == active;
                    let page_index = page.index;
                    let page_name = page.name.clone();
                    let remove_label = format!("{remove_page_label}: {page_name}");
                    view! {
                        <span class="fly-pages__entry">
                            <button
                                type="button"
                                class="fly-pages__tab"
                                aria-current=is_active.then_some("page")
                                on:click=move |_| on_activate.run(page_index)
                            >
                                {page_name}
                            </button>
                            {on_remove.map(|on_remove| {
                                view! {
                                    <button
                                        type="button"
                                        disabled=remove_disabled || !can_edit
                                        class="fly-pages__remove"
                                        aria-label=remove_label
                                        on:click=move |_| on_remove.run(page_index)
                                    >
                                        "×"
                                    </button>
                                }
                            })}
                        </span>
                    }
                })
                .collect_view()}
            {on_add.map(|on_add| {
                view! {
                    <span class="fly-pages__create">
                        <input
                            type="text"
                            class="fly-pages__new-name"
                            disabled=!can_edit
                            aria-label=new_page_name_label.clone()
                            placeholder=new_page_name_label
                            prop:value=move || new_page_name.get()
                            on:input=move |event| {
                                new_page_name.set(event_target_value(&event))
                            }
                        />
                        <button
                            type="button"
                            class="fly-pages__add"
                            disabled=!can_edit
                            on:click=move |_| {
                                let name = new_page_name.get_untracked();
                                on_add.run(name);
                                new_page_name.set(String::new());
                            }
                        >
                            {add_page_label}
                        </button>
                    </span>
                }
            })}
        </nav>
    }
}

/// Responsive + interaction-state styles for the selected component: one CSS
/// declaration under a media-query scope (breakpoint) or a pseudo-class state
/// scope, seeded from the document's current rule for that scope. Apply writes,
/// Clear removes; the optional preview callback lets the host switch its
/// preview surface to the breakpoint's device width.
#[component]
pub fn FlyStyleRules(
    component_id: Option<String>,
    rules: fly::StyleRuleCatalog,
    on_upsert: Callback<ScopedStyleChange>,
    #[prop(optional)] on_preview_breakpoint: Option<Callback<String>>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_else(EditorMessages::english);
    let styles_enabled = capabilities.map(|state| state.styles).unwrap_or(true);
    let breakpoints = builtin_responsive_breakpoints();
    let states = builtin_style_states();
    let descriptors = builtin_style_properties();
    let Some(component_id) = component_id else {
        return view! {
            <section class="fly-style-rules">
                <h3>{messages.responsive_styles}</h3>
                <p class="fly-style-rules__empty">{messages.no_component_selected}</p>
            </section>
        }
        .into_any();
    };
    let media_property = RwSignal::new("padding".to_string());
    let state_property = RwSignal::new("padding".to_string());
    let breakpoint_id = RwSignal::new(
        breakpoints
            .first()
            .map(|breakpoint| breakpoint.id.clone())
            .unwrap_or_else(|| "mobile-down".to_string()),
    );
    let state_pseudo = RwSignal::new(
        states
            .first()
            .map(|state| state.pseudo.clone())
            .unwrap_or_else(|| "hover".to_string()),
    );
    let media_scope = fly::StyleRuleScope::Media {
        query: breakpoints
            .first()
            .map(|breakpoint| breakpoint.media_query.clone())
            .unwrap_or_default(),
    };
    let rules_signal = RwSignal::new(rules);
    let media_value = RwSignal::new(scoped_style_value(
        &rules_signal.get_untracked(),
        component_id.as_str(),
        &media_scope,
        "padding",
    ));
    let state_scope = fly::StyleRuleScope::State {
        pseudo: state_pseudo.get_untracked(),
    };
    let state_value = RwSignal::new(scoped_style_value(
        &rules_signal.get_untracked(),
        component_id.as_str(),
        &state_scope,
        "padding",
    ));
    let component_for_media = component_id.clone();
    let component_for_media_property = component_id.clone();
    let component_for_apply = component_id.clone();
    let component_for_state = component_id.clone();
    let component_for_state_property = component_id.clone();
    let component_for_state_apply = component_id.clone();
    let component_for_state_clear = component_id;
    let component_for_clear = component_for_apply.clone();
    let breakpoint_label = messages.breakpoint.clone();
    let state_label = messages.interaction_state.clone();
    let property_label = messages.style_property.clone();
    let apply_label = messages.apply.clone();
    let clear_label = messages.clear.clone();
    let preview_button = on_preview_breakpoint.map(|preview| {
        view! {
            <button
                type="button"
                disabled=!styles_enabled
                on:click=move |_| preview.run(breakpoint_id.get_untracked())
            >
                {messages.preview_breakpoint.clone()}
            </button>
        }
    });
    view! {
        <section class="fly-style-rules">
            <h3>{messages.responsive_styles}</h3>
            <div class="fly-style-rules__row">
                <label>{breakpoint_label.clone()}</label>
                <select
                    attr:aria-label=breakpoint_label
                    prop:value=move || breakpoint_id.get()
                    on:change=move |event| {
                        let next = event_target_value(&event);
                        breakpoint_id.set(next.clone());
                        let scope = current_media_scope(next);
                        let catalog = rules_signal.get_untracked();
                        media_value.set(scoped_style_value(
                            &catalog,
                            component_for_media.as_str(),
                            &scope,
                            media_property.get_untracked().as_str(),
                        ));
                    }
                >
                    {breakpoints.clone().into_iter().map(|breakpoint| view! {
                        <option value=breakpoint.id>{breakpoint.label}" · "{breakpoint.media_query}</option>
                    }).collect_view()}
                </select>
                <label>{property_label.clone()}</label>
                <select
                    attr:aria-label=property_label.clone()
                    prop:value=move || media_property.get()
                    on:change=move |event| {
                        let next = event_target_value(&event);
                        media_property.set(next.clone());
                        let scope = current_media_scope(breakpoint_id.get_untracked());
                        let catalog = rules_signal.get_untracked();
                        media_value.set(scoped_style_value(
                            &catalog,
                            component_for_media_property.as_str(),
                            &scope,
                            next.as_str(),
                        ));
                    }
                >
                    {descriptors.iter().map(|descriptor| view! {
                        <option value=descriptor.property>{descriptor.property}</option>
                    }).collect_view()}
                </select>
                <input
                    attr:aria-label=messages.style_value.clone()
                    prop:value=move || media_value.get()
                    disabled=!styles_enabled
                    on:input=move |event| media_value.set(event_target_value(&event))
                />
                <button
                    type="button"
                    disabled=!styles_enabled
                    on:click=move |_| {
                        on_upsert.run(ScopedStyleChange {
                            component_id: component_for_apply.clone(),
                            scope: current_media_scope(breakpoint_id.get_untracked()),
                            property: media_property.get_untracked(),
                            value: media_value.get_untracked(),
                            remove: false,
                        });
                    }
                >
                    {apply_label.clone()}
                </button>
                {preview_button}
                <button
                    type="button"
                    disabled=!styles_enabled
                    on:click=move |_| {
                        on_upsert.run(ScopedStyleChange {
                            component_id: component_for_clear.clone(),
                            scope: current_media_scope(breakpoint_id.get_untracked()),
                            property: media_property.get_untracked(),
                            value: media_value.get_untracked(),
                            remove: true,
                        });
                    }
                >
                    {clear_label.clone()}
                </button>
            </div>
            <h3>{messages.interaction_states}</h3>
            <div class="fly-style-rules__row">
                <label>{state_label.clone()}</label>
                <select
                    attr:aria-label=state_label
                    prop:value=move || state_pseudo.get()
                    on:change=move |event| {
                        let next = event_target_value(&event);
                        state_pseudo.set(next.clone());
                        let scope = fly::StyleRuleScope::State { pseudo: next };
                        let catalog = rules_signal.get_untracked();
                        state_value.set(scoped_style_value(
                            &catalog,
                            component_for_state.as_str(),
                            &scope,
                            state_property.get_untracked().as_str(),
                        ));
                    }
                >
                    {states.into_iter().map(|state| view! {
                        <option value=state.pseudo>{state.label}</option>
                    }).collect_view()}
                </select>
                <label>{property_label.clone()}</label>
                <select
                    attr:aria-label=property_label
                    prop:value=move || state_property.get()
                    on:change=move |event| {
                        let next = event_target_value(&event);
                        state_property.set(next.clone());
                        let scope = fly::StyleRuleScope::State {
                            pseudo: state_pseudo.get_untracked(),
                        };
                        let catalog = rules_signal.get_untracked();
                        state_value.set(scoped_style_value(
                            &catalog,
                            component_for_state_property.as_str(),
                            &scope,
                            next.as_str(),
                        ));
                    }
                >
                    {descriptors.iter().map(|descriptor| view! {
                        <option value=descriptor.property>{descriptor.property}</option>
                    }).collect_view()}
                </select>
                <input
                    attr:aria-label=messages.style_value.clone()
                    prop:value=move || state_value.get()
                    disabled=!styles_enabled
                    on:input=move |event| state_value.set(event_target_value(&event))
                />
                <button
                    type="button"
                    disabled=!styles_enabled
                    on:click=move |_| {
                        on_upsert.run(ScopedStyleChange {
                            component_id: component_for_state_apply.clone(),
                            scope: fly::StyleRuleScope::State {
                                pseudo: state_pseudo.get_untracked(),
                            },
                            property: state_property.get_untracked(),
                            value: state_value.get_untracked(),
                            remove: false,
                        });
                    }
                >
                    {apply_label}
                </button>
                <button
                    type="button"
                    disabled=!styles_enabled
                    on:click=move |_| {
                        on_upsert.run(ScopedStyleChange {
                            component_id: component_for_state_clear.clone(),
                            scope: fly::StyleRuleScope::State {
                                pseudo: state_pseudo.get_untracked(),
                            },
                            property: state_property.get_untracked(),
                            value: state_value.get_untracked(),
                            remove: true,
                        });
                    }
                >
                    {clear_label}
                </button>
            </div>
        </section>
    }
    .into_any()
}

/// Page identity + SEO / Open Graph properties form for one page. The form owns
/// its field state and submits everything at once through `on_apply`; reordering
/// rides `on_move` with a signed delta (`-1` up, `+1` down).
#[component]
pub fn FlyPageProperties(
    summary: PageSummary,
    metadata: PageMetadata,
    page_count: usize,
    on_apply: Callback<PagePropertiesUpdate>,
    #[prop(optional)] on_move: Option<Callback<i32>>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let can_properties = capabilities.map(|state| state.properties).unwrap_or(true);
    let page_name_label = messages.page_name_label.clone();
    let page_id_label = messages.page_id_label.clone();
    let seo_title_label = messages.seo_title.clone();
    let seo_description_label = messages.seo_description.clone();
    let slug_label = messages.slug.clone();
    let canonical_label = messages.canonical_url.clone();
    let og_title_label = messages.og_title.clone();
    let og_description_label = messages.og_description.clone();
    let og_image_label = messages.og_image.clone();
    let no_index_label = messages.no_index.clone();
    let apply_label = messages.apply.clone();
    let move_up_label = messages.move_up.clone();
    let move_down_label = messages.move_down.clone();

    let page_index = summary.index;
    let can_move_up = page_index > 0;
    let can_move_down = page_index + 1 < page_count;
    let name = RwSignal::new(summary.name.clone());
    let page_id = RwSignal::new(summary.id.clone().unwrap_or_default());
    let title = RwSignal::new(metadata.title.clone().unwrap_or_default());
    let description = RwSignal::new(metadata.description.clone().unwrap_or_default());
    let slug = RwSignal::new(metadata.slug.clone().unwrap_or_default());
    let canonical_url = RwSignal::new(metadata.canonical_url.clone().unwrap_or_default());
    let open_graph_title = RwSignal::new(metadata.open_graph_title.clone().unwrap_or_default());
    let open_graph_description =
        RwSignal::new(metadata.open_graph_description.clone().unwrap_or_default());
    let open_graph_image = RwSignal::new(metadata.open_graph_image.clone().unwrap_or_default());
    let no_index = RwSignal::new(metadata.no_index);
    let summary_for_apply = summary;

    view! {
        <form
            class="fly-page-properties"
            on:submit=move |event| {
                event.prevent_default();
                let mut metadata = PageMetadata::default();
                metadata.title = Some(title.get_untracked());
                metadata.description = Some(description.get_untracked());
                metadata.slug = Some(slug.get_untracked());
                metadata.canonical_url = Some(canonical_url.get_untracked());
                metadata.open_graph_title = Some(open_graph_title.get_untracked());
                metadata.open_graph_description =
                    Some(open_graph_description.get_untracked());
                metadata.open_graph_image = Some(open_graph_image.get_untracked());
                metadata.no_index = no_index.get_untracked();
                on_apply.run(PagePropertiesUpdate {
                    summary: summary_for_apply.clone(),
                    name: name.get_untracked().trim().to_string(),
                    page_id: page_id.get_untracked().trim().to_string(),
                    metadata,
                });
            }
        >
            <label class="fly-page-properties__field">
                <span>{page_name_label}</span>
                <input
                    type="text"
                    prop:value=move || name.get()
                    on:input=move |event| name.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{page_id_label}</span>
                <input
                    type="text"
                    prop:value=move || page_id.get()
                    on:input=move |event| page_id.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{seo_title_label}</span>
                <input
                    type="text"
                    prop:value=move || title.get()
                    on:input=move |event| title.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{seo_description_label}</span>
                <textarea
                    prop:value=move || description.get()
                    on:input=move |event| description.set(event_target_value(&event))
                ></textarea>
            </label>
            <label class="fly-page-properties__field">
                <span>{slug_label}</span>
                <input
                    type="text"
                    prop:value=move || slug.get()
                    on:input=move |event| slug.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{canonical_label}</span>
                <input
                    type="text"
                    prop:value=move || canonical_url.get()
                    on:input=move |event| canonical_url.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{og_title_label}</span>
                <input
                    type="text"
                    prop:value=move || open_graph_title.get()
                    on:input=move |event| open_graph_title.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__field">
                <span>{og_description_label}</span>
                <textarea
                    prop:value=move || open_graph_description.get()
                    on:input=move |event| open_graph_description.set(event_target_value(&event))
                ></textarea>
            </label>
            <label class="fly-page-properties__field">
                <span>{og_image_label}</span>
                <input
                    type="text"
                    prop:value=move || open_graph_image.get()
                    on:input=move |event| open_graph_image.set(event_target_value(&event))
                />
            </label>
            <label class="fly-page-properties__check">
                <input
                    type="checkbox"
                    prop:checked=move || no_index.get()
                    on:change=move |event| no_index.set(event_target_checked(&event))
                />
                <span>{no_index_label}</span>
            </label>
            <div class="fly-page-properties__actions">
                <button type="submit" class="fly-page-properties__apply" disabled=!can_properties>
                    {apply_label}
                </button>
                {on_move.map(|on_move| {
                    let up_label = move_up_label.clone();
                    let down_label = move_down_label.clone();
                    view! {
                        <button
                            type="button"
                            class="fly-page-properties__move"
                            disabled=!can_move_up || !can_properties
                            on:click=move |_| on_move.run(-1)
                        >
                            {up_label}
                        </button>
                        <button
                            type="button"
                            class="fly-page-properties__move"
                            disabled=!can_move_down || !can_properties
                            on:click=move |_| on_move.run(1)
                        >
                            {down_label}
                        </button>
                    }
                })}
            </div>
        </form>
    }
}

/// Property editor for the selected component: tag, text content, arbitrary
/// attributes (with per-attribute removal), and one inline style property at a
/// time from the shared builtin style catalogue. Every accepted edit leaves as
/// a [`ComponentPropertyChange`]; the workspace maps it to
/// `EditorCommand::Patch` locally or to a `patch_component_property` envelope
/// in intent mode — the same contract the SSR inspector forms use. The two
/// sections honour `CapabilityState::properties` / `styles` independently.
#[component]
pub fn FlyComponentProperties(
    view: Option<ComponentPropertyView>,
    on_patch: Callback<ComponentPropertyChange>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let properties_label = messages.properties.clone();
    let styles_label = messages.styles.clone();
    let selected_label = messages.selected_component.clone();
    let no_selection_label = messages.no_selection.clone();
    let type_label = messages.component_type.clone();
    let tag_label = messages.tag_name.clone();
    let content_label = messages.content.clone();
    let attribute_name_label = messages.attribute_name.clone();
    let attribute_value_label = messages.attribute_value.clone();
    let attributes_label = messages.attributes.clone();
    let apply_label = messages.apply.clone();
    let clear_label = messages.clear.clone();
    let clear_label_for_list = messages.clear.clone();
    let style_property_label = messages.style_property.clone();
    let style_value_label = messages.style_value.clone();
    let can_properties = capabilities.map(|state| state.properties).unwrap_or(true);
    let can_styles = capabilities.map(|state| state.styles).unwrap_or(true);

    let component_id = view.as_ref().map(|view| view.id.clone());
    let component_type = view
        .as_ref()
        .map(|view| view.component_type.clone())
        .unwrap_or_default();
    let seeded_attributes = view
        .as_ref()
        .map(|view| view.attributes.clone())
        .unwrap_or_default();
    let seeded_tag = view
        .as_ref()
        .and_then(|view| view.tag_name.clone())
        .unwrap_or_default();
    let seeded_content = view
        .as_ref()
        .and_then(|view| view.content.clone())
        .unwrap_or_default();
    let seeded_style = view
        .as_ref()
        .map(|view| view.style.clone())
        .unwrap_or_default();
    let has_selection = component_id.is_some();
    let selected_display = component_id.clone().unwrap_or(no_selection_label);
    let tag_name = RwSignal::new(seeded_tag);
    let content_value = RwSignal::new(seeded_content);
    let attribute_name = RwSignal::new(String::new());
    let attribute_value = RwSignal::new(String::new());
    let style_property = RwSignal::new("padding".to_string());
    let style_error = RwSignal::new(Option::<String>::None);
    let initial_style_value = seeded_style
        .get("padding")
        .and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let style_value = RwSignal::new(initial_style_value);
    let style_lookup = seeded_style.clone();
    let component_for_tag = component_id.clone();
    let component_for_content = component_id.clone();
    let attribute_error_label = attribute_name_label.clone();
    let component_for_attribute = component_id.clone();
    let component_for_style = component_id.clone();
    let descriptors = builtin_style_properties();

    view! {
        <section class="fly-component-properties">
            <h2>{properties_label}</h2>
            <dl class="fly-component-properties__summary">
                <div>
                    <dt>{selected_label}</dt>
                    <dd>{selected_display}</dd>
                </div>
                <div>
                    <dt>{type_label}</dt>
                    <dd>{component_type}</dd>
                </div>
            </dl>
            <fieldset disabled=!can_properties || !has_selection>
                <label class="fly-component-properties__field">
                    <span>{tag_label.clone()}</span>
                    <input
                        type="text"
                        aria-label=tag_label
                        prop:value=move || tag_name.get()
                        on:input=move |event| tag_name.set(event_target_value(&event))
                    />
                </label>
                <button
                    type="button"
                    class="fly-component-properties__apply"
                    on:click={
                        let component_id = component_for_tag.clone();
                        move |_| {
                            if let Some(component_id) = &component_id {
                                let value = tag_name.get_untracked();
                                on_patch.run(ComponentPropertyChange {
                                    component_id: component_id.clone(),
                                    kind: ComponentPropertyKind::Field,
                                    name: "tagName".to_string(),
                                    remove: value.trim().is_empty(),
                                    value,
                                });
                            }
                        }
                    }
                >
                    {apply_label.clone()}
                </button>
                <label class="fly-component-properties__field">
                    <span>{content_label.clone()}</span>
                    <textarea
                        aria-label=content_label
                        prop:value=move || content_value.get()
                        on:input=move |event| content_value.set(event_target_value(&event))
                    ></textarea>
                </label>
                <div class="fly-component-properties__row">
                    <button
                        type="button"
                        class="fly-component-properties__apply"
                        on:click={
                            let component_id = component_for_content.clone();
                            move |_| {
                                if let Some(component_id) = &component_id {
                                    on_patch.run(ComponentPropertyChange {
                                        component_id: component_id.clone(),
                                        kind: ComponentPropertyKind::Field,
                                        name: "content".to_string(),
                                        value: content_value.get_untracked(),
                                        remove: false,
                                    });
                                }
                            }
                        }
                    >
                        {apply_label.clone()}
                    </button>
                    <button
                        type="button"
                        class="fly-component-properties__clear"
                        on:click={
                            let component_id = component_for_content;
                            move |_| {
                                if let Some(component_id) = &component_id {
                                    on_patch.run(ComponentPropertyChange {
                                        component_id: component_id.clone(),
                                        kind: ComponentPropertyKind::Field,
                                        name: "content".to_string(),
                                        value: String::new(),
                                        remove: true,
                                    });
                                }
                            }
                        }
                    >
                        {clear_label.clone()}
                    </button>
                </div>
                <div class="fly-component-properties__attributes">
                    <h3>{attributes_label}</h3>
                    <label class="fly-component-properties__field">
                        <span>{attribute_name_label.clone()}</span>
                        <input
                            type="text"
                            aria-label=attribute_name_label
                            prop:value=move || attribute_name.get()
                            on:input=move |event| attribute_name.set(event_target_value(&event))
                        />
                    </label>
                    <label class="fly-component-properties__field">
                        <span>{attribute_value_label.clone()}</span>
                        <input
                            type="text"
                            aria-label=attribute_value_label
                            prop:value=move || attribute_value.get()
                            on:input=move |event| attribute_value.set(event_target_value(&event))
                        />
                    </label>
                    <button
                        type="button"
                        class="fly-component-properties__apply"
                        on:click={
                            let attribute_error_label = attribute_error_label.clone();
                            let component_id = component_for_attribute.clone();
                            move |_| {
                                let name = attribute_name.get_untracked().trim().to_string();
                                if name.is_empty() {
                                    style_error.set(Some(format!(
                                        "{attribute_error_label} must not be empty"
                                    )));
                                    return;
                                }
                                style_error.set(None);
                                if let Some(component_id) = &component_id {
                                    on_patch.run(ComponentPropertyChange {
                                        component_id: component_id.clone(),
                                        kind: ComponentPropertyKind::Attribute,
                                        name,
                                        value: attribute_value.get_untracked(),
                                        remove: false,
                                    });
                                    attribute_name.set(String::new());
                                    attribute_value.set(String::new());
                                }
                            }
                        }
                    >
                        {apply_label.clone()}
                    </button>
                    <ul class="fly-component-properties__attribute-list">
                        {seeded_attributes
                            .into_iter()
                            .map(|(name, value)| {
                                let component_id = component_for_attribute.clone();
                                let remove_name = name.clone();
                                let clear_accessible =
                                    format!("{clear_label_for_list}: {name}");
                                view! {
                                    <li>
                                        <code>{format!("{name}={value}")}</code>
                                        <button
                                            type="button"
                                            aria-label=clear_accessible
                                            on:click=move |_| {
                                                if let Some(component_id) = &component_id {
                                                    on_patch.run(ComponentPropertyChange {
                                                        component_id: component_id.clone(),
                                                        kind: ComponentPropertyKind::Attribute,
                                                        name: remove_name.clone(),
                                                        value: String::new(),
                                                        remove: true,
                                                    });
                                                }
                                            }
                                        >
                                            {clear_label_for_list.clone()}
                                        </button>
                                    </li>
                                }
                            })
                            .collect_view()}
                    </ul>
                </div>
            </fieldset>
            <fieldset disabled=!can_styles || !has_selection>
                <h3>{styles_label}</h3>
                <label class="fly-component-properties__field">
                    <span>{style_property_label.clone()}</span>
                    <select
                        aria-label=style_property_label
                        prop:value=move || style_property.get()
                        on:change=move |event| {
                            let property = event_target_value(&event);
                            let value = style_lookup
                                .get(&property)
                                .and_then(|value| match value {
                                    serde_json::Value::String(value) => Some(value.clone()),
                                    serde_json::Value::Number(value) => {
                                        Some(value.to_string())
                                    }
                                    serde_json::Value::Bool(value) => Some(value.to_string()),
                                    _ => None,
                                })
                                .unwrap_or_default();
                            style_property.set(property);
                            style_value.set(value);
                        }
                    >
                        {descriptors
                            .into_iter()
                            .map(|descriptor| {
                                view! {
                                    <option value=descriptor.property>
                                        {format!("{:?} · {}", descriptor.group, descriptor.label)}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </label>
                <label class="fly-component-properties__field">
                    <span>{style_value_label.clone()}</span>
                    <input
                        type="text"
                        aria-label=style_value_label
                        prop:value=move || style_value.get()
                        on:input=move |event| style_value.set(event_target_value(&event))
                    />
                </label>
                <div class="fly-component-properties__row">
                    <button
                        type="button"
                        class="fly-component-properties__apply"
                        on:click={
                            let component_id = component_for_style.clone();
                            move |_| {
                                if let Some(component_id) = &component_id {
                                    let property = style_property.get_untracked();
                                    let value = style_value.get_untracked();
                                    let remove = value.trim().is_empty();
                                    match style_patch([StyleEntry {
                                        property: property.clone(),
                                        value: value.clone(),
                                    }]) {
                                        Ok(_) => {
                                            style_error.set(None);
                                            on_patch.run(ComponentPropertyChange {
                                                component_id: component_id.clone(),
                                                kind: ComponentPropertyKind::Style,
                                                name: property,
                                                value,
                                                remove,
                                            });
                                        }
                                        Err(errors) => {
                                            style_error.set(Some(
                                                errors
                                                    .into_iter()
                                                    .map(|error| {
                                                        format!(
                                                            "{}: {}",
                                                            error.property, error.message
                                                        )
                                                    })
                                                    .collect::<Vec<_>>()
                                                    .join("; "),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    >
                        {apply_label.clone()}
                    </button>
                    <button
                        type="button"
                        class="fly-component-properties__clear"
                        on:click={
                            let component_id = component_for_style;
                            move |_| {
                                if let Some(component_id) = &component_id {
                                    on_patch.run(ComponentPropertyChange {
                                        component_id: component_id.clone(),
                                        kind: ComponentPropertyKind::Style,
                                        name: style_property.get_untracked(),
                                        value: String::new(),
                                        remove: true,
                                    });
                                }
                            }
                        }
                    >
                        {clear_label}
                    </button>
                </div>
                {move || {
                    style_error
                        .get()
                        .map(|message| {
                            view! {
                                <p class="fly-component-properties__error" role="alert">
                                    {message}
                                </p>
                            }
                        })
                }}
            </fieldset>
        </section>
    }
}

/// Document asset catalogue panel: upsert by id/URL, remove, and apply an
/// asset to the selected component's `src` attribute. Edits leave as typed
/// callbacks; the workspace turns them into `EditorCommand::Asset` locally or
/// `upsert_asset`/`remove_asset`/`select_asset` envelopes in intent mode — the
/// contracts the admin SSR asset forms use. (Host media-library provider
/// browsing stays a host panel: it needs the host's provider port, which is
/// deliberately not part of the neutral workspace.)
#[component]
pub fn FlyAssets(
    assets: Vec<(String, Option<String>, String)>,
    selected_component: Option<String>,
    on_upsert: Callback<(Option<String>, String)>,
    on_remove: Callback<String>,
    on_apply: Callback<(String, String)>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
    #[prop(optional)] messages: Option<EditorMessages>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let assets_label = messages.assets.clone();
    let asset_id_label = messages.asset_id.clone();
    let asset_url_label = messages.asset_url.clone();
    let add_label = messages.add_asset.clone();
    let use_label = messages.use_asset.clone();
    let remove_label = messages.remove.clone();
    let can_assets = capabilities.map(|state| state.assets).unwrap_or(true);
    let can_apply = capabilities.map(|state| state.properties).unwrap_or(true);
    let has_selection = selected_component.is_some();
    let selected_for_apply = selected_component;
    let asset_id = RwSignal::new(String::new());
    let asset_url = RwSignal::new(String::new());

    view! {
        <section class="fly-assets">
            <h2>{assets_label}</h2>
            <fieldset disabled=!can_assets>
                <label class="fly-assets__field">
                    <span>{asset_id_label.clone()}</span>
                    <input
                        type="text"
                        aria-label=asset_id_label
                        prop:value=move || asset_id.get()
                        on:input=move |event| asset_id.set(event_target_value(&event))
                    />
                </label>
                <label class="fly-assets__field">
                    <span>{asset_url_label.clone()}</span>
                    <input
                        type="text"
                        aria-label=asset_url_label
                        prop:value=move || asset_url.get()
                        on:input=move |event| asset_url.set(event_target_value(&event))
                    />
                </label>
                <button
                    type="button"
                    class="fly-assets__add"
                    on:click=move |_| {
                        let source = asset_url.get_untracked().trim().to_string();
                        if !source.is_empty() {
                            let id = asset_id.get_untracked().trim().to_string();
                            on_upsert.run(((!id.is_empty()).then_some(id), source));
                            asset_id.set(String::new());
                            asset_url.set(String::new());
                        }
                    }
                >
                    {add_label}
                </button>
            </fieldset>
            <ul class="fly-assets__list">
                {assets
                    .into_iter()
                    .map(|(id, name, source)| {
                        let display = name.clone().unwrap_or_else(|| id.clone());
                        let remove_id = id.clone();
                        let apply_id = id.clone();
                        let selected_component = selected_for_apply.clone();
                        view! {
                            <li>
                                <div class="fly-assets__name">{display}</div>
                                <div class="fly-assets__source">{source}</div>
                                <div class="fly-assets__row">
                                    <button
                                        type="button"
                                        disabled=!can_apply || !has_selection
                                        on:click=move |_| {
                                            if let Some(component_id) = &selected_component {
                                                on_apply.run((
                                                    component_id.clone(),
                                                    apply_id.clone(),
                                                ));
                                            }
                                        }
                                    >
                                        {use_label.clone()}
                                    </button>
                                    <button
                                        type="button"
                                        class="fly-assets__remove"
                                        disabled=!can_assets
                                        on:click=move |_| on_remove.run(remove_id.clone())
                                    >
                                        {remove_label.clone()}
                                    </button>
                                </div>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

/// Full editor workspace: palette, page tabs (switch, create, remove), page properties (rename, SEO, move), layer tree, canvas preview, design-token editor, and undo/redo, wired to a
/// `FlyEditor` + `FlyUiStateMachine` held in local signals.
///
/// The host passes the initial document and an `on_change` callback that fires with the
/// new document after every accepted command; persistence, permissions, and transport
/// remain the host's business (AGENTS/FFA boundary). Registries default to the builtins;
/// hosts with provider blocks pass their own through `registries`. When the host also
/// passes a `contribution_assembly`, the palette is restricted by contribution access —
/// namespaced provider blocks appear (and insert) only when an active contribution
/// declares them, the same policy the page-builder admin enforces.
///
/// Panel labels come from `messages`: the host resolves an
/// [`EditorMessages`](fly_ui::EditorMessages) from its own message catalog for the
/// current locale (English by default), keeping the adapters platform-neutral.
///
/// When the host passes `on_intent`, the workspace runs in SSR intent mode instead:
/// palette inserts, selection, and undo/redo are emitted as typed
/// [`BrowserIntentEnvelope`]s for the host to POST to its intent endpoint, and the
/// host re-renders with the server-returned document — the workspace mutates nothing
/// locally. Design-token edits ride the same transport (`set_theme_token` /
/// `remove_theme_token` intents).
#[component]
pub fn FlyEditorWorkspace(
    document: ProjectDocument,
    #[prop(optional)] registries: Option<RegistrySet>,
    #[prop(optional)] contribution_assembly: Option<ContributionAssemblyResult>,
    #[prop(optional)] instance_id: Option<String>,
    #[prop(optional)] on_intent: Option<Callback<BrowserIntentEnvelope>>,
    #[prop(optional)] on_change: Option<Callback<ProjectDocument>>,
    #[prop(optional)] on_error: Option<Callback<String>>,
    #[prop(optional)] messages: Option<EditorMessages>,
    #[prop(optional)] initial_page: Option<usize>,
    #[prop(optional)] capabilities: Option<CapabilityState>,
) -> impl IntoView {
    let messages = messages.unwrap_or_default();
    let workspace_title = messages.workspace_title.clone();
    let toolbar_label = messages.toolbar.clone();
    let undo_label = messages.undo.clone();
    let redo_label = messages.redo.clone();
    let page_summaries = document.page_summaries();
    let active_page = RwSignal::new(initial_page.unwrap_or(0));
    let registries = registries.unwrap_or_else(RegistrySet::with_builtins);
    let summary_assembly = contribution_assembly.clone();
    let palette_access = PaletteBlockAccess::from_optional_assembly(contribution_assembly.as_ref());
    let blocks = palette_block_items_with_access(&registries, &palette_access);
    let intent_instance = instance_id.unwrap_or_else(|| "fly-workspace".to_string());
    let editor = RwSignal::new(FlyEditor::new(document, registries));
    let ui = RwSignal::new(FlyUiStateMachine::new(Presentation::Full));

    let palette_editor = editor;
    let palette_ui = ui;
    let insert_instance = intent_instance.clone();
    let on_insert = Callback::new(move |block_id: String| {
        if !palette_access.allows(&block_id) {
            return;
        }
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::insert_block(
                &insert_instance,
                &block_id,
            ));
            return;
        }
        let template = palette_editor.with(|editor| {
            editor
                .registries()
                .blocks
                .get(block_id.as_str())
                .map(|block| block.component.clone())
        });
        if let Some(component) = template {
            let apply_result = palette_editor.try_update(|editor| {
                editor.apply(fly::EditorCommand::Insert {
                    parent_id: None,
                    index: usize::MAX,
                    component,
                })
            });
            if let Some(Err(error)) = &apply_result {
                report_workspace_error(on_error, "Insert block failed", error);
            }
            if let Some(on_change) = on_change {
                palette_editor.with(|editor| on_change.run(editor.document().clone()));
            }
        }
        palette_ui.update(|machine| {
            let _ = machine.dispatch(UiIntent::ReplaceDiagnostics(Vec::new()));
        });
    });

    let activate_instance = intent_instance.clone();
    let activate_ui = ui;
    let activate_editor = editor;
    let on_activate = Callback::new(move |page_index: usize| {
        let page_id = activate_editor.with(|editor| {
            editor
                .document()
                .page_summaries()
                .into_iter()
                .find(|page| page.index == page_index)
                .and_then(|page| page.id)
        });
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::activate_page(
                &activate_instance,
                page_index,
                page_id.as_deref(),
            ));
            return;
        }
        active_page.set(page_index);
        activate_ui.update(|machine| {
            let _ = machine.dispatch(UiIntent::ActivatePage {
                page_id,
                page_index,
            });
        });
    });

    let untitled_page_label = messages.untitled_page.clone();
    let add_editor = editor;
    let add_ui = ui;
    let add_instance = intent_instance.clone();
    let on_add_page = Callback::new(move |name: String| {
        let name = name.trim().to_string();
        let name = if name.is_empty() {
            untitled_page_label.clone()
        } else {
            name
        };
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::create_page(&add_instance, &name));
            return;
        }
        let (page_id, page_index) = add_editor.with(|editor| {
            let summaries = editor.document().page_summaries();
            (unique_workspace_page_id(&summaries, &name), summaries.len())
        });
        let apply_result = add_editor.try_update(|editor| {
            editor.apply(EditorCommand::Page {
                command: PageCommand::Add {
                    index: page_index,
                    page: Box::new(blank_page(page_id.clone(), name.clone())),
                },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Add page failed", error);
        }
        active_page.set(page_index);
        add_ui.update(|machine| {
            let _ = machine.dispatch(UiIntent::ActivatePage {
                page_id: Some(page_id),
                page_index,
            });
        });
        if let Some(on_change) = on_change {
            add_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let remove_editor = editor;
    let remove_instance = intent_instance.clone();
    let on_remove_page = Callback::new(move |page_index: usize| {
        if let Some(on_intent) = on_intent {
            let page_id = remove_editor.with(|editor| {
                editor
                    .document()
                    .page_summaries()
                    .into_iter()
                    .find(|page| page.index == page_index)
                    .and_then(|page| page.id)
            });
            on_intent.run(BrowserIntentEnvelope::remove_page(
                &remove_instance,
                page_index,
                page_id.as_deref(),
            ));
            return;
        }
        let apply_result = remove_editor.try_update(|editor| {
            editor.apply(EditorCommand::Page {
                command: PageCommand::Remove {
                    locator: PageLocator::by_index(page_index),
                },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Remove page failed", error);
        }
        let remaining = remove_editor.with(|editor| editor.document().project.pages.len());
        let current = active_page.get_untracked();
        let clamped = if remaining == 0 {
            0
        } else if page_index < current {
            current.saturating_sub(1)
        } else if current >= remaining {
            remaining - 1
        } else {
            current
        };
        active_page.set(clamped);
        if let Some(on_change) = on_change {
            remove_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let properties_editor = editor;
    let properties_instance = intent_instance.clone();
    let on_apply_page_properties = Callback::new(move |update: PagePropertiesUpdate| {
        if let Some(on_intent) = on_intent {
            let metadata = serde_json::to_value(update.metadata.normalized())
                .unwrap_or(serde_json::Value::Null);
            on_intent.run(BrowserIntentEnvelope::page_properties(
                &properties_instance,
                update.summary.index,
                update.summary.id.as_deref(),
                &update.name,
                (!update.page_id.is_empty()).then_some(update.page_id.as_str()),
                metadata,
            ));
            return;
        }
        let mut fields = update.metadata.normalized().into_page_patch().fields;
        if !update.name.is_empty() {
            fields.insert(
                "name".to_string(),
                serde_json::Value::String(update.name.clone()),
            );
        }
        if !update.page_id.is_empty() {
            fields.insert(
                "id".to_string(),
                serde_json::Value::String(update.page_id.clone()),
            );
        }
        let apply_result = properties_editor.try_update(|editor| {
            editor.apply(EditorCommand::Page {
                command: PageCommand::Patch {
                    locator: PageLocator::by_index(update.summary.index),
                    patch: PagePatch {
                        fields,
                        ..PagePatch::default()
                    },
                },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Save page properties failed", error);
        }
        if let Some(on_change) = on_change {
            properties_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let patch_editor = editor;
    let patch_instance = intent_instance.clone();
    let on_patch_property = Callback::new(move |change: ComponentPropertyChange| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::patch_component_property(
                &patch_instance,
                &change.component_id,
                change.kind.as_str(),
                &change.name,
                &change.value,
                change.remove,
            ));
            return;
        }
        let mut patch = fly::ComponentPatch::default();
        match change.kind {
            ComponentPropertyKind::Field => {
                if change.remove {
                    patch.remove_fields.push(change.name.clone());
                } else {
                    patch.fields.insert(
                        change.name.clone(),
                        serde_json::Value::String(change.value.clone()),
                    );
                }
            }
            ComponentPropertyKind::Attribute => {
                if change.remove {
                    patch.remove_attributes.push(change.name.clone());
                } else {
                    patch.attributes.insert(
                        change.name.clone(),
                        serde_json::Value::String(change.value.clone()),
                    );
                }
            }
            ComponentPropertyKind::Style => {
                let mut style = serde_json::Map::new();
                style.insert(
                    change.name.clone(),
                    if change.remove {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(change.value.clone())
                    },
                );
                patch.style = Some(serde_json::Value::Object(style));
            }
        }
        let apply_result = patch_editor.try_update(|editor| {
            editor.apply(fly::EditorCommand::Patch {
                component_id: change.component_id.clone(),
                patch,
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Save component property failed", error);
        }
        if let Some(on_change) = on_change {
            patch_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let asset_editor = editor;
    let asset_instance = intent_instance.clone();
    let asset_remove_instance = intent_instance.clone();
    let asset_apply_instance = intent_instance.clone();
    let preview_ui = ui;
    let preview_width = RwSignal::new(None::<u32>);
    let on_preview_breakpoint = Callback::new(move |breakpoint_id: String| {
        preview_ui.update(|machine| {
            let current = machine.state.viewport;
            if let Some(next) = preview_viewport_for_breakpoint(&breakpoint_id, current) {
                let _ = machine.dispatch(UiIntent::SetViewport(next));
                preview_width.set(Some(machine.state.viewport.width));
            }
        });
    });
    let on_upsert_style = Callback::new({
        let intent_instance = instance_id.clone();
        let apply_editor = editor;
        move |change: ScopedStyleChange| {
            if let Some(on_intent) = on_intent {
                let (scope_kind, scope_value) = match &change.scope {
                    fly::StyleRuleScope::Media { query } => ("media", query.clone()),
                    fly::StyleRuleScope::State { pseudo } => ("state", pseudo.clone()),
                    _ => return,
                };
                on_intent.run(BrowserIntentEnvelope::upsert_style_rule(
                    intent_instance.clone(),
                    change.component_id.as_str(),
                    scope_kind,
                    scope_value.as_str(),
                    change.property.as_str(),
                    change.value.as_str(),
                    change.remove,
                ));
                return;
            }
            let entry = fly_ui::StyleEntry {
                property: change.property.clone(),
                value: if change.remove {
                    String::new()
                } else {
                    change.value.clone()
                },
            };
            let Ok(patch) = style_patch([entry]) else {
                report_workspace_error_raw(on_error, "Scoped style validation failed");
                return;
            };
            let mut declarations = patch
                .style
                .and_then(|style| style.as_object().cloned())
                .unwrap_or_default();
            if change.remove {
                declarations.remove(change.property.as_str());
            }
            let apply_result = apply_editor.try_update(|editor| {
                editor.apply(fly::EditorCommand::StyleRule {
                    command: fly::StyleRuleCommand::UpsertComponentRule {
                        component_id: change.component_id.clone(),
                        scope: change.scope.clone(),
                        declarations,
                        remove_properties: patch.remove_style_properties,
                    },
                })
            });
            if let Some(Err(error)) = &apply_result {
                report_workspace_error(on_error, "Save scoped style failed", error);
                return;
            }
            if let Some(on_change) = on_change {
                if let Some(document) = apply_editor.try_with(|editor| editor.document().clone()) {
                    on_change.run(document);
                }
            }
        }
    });
    let on_upsert_asset = Callback::new(move |(asset_id, source): (Option<String>, String)| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::upsert_asset(
                &asset_instance,
                asset_id.as_deref(),
                &source,
                None,
            ));
            return;
        }
        let asset = match &asset_id {
            Some(asset_id) => serde_json::json!({ "id": asset_id, "src": source }),
            None => serde_json::json!({ "src": source }),
        };
        let apply_result = asset_editor.try_update(|editor| {
            editor.apply(EditorCommand::Asset {
                command: AssetCommand::Upsert { asset },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Save asset failed", error);
        }
        if let Some(on_change) = on_change {
            asset_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });
    let on_remove_asset = Callback::new(move |asset_id: String| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::remove_asset(
                &asset_remove_instance,
                &asset_id,
            ));
            return;
        }
        let apply_result = asset_editor.try_update(|editor| {
            editor.apply(EditorCommand::Asset {
                command: AssetCommand::Remove { asset_id },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Remove asset failed", error);
        }
        if let Some(on_change) = on_change {
            asset_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });
    let on_apply_asset = Callback::new(move |(component_id, asset_id): (String, String)| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::select_asset(
                &asset_apply_instance,
                &component_id,
                &asset_id,
                "src",
            ));
            return;
        }
        let source = asset_editor.with(|editor| {
            AssetCatalog::from_document(editor.document())
                .assets
                .into_iter()
                .find(|asset| asset.id == asset_id)
                .map(|asset| asset.source)
        });
        if let Some(source) = source {
            let apply_result = asset_editor.try_update(|editor| {
                editor.apply(fly::EditorCommand::Patch {
                    component_id: component_id.clone(),
                    patch: fly::ComponentPatch {
                        attributes: serde_json::Map::from_iter([(
                            "src".to_string(),
                            serde_json::Value::String(source),
                        )]),
                        ..fly::ComponentPatch::default()
                    },
                })
            });
            if let Some(Err(error)) = &apply_result {
                report_workspace_error(on_error, "Apply asset failed", error);
            }
            if let Some(on_change) = on_change {
                asset_editor.with(|editor| on_change.run(editor.document().clone()));
            }
        }
    });

    let move_editor = editor;
    let move_instance = intent_instance.clone();
    let on_move_page = Callback::new(move |delta: i32| {
        let current = active_page.get_untracked();
        let (page_id, target) = move_editor.with(|editor| {
            let summaries = editor.document().page_summaries();
            let page_id = summaries.get(current).and_then(|page| page.id.clone());
            let target = if delta < 0 {
                current.saturating_sub(1)
            } else {
                (current + 2).min(summaries.len())
            };
            (page_id, target)
        });
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::move_page(
                &move_instance,
                current,
                page_id.as_deref(),
                target,
            ));
            return;
        }
        let apply_result = move_editor.try_update(|editor| {
            editor.apply(EditorCommand::Page {
                command: PageCommand::Move {
                    locator: PageLocator::by_index(current),
                    index: target,
                },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Move page failed", error);
        }
        let new_active = if delta < 0 {
            current.saturating_sub(1)
        } else {
            current + 1
        };
        active_page.set(new_active);
        if let Some(on_change) = on_change {
            move_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let theme_editor = editor;
    let theme_upsert_instance = intent_instance.clone();
    let on_token_upsert = Callback::new(move |token: DesignToken| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::set_theme_token(
                &theme_upsert_instance,
                &token.name,
                &token.value,
            ));
            return;
        }
        let apply_result = theme_editor.try_update(|editor| {
            editor.apply(EditorCommand::Theme {
                command: ThemeCommand::Upsert { token },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Save design token failed", error);
        }
        if let Some(on_change) = on_change {
            theme_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });
    let theme_remove_editor = editor;
    let theme_remove_instance = intent_instance.clone();
    let on_token_remove = Callback::new(move |name: String| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::remove_theme_token(
                &theme_remove_instance,
                &name,
            ));
            return;
        }
        let apply_result = theme_remove_editor.try_update(|editor| {
            editor.apply(EditorCommand::Theme {
                command: ThemeCommand::Remove { name },
            })
        });
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Remove design token failed", error);
        }
        if let Some(on_change) = on_change {
            theme_remove_editor.with(|editor| on_change.run(editor.document().clone()));
        }
    });

    let layers_editor = editor;
    let select_instance = intent_instance.clone();
    let on_select = Callback::new(move |component_id: Option<String>| {
        if let Some(on_intent) = on_intent {
            on_intent.run(BrowserIntentEnvelope::select(
                &select_instance,
                component_id.as_deref(),
            ));
            return;
        }
        let apply_result = layers_editor
            .try_update(|editor| editor.apply(fly::EditorCommand::Select { component_id }));
        if let Some(Err(error)) = &apply_result {
            report_workspace_error(on_error, "Select component failed", error);
        }
    });

    // The properties form is fed through a memo so it is only re-created when
    // the *page itself* (summary or metadata) actually changes: unrelated
    // document mutations publish nothing new, and unsubmitted form edits are
    // never clobbered by an insert/undo elsewhere in the workspace.
    let properties_snapshot = Memo::new(move |_| {
        let active = active_page.get();
        editor.with(|editor| {
            let summaries = editor.document().page_summaries();
            let summary = summaries
                .get(active)
                .cloned()
                .or_else(|| summaries.first().cloned());
            let metadata = summary
                .as_ref()
                .and_then(|summary| editor.document().project.pages.get(summary.index))
                .map(PageMetadata::from_page)
                .unwrap_or_default();
            (summary, metadata, summaries.len())
        })
    });

    let asset_snapshot = Memo::new(move |_| {
        editor.with(|editor| {
            AssetCatalog::from_document(editor.document())
                .assets
                .into_iter()
                .map(|asset| (asset.id, asset.name, asset.source))
                .collect::<Vec<_>>()
        })
    });
    let selection_snapshot = Memo::new(move |_| {
        editor.with(|editor| {
            editor
                .selection()
                .and_then(|selected| component_property_view(editor.document(), selected))
        })
    });

    let theme_messages = messages.clone();
    let tabs_messages = messages.clone();
    let properties_messages = messages.clone();
    let component_messages = messages.clone();
    let asset_messages = messages.clone();
    let panel_messages = messages;
    let undo_editor = editor;
    let redo_editor = editor;
    let undo_instance = intent_instance.clone();
    let redo_instance = intent_instance;
    view! {
        <section class="fly-editor fly-editor--full" role="application" aria-label=workspace_title>
            <div class="fly-editor__toolbar" role="toolbar" aria-label=toolbar_label>
                <button
                    type="button"
                    disabled=move || {
                        on_intent.is_none()
                            && !undo_editor.with(|editor| editor.history().can_undo())
                    }
                    on:click=move |_| {
                        if let Some(on_intent) = on_intent {
                            on_intent.run(BrowserIntentEnvelope::undo(&undo_instance));
                            return;
                        }
                        undo_editor.update(|editor| {
                            let _ = editor.undo();
                        });
                        if let Some(on_change) = on_change {
                            undo_editor.with(|editor| on_change.run(editor.document().clone()));
                        }
                    }
                >
                    {undo_label}
                </button>
                <button
                    type="button"
                    disabled=move || {
                        on_intent.is_none()
                            && !redo_editor.with(|editor| editor.history().can_redo())
                    }
                    on:click=move |_| {
                        if let Some(on_intent) = on_intent {
                            on_intent.run(BrowserIntentEnvelope::redo(&redo_instance));
                            return;
                        }
                        redo_editor.update(|editor| {
                            let _ = editor.redo();
                        });
                        if let Some(on_change) = on_change {
                            redo_editor.with(|editor| on_change.run(editor.document().clone()));
                        }
                    }
                >
                    {redo_label}
                </button>
            </div>
            {move || {
                let active = active_page.get();
                view! {
                    <FlyPageTabs
                        pages=page_summaries.clone()
                        active=active
                        on_activate=on_activate
                        on_add=on_add_page
                        on_remove=on_remove_page
                        capabilities=capabilities
                        messages=Some(tabs_messages.clone())
                    />
                }
            }}
            <div class="fly-editor__body">
                {summary_assembly.map(|assembly| {
                    view! {
                        <FlyContributionSummary
                            assembly=assembly
                            messages=Some(theme_messages.clone())
                        />
                    }
                })}
                <FlyPalette
                    blocks=blocks
                    on_insert=on_insert
                    capabilities=capabilities
                    messages=Some(theme_messages.clone())
                />
                {move || {
                    let tokens = editor.with(|editor| theme_token_items(editor.document()));
                    view! {
                        <FlyThemeTokenEditor
                            tokens=tokens
                            on_upsert=on_token_upsert
                            on_remove=on_token_remove
                            capabilities=capabilities
                            messages=Some(theme_messages.clone())
                        />
                    }
                }}
                {move || {
                    let snapshot = properties_snapshot.get();
                    match snapshot {
                        (Some(summary), metadata, page_count) => view! {
                            <FlyPageProperties
                                summary=summary
                                metadata=metadata
                                page_count=page_count
                                on_apply=on_apply_page_properties
                                on_move=on_move_page
                                capabilities=capabilities
                                messages=Some(properties_messages.clone())
                            />
                        }
                        .into_any(),
                        _ => view! { <div class="fly-page-properties fly-page-properties--empty"></div> }
                            .into_any(),
                    }
                }}
                {move || {
                    let document = editor.with(|editor| editor.document().clone());
                    let selected = editor.with(|editor| editor.selection().map(ToString::to_string));
                    let page_index = active_page.get();
                    view! {
                        <FlyLayerTree
                            document=document.clone()
                            selected=selected
                            on_select=on_select
                            page_index=page_index
                            messages=Some(panel_messages.clone())
                        />
                        <FlyCanvasPreview
                            document=document
                            page_index=Some(page_index)
                            viewport_width=preview_width.get()
                            messages=Some(panel_messages.clone())
                        />
                    }
                }}
                {move || {
                    let view = selection_snapshot.get();
                    view! {
                        <FlyComponentProperties
                            view=view
                            on_patch=on_patch_property
                            capabilities=capabilities
                            messages=Some(component_messages.clone())
                        />
                    }
                }}
                {move || {
                    let document = editor.with(|editor| editor.document().clone());
                    let component_id =
                        editor.with(|editor| editor.selection().map(ToString::to_string));
                    let catalog = fly::StyleRuleCatalog::from_document(&document);
                    view! {
                        <FlyStyleRules
                            component_id=component_id
                            rules=catalog
                            on_upsert=on_upsert_style
                            on_preview_breakpoint=on_preview_breakpoint
                            capabilities=capabilities
                            messages=Some(component_messages.clone())
                        />
                    }
                }}
                {move || {
                    let assets = asset_snapshot.get();
                    let selected_component =
                        editor.with(|editor| editor.selection().map(ToString::to_string));
                    view! {
                        <FlyAssets
                            assets=assets
                            selected_component=selected_component
                            on_upsert=on_upsert_asset
                            on_remove=on_remove_asset
                            on_apply=on_apply_asset
                            capabilities=capabilities
                            messages=Some(asset_messages.clone())
                        />
                    }
                }}
            </div>
        </section>
    }
}
