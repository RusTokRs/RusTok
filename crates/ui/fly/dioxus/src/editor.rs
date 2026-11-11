//! A working Dioxus editor surface on the shared Fly contracts — the parallel twin of
//! the `fly-leptos` editor. Semantics come from `fly`/`fly-ui`; this module only binds
//! them to Dioxus signals and DOM, and stays renderer-agnostic (SSR, web, or desktop).

use dioxus::prelude::*;
use fly::{
    DesignToken, EditorCommand, FlyEditor, PageSelection, ProjectDocument, RegistrySet,
    RenderPolicy, ThemeCommand, render_page, validate_token_name, validate_token_value,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, EditorMessages, PaletteBlockAccess,
    PaletteBlockItem, ThemeTokenItem, editor_layer_items, palette_block_items_with_access,
    theme_token_items,
};

/// Summary of a contribution assembly: how many contributions registered, how
/// many were skipped, and every assembly diagnostic with an accessible role
/// matching its severity. Rendered above the palette so provider problems are
/// visible where authors look for their blocks.
#[component]
pub fn FlyContributionSummary(
    assembly: ContributionAssemblyResult,
    messages: Option<EditorMessages>,
) -> Element {
    let template = messages.unwrap_or_default().contributions_summary;
    let headline = template
        .replace(
            "{registered}",
            &assembly.registered_contributions.to_string(),
        )
        .replace("{skipped}", &assembly.skipped_contributions.to_string());
    let registered = assembly.registered_contributions;
    let skipped = assembly.skipped_contributions;

    rsx! {
        div {
            class: "fly-contributions",
            "data-fly-contribution-registry": "true",
            "data-fly-contributions-registered": "{registered}",
            "data-fly-contributions-skipped": "{skipped}",
            p { class: "fly-contributions__headline", "{headline}" }
            for diagnostic in assembly.diagnostics.iter() {
                {
                    let role = match diagnostic.severity {
                        fly_ui::ContributionAssemblySeverity::Error => "alert",
                        _ => "status",
                    };
                    let code = diagnostic.code.clone();
                    rsx! {
                        p {
                            class: "fly-contributions__diagnostic",
                            role: "{role}",
                            "data-fly-contribution-diagnostic": "{code}",
                            code { "{code}" }
                            ": {diagnostic.message}"
                        }
                    }
                }
            }
        }
    }
}

/// Palette of insertable blocks, grouped by category in collapsible sections
/// (`landing` and `commerce` start open). Activating the insert control notifies the host
/// through `on_insert` with the registry block id.
///
/// The markup is the one `fly-leptos` renders, so the `fly-browser` SSR control contract,
/// shared styles and browser evidence runners see one palette whichever adapter produced it:
/// an item with `data-fly-block-id`, `data-fly-can-insert` and `data-fly-can-drag`, an insert
/// control with `data-fly-action="insert-block"`, and a drag control with
/// `data-fly-action="begin-block-drag"`. The drag control renders only when the host passes
/// `on_begin_drag`: a surface with no drag transport must not show a control that does nothing.
#[component]
pub fn FlyPalette(
    blocks: Vec<PaletteBlockItem>,
    on_insert: EventHandler<String>,
    on_begin_drag: Option<EventHandler<String>>,
    capabilities: Option<CapabilityState>,
    messages: Option<EditorMessages>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let palette_label = messages.palette;
    let insert_label = messages.insert_block;
    let drag_label = messages.drag_block;
    let can_insert = capabilities.map(|s| s.edit).unwrap_or(true);
    let can_drag = capabilities.map(|s| s.drag_drop).unwrap_or(true);
    let mut groups = std::collections::BTreeMap::<String, Vec<PaletteBlockItem>>::new();
    for block in blocks {
        groups
            .entry(block.category.clone())
            .or_default()
            .push(block);
    }

    rsx! {
        div { class: "fly-palette", role: "group", aria_label: "{palette_label}",
            for (category, items) in groups.into_iter() {
                details {
                    key: "{category}",
                    class: "fly-palette__category",
                    open: category == "landing" || category == "commerce",
                    summary { class: "fly-palette__category-title", "{category}" }
                    div { class: "fly-palette__grid", role: "list",
                        for item in items.into_iter() {
                            {
                                let insert_id = item.id.clone();
                                let drag_id = item.id.clone();
                                let contribution_ids = item.contribution_ids.join(",");
                                let insert_name = format!("{insert_label}: {}", item.label);
                                let drag_name = format!("{drag_label}: {}", item.label);
                                rsx! {
                                    div {
                                        key: "{item.id}",
                                        class: "fly-palette__item",
                                        role: "listitem",
                                        "data-fly-block-id": "{item.id}",
                                        "data-fly-category": "{item.category}",
                                        "data-fly-contribution-ids": "{contribution_ids}",
                                        "data-fly-can-insert": "{can_insert}",
                                        "data-fly-can-drag": "{can_drag}",
                                        button {
                                            r#type: "button",
                                            class: "fly-palette__block",
                                            aria_label: "{insert_name}",
                                            "data-fly-action": "insert-block",
                                            disabled: !can_insert,
                                            onclick: move |_| on_insert.call(insert_id.clone()),
                                            "{item.label}"
                                        }
                                        if let Some(on_begin_drag) = on_begin_drag {
                                            button {
                                                r#type: "button",
                                                class: "fly-palette__drag",
                                                aria_label: "{drag_name}",
                                                "data-fly-action": "begin-block-drag",
                                                disabled: !can_drag,
                                                onclick: move |_| {
                                                    on_begin_drag.call(drag_id.clone())
                                                },
                                                "{drag_label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Document layer tree hierarchy. Selecting an item invokes `on_select`.
///
/// A selectable row carries `data-fly-component-id` and `data-fly-action="select-component"`,
/// the same markup `fly-leptos` renders; rows that cannot be selected carry neither.
#[component]
pub fn FlyLayerTree(
    layers: Vec<fly_ui::EditorLayerItem>,
    selected: Option<String>,
    on_select: EventHandler<String>,
    messages: Option<EditorMessages>,
) -> Element {
    let layers_label = messages.unwrap_or_default().layers;

    rsx! {
        ol { class: "fly-layers", aria_label: "{layers_label}",
            for item in layers.into_iter() {
                {
                    let item_id = item.id.clone();
                    let selectable = !item.opaque && !item.id.is_empty();
                    let browser_component_id = selectable.then(|| item.id.clone());
                    let browser_action = selectable.then_some("select-component");
                    let is_selected = selected.as_deref() == Some(&item_id);
                    let class_name = if is_selected {
                        "fly-layers__row fly-layers__row--selected"
                    } else {
                        "fly-layers__row"
                    };
                    rsx! {
                        li {
                            key: "{item.id}-{item.depth}",
                            button {
                                r#type: "button",
                                class: "{class_name}",
                                style: format!("padding-left:{}rem", 0.5 + item.depth as f32 * 1.25),
                                "data-fly-component-id": browser_component_id,
                                "data-fly-action": browser_action,
                                disabled: !selectable,
                                onclick: move |_| on_select.call(item_id.clone()),
                                "{item.label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Design-token listing for adapter theme panels.
#[component]
pub fn FlyThemeTokens(
    tokens: Vec<ThemeTokenItem>,
    on_remove: Option<EventHandler<String>>,
    capabilities: Option<CapabilityState>,
    messages: Option<EditorMessages>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let tokens_label = messages.theme_tokens;
    let remove_label = messages.remove;
    let can_edit_styles = capabilities.map(|s| s.styles).unwrap_or(true);

    rsx! {
        dl { class: "fly-theme-tokens", aria_label: "{tokens_label}",
            for token in tokens.into_iter() {
                {
                    let name = token.name.clone();
                    rsx! {
                        div { key: "{token.name}", class: "fly-theme-tokens__row",
                            dt { code { "{token.css_variable}" } }
                            dd { "{token.value}" }
                            if let Some(on_remove_handler) = on_remove {
                                dd {
                                    button {
                                        r#type: "button",
                                        class: "fly-theme-tokens__remove",
                                        disabled: !can_edit_styles,
                                        onclick: move |_| on_remove_handler.call(name.clone()),
                                        "{remove_label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Token editor: lists existing tokens and provides an authoring form for upsert/remove.
#[component]
pub fn FlyThemeTokenEditor(
    tokens: Vec<ThemeTokenItem>,
    on_upsert: EventHandler<(String, String)>,
    on_remove: EventHandler<String>,
    capabilities: Option<CapabilityState>,
    messages: Option<EditorMessages>,
    on_error: Option<EventHandler<String>>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let token_name_label = messages.token_name.clone();
    let token_value_label = messages.token_value.clone();
    let save_token_label = messages.save_token.clone();
    let can_edit_styles = capabilities.map(|s| s.styles).unwrap_or(true);

    let mut token_name = use_signal(String::new);
    let mut token_value = use_signal(String::new);
    let mut token_error = use_signal(|| Option::<String>::None);
    let token_error_message = token_error.read().clone();

    rsx! {
        div { class: "fly-theme-token-editor",
            FlyThemeTokens {
                tokens,
                on_remove: Some(on_remove),
                capabilities,
                messages: Some(messages),
            }
            div { class: "fly-theme-token-editor__form",
                label {
                    "{token_name_label}"
                    input {
                        r#type: "text",
                        placeholder: "brand",
                        value: "{token_name}",
                        disabled: !can_edit_styles,
                        oninput: move |event| token_name.set(event.value()),
                    }
                }
                label {
                    "{token_value_label}"
                    input {
                        r#type: "text",
                        placeholder: "#0b5fff",
                        value: "{token_value}",
                        disabled: !can_edit_styles,
                        oninput: move |event| token_value.set(event.value()),
                    }
                }
                button {
                    r#type: "button",
                    disabled: !can_edit_styles,
                    onclick: move |_| {
                        let name = token_name.read().trim().to_string();
                        let value = token_value.read().trim().to_string();
                        match validate_token_name(&name)
                            .and_then(|_| validate_token_value(&name, &value))
                        {
                            Ok(()) => {
                                token_error.set(None);
                                on_upsert.call((name, value));
                                token_name.set(String::new());
                                token_value.set(String::new());
                            }
                            Err(message) => {
                                token_error.set(Some(message.clone()));
                                if let Some(handler) = on_error {
                                    handler.call(message);
                                }
                            }
                        }
                    },
                    "{save_token_label}"
                }
                if let Some(message) = &token_error_message {
                    p { class: "fly-theme-token-editor__error", role: "alert", "{message}" }
                }
            }
        }
    }
}

/// Canvas preview container rendering SSR-generated markup and stylesheet.
#[component]
pub fn FlyCanvasPreview(
    rendered: Result<fly::RenderedPage, fly::FlyError>,
    preview_failed_label: Option<String>,
) -> Element {
    let failed_label = preview_failed_label.unwrap_or_else(|| "Preview failed".to_string());

    rsx! {
        div { class: "fly-canvas", "data-fly-canvas": "true",
            match &rendered {
                Ok(page) => rsx! {
                    style { "{page.css}" }
                    div { dangerous_inner_html: "{page.html}" }
                },
                Err(error) => rsx! {
                    div { class: "fly-canvas fly-canvas--error", role: "alert",
                        "{failed_label}: {error}"
                    }
                },
            }
        }
    }
}

/// Full editor workspace: palette, layer tree, canvas preview, design-token editor, and undo/redo, wired to a
/// `FlyEditor` held in a signal.
///
/// Local mutations report errors through `on_error` rather than swallowing them.
/// In SSR intent mode (`on_intent`), commands are emitted as typed [`BrowserIntentEnvelope`]s.
#[component]
pub fn FlyEditorWorkspace(
    document: ProjectDocument,
    registries: Option<RegistrySet>,
    contribution_assembly: Option<ContributionAssemblyResult>,
    instance_id: Option<String>,
    on_intent: Option<EventHandler<BrowserIntentEnvelope>>,
    on_change: Option<EventHandler<ProjectDocument>>,
    on_error: Option<EventHandler<String>>,
    messages: Option<EditorMessages>,
    initial_page: Option<usize>,
    capabilities: Option<CapabilityState>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let workspace_title = messages.workspace_title.clone();
    let toolbar_label = messages.toolbar.clone();
    let undo_label = messages.undo.clone();
    let redo_label = messages.redo.clone();
    let preview_failed_label = messages.preview_failed.clone();

    let intent_mode = on_intent.is_some();
    let intent_instance = instance_id.unwrap_or_else(|| "fly-workspace".to_string());
    let registries = registries.unwrap_or_else(RegistrySet::with_builtins);
    let summary_assembly = contribution_assembly.clone();
    let palette_access = PaletteBlockAccess::from_optional_assembly(contribution_assembly.as_ref());
    let blocks = palette_block_items_with_access(&registries, &palette_access);

    let mut editor = use_signal(|| FlyEditor::new(document, registries));

    // Intent mode only: the drag is completed by a `drop` intent, which needs the host's
    // intent transport and canvas geometry. A local-document workspace has neither.
    let begin_palette_drag = use_callback({
        let instance = intent_instance.clone();
        let access = palette_access.clone();
        move |block_id: String| {
            if !access.allows(&block_id) {
                return;
            }
            if let Some(handler) = on_intent {
                handler.call(BrowserIntentEnvelope::begin_palette_drag(
                    &instance, &block_id,
                ));
            }
        }
    });
    let on_begin_drag = intent_mode.then_some(begin_palette_drag);

    let page_selection = match initial_page {
        Some(index) => PageSelection::Index(index),
        None => PageSelection::First,
    };
    let rendered = render_page(
        editor.read().document(),
        &page_selection,
        &RenderPolicy::default(),
    );
    let layers = editor_layer_items(editor.read().document());
    let tokens = theme_token_items(editor.read().document());
    let selected = editor.read().selection().map(ToString::to_string);
    let current_document = editor.read().document().clone();
    let can_undo = intent_mode || editor.read().history().can_undo();
    let can_redo = intent_mode || editor.read().history().can_redo();

    rsx! {
        section {
            class: "fly-editor fly-editor--full",
            role: "application",
            aria_label: "{workspace_title}",
            div { class: "fly-editor__toolbar", role: "toolbar", aria_label: "{toolbar_label}",
                button {
                    r#type: "button",
                    disabled: !can_undo,
                    onclick: {
                        let instance = intent_instance.clone();
                        move |_| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::undo(&instance));
                                return;
                            }
                            let result = {
                                let mut ed = editor.write();
                                ed.undo().cloned().map_err(|e| e.to_string())
                            };
                            match result {
                                Ok(doc) => {
                                    if let Some(handler) = on_change {
                                        handler.call(doc);
                                    }
                                }
                                Err(err) => {
                                    if let Some(handler) = on_error {
                                        handler.call(err);
                                    }
                                }
                            }
                        }
                    },
                    "{undo_label}"
                }
                button {
                    r#type: "button",
                    disabled: !can_redo,
                    onclick: {
                        let instance = intent_instance.clone();
                        move |_| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::redo(&instance));
                                return;
                            }
                            let result = {
                                let mut ed = editor.write();
                                ed.redo().cloned().map_err(|e| e.to_string())
                            };
                            match result {
                                Ok(doc) => {
                                    if let Some(handler) = on_change {
                                        handler.call(doc);
                                    }
                                }
                                Err(err) => {
                                    if let Some(handler) = on_error {
                                        handler.call(err);
                                    }
                                }
                            }
                        }
                    },
                    "{redo_label}"
                }
            }
            div { class: "fly-editor__body",
                if let Some(assembly) = summary_assembly {
                    FlyContributionSummary {
                        assembly,
                        messages: Some(messages.clone()),
                    }
                }
                FlyPalette {
                    blocks,
                    on_insert: {
                        let instance = intent_instance.clone();
                        let access = palette_access.clone();
                        move |block_id: String| {
                            if !access.allows(&block_id) {
                                return;
                            }
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::insert_block(&instance, &block_id));
                                return;
                            }
                            let template = editor
                                .read()
                                .registries()
                                .blocks
                                .get(block_id.as_str())
                                .map(|definition| definition.component.clone());
                            if let Some(component) = template {
                                let result = {
                                    let mut ed = editor.write();
                                    ed.apply(fly::EditorCommand::Insert {
                                        parent_id: None,
                                        index: usize::MAX,
                                        component,
                                    })
                                    .map(|_| ed.document().clone())
                                    .map_err(|e| e.to_string())
                                };
                                match result {
                                    Ok(doc) => {
                                        if let Some(handler) = on_change {
                                            handler.call(doc);
                                        }
                                    }
                                    Err(err) => {
                                        if let Some(handler) = on_error {
                                            handler.call(err);
                                        }
                                    }
                                }
                            }
                        }
                    },
                    on_begin_drag,
                    capabilities,
                    messages: Some(messages.clone()),
                }
                FlyThemeTokenEditor {
                    tokens,
                    on_upsert: {
                        let instance = intent_instance.clone();
                        move |(name, value): (String, String)| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::set_theme_token(&instance, &name, &value));
                                return;
                            }
                            let result = {
                                let mut ed = editor.write();
                                ed.apply(EditorCommand::Theme {
                                    command: ThemeCommand::Upsert {
                                        token: DesignToken { name, value },
                                    },
                                })
                                .map(|_| ed.document().clone())
                                .map_err(|e| e.to_string())
                            };
                            match result {
                                Ok(doc) => {
                                    if let Some(handler) = on_change {
                                        handler.call(doc);
                                    }
                                }
                                Err(err) => {
                                    if let Some(handler) = on_error {
                                        handler.call(err);
                                    }
                                }
                            }
                        }
                    },
                    on_remove: {
                        let instance = intent_instance.clone();
                        move |name: String| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::remove_theme_token(&instance, &name));
                                return;
                            }
                            let result = {
                                let mut ed = editor.write();
                                ed.apply(EditorCommand::Theme {
                                    command: ThemeCommand::Remove { name },
                                })
                                .map(|_| ed.document().clone())
                                .map_err(|e| e.to_string())
                            };
                            match result {
                                Ok(doc) => {
                                    if let Some(handler) = on_change {
                                        handler.call(doc);
                                    }
                                }
                                Err(err) => {
                                    if let Some(handler) = on_error {
                                        handler.call(err);
                                    }
                                }
                            }
                        }
                    },
                    capabilities,
                    messages: Some(messages.clone()),
                    on_error,
                }
                FlyLayerTree {
                    layers,
                    selected: selected.clone(),
                    on_select: {
                        let instance = intent_instance.clone();
                        let current_selected = selected.clone();
                        move |item_id: String| {
                            let next = if current_selected.as_deref() == Some(item_id.as_str()) {
                                None
                            } else {
                                Some(item_id)
                            };
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::select(&instance, next.as_deref()));
                                return;
                            }
                            let result = {
                                let mut ed = editor.write();
                                ed.apply(fly::EditorCommand::Select {
                                    component_id: next,
                                })
                                .map_err(|e| e.to_string())
                            };
                            if let Err(err) = result
                                && let Some(handler) = on_error
                            {
                                handler.call(err);
                            }
                        }
                    },
                    messages: Some(messages),
                }
                FlyCanvasPreview {
                    rendered,
                    preview_failed_label: Some(preview_failed_label),
                }
            }
            span { hidden: true, "data-fly-components": "{current_document.project.pages.len()}" }
        }
    }
}
