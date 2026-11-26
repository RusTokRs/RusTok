//! A working Dioxus editor surface on the shared Fly contracts — the parallel twin of
//! the `fly-leptos` editor. Semantics come from `fly`/`fly-ui`; this module only binds
//! them to Dioxus signals and DOM, and stays renderer-agnostic (SSR, web, or desktop).

use dioxus::prelude::*;
use fly::{
    DesignToken, EditorCommand, FlyEditor, PageSelection, ProjectDocument, RegistrySet,
    RenderPolicy, ThemeCommand, render_page, validate_token_name, validate_token_value,
};
use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, EditorMessages, PaletteBlockAccess,
    PaletteBlockItem, ThemeTokenItem, editor_layer_items_for_page, palette_block_items_with_access,
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
/// control with `data-fly-action="insert-block"`, and a native HTML5 drag source with
/// `data-fly-action="begin-block-drag"`. The drag control renders only when the host passes
/// both `on_begin_drag` and `on_cancel_drag`, so cancelled drags cannot leave stale editor state.
#[component]
pub fn FlyPalette(
    blocks: Vec<PaletteBlockItem>,
    on_insert: EventHandler<String>,
    on_begin_drag: Option<EventHandler<String>>,
    #[props(default)]
    on_cancel_drag: Option<EventHandler<()>>,
    #[props(default)]
    on_error: Option<EventHandler<String>>,
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
                                let drag_enabled = can_drag
                                    && on_begin_drag.is_some()
                                    && on_cancel_drag.is_some();
                                rsx! {
                                    div {
                                        key: "{item.id}",
                                        class: "fly-palette__item",
                                        role: "listitem",
                                        "data-fly-block-id": "{item.id}",
                                        "data-fly-category": "{item.category}",
                                        "data-fly-contribution-ids": "{contribution_ids}",
                                        "data-fly-can-insert": "{can_insert}",
                                        "data-fly-can-drag": "{drag_enabled}",
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
                                                draggable: drag_enabled,
                                                disabled: !drag_enabled,
                                                ondragstart: move |event| {
                                                    if !drag_enabled {
                                                        event.prevent_default();
                                                        return;
                                                    }
                                                    let transfer = event.data.data_transfer();
                                                    if let Err(error) = transfer.set_data("text/plain", &drag_id) {
                                                        event.prevent_default();
                                                        if let Some(handler) = on_error {
                                                            handler.call(format!("Could not start block drag: {error:?}"));
                                                        }
                                                        return;
                                                    }
                                                    transfer.set_effect_allowed("move");
                                                    if let Some(handler) = on_begin_drag {
                                                        handler.call(drag_id.clone());
                                                    }
                                                },
                                                ondragend: move |event| {
                                                    if event.data.data_transfer().drop_effect() == "none" {
                                                        if let Some(handler) = on_cancel_drag {
                                                            handler.call(());
                                                        }
                                                    }
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
    #[props(default)]
    on_begin_move: Option<EventHandler<String>>,
    #[props(default)]
    on_cancel_drag: Option<EventHandler<()>>,
    #[props(default)]
    on_error: Option<EventHandler<String>>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let layers_label = messages.layers.clone();
    let move_label = messages.drag_block.clone();

    rsx! {
        ol { class: "fly-layers", aria_label: "{layers_label}",
            for item in layers.into_iter() {
                {
                    let item_id = item.id.clone();
                    let move_id = item.id.clone();
                    let selectable = !item.opaque && !item.id.is_empty();
                    let can_move = selectable && item.depth > 0 && !item.is_locked;
                    let move_enabled = can_move
                        && on_begin_move.is_some()
                        && on_cancel_drag.is_some();
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
                            if let Some(on_begin_move) = on_begin_move {
                                button {
                                    r#type: "button",
                                    class: "fly-layers__move",
                                    aria_label: format!("{} {}", move_label, move_id),
                                    "data-fly-action": "begin-component-move",
                                    "data-fly-component-id": "{move_id}",
                                    draggable: move_enabled,
                                    disabled: !move_enabled,
                                    ondragstart: move |event| {
                                        if !move_enabled {
                                            event.prevent_default();
                                            return;
                                        }
                                        let transfer = event.data.data_transfer();
                                        if let Err(error) = transfer.set_data("text/plain", &move_id) {
                                            event.prevent_default();
                                            if let Some(handler) = on_error {
                                                handler.call(format!("Could not start component move: {error:?}"));
                                            }
                                            return;
                                        }
                                        transfer.set_effect_allowed("move");
                                        if let Some(handler) = on_begin_move {
                                            handler.call(move_id.clone());
                                        }
                                    },
                                    ondragend: move |event| {
                                        if event.data.data_transfer().drop_effect() == "none" {
                                            if let Some(handler) = on_cancel_drag {
                                                handler.call(());
                                            }
                                        }
                                    },
                                    "{move_label}"
                                }
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

fn token_submission_is_visible(tokens: &[ThemeTokenItem], name: &str, value: &str) -> bool {
    tokens
        .iter()
        .any(|token| token.name == name && token.value == value)
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
    let mut pending_submission = use_signal(|| Option::<(String, String)>::None);
    let token_error_message = token_error.read().clone();
    let observed_tokens = tokens.clone();
    let effect_tokens = tokens.clone();
    use_effect(use_reactive!(|effect_tokens| {
        let Some((name, value)) = pending_submission.peek().clone() else {
            return;
        };
        if token_submission_is_visible(&effect_tokens, &name, &value) {
            token_name.set(String::new());
            token_value.set(String::new());
            pending_submission.set(None);
        }
    }));

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
                        oninput: move |event| {
                            pending_submission.set(None);
                            token_name.set(event.value());
                        },
                    }
                }
                label {
                    "{token_value_label}"
                    input {
                        r#type: "text",
                        placeholder: "#0b5fff",
                        value: "{token_value}",
                        disabled: !can_edit_styles,
                        oninput: move |event| {
                            pending_submission.set(None);
                            token_value.set(event.value());
                        },
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
                                if token_submission_is_visible(&observed_tokens, &name, &value) {
                                    token_name.set(String::new());
                                    token_value.set(String::new());
                                    pending_submission.set(None);
                                } else {
                                    pending_submission.set(Some((name.clone(), value.clone())));
                                    on_upsert.call((name, value));
                                }
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

pub(crate) fn decode_fly_canvas_envelope(
    payload: &str,
    expected_instance: &str,
    last_sequence: Option<u64>,
) -> Option<(u64, String)> {
    let envelope = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    if envelope.get("protocol")?.as_str()? != fly_web::FLY_IFRAME_PROTOCOL
        || envelope.get("instance_id")?.as_str()? != expected_instance
    {
        return None;
    }
    let sequence = envelope.get("sequence")?.as_u64()?;
    if last_sequence.is_some_and(|last| sequence <= last) {
        return None;
    }
    Some((sequence, payload.to_string()))
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
fn subscribe_canvas_iframe(
    iframe_id: &str,
    expected_instance: &str,
    expected_origin: &str,
    mut subscription: Signal<Option<fly_web::IframeJsonSubscription>>,
    mut message_port: Signal<Option<fly_web::IframeMessagePort>>,
    on_message: EventHandler<String>,
    on_error: Option<EventHandler<String>>,
) {
    use fly_web::{BrowserRuntimeError, IframeBridgeMessage};

    subscription.set(None);
    let instance = expected_instance.to_string();
    match fly_web::IframeJsonSubscription::subscribe_by_element_id(
        iframe_id,
        expected_origin,
        move |payload, last_sequence| {
            decode_fly_canvas_envelope(payload, &instance, last_sequence)
        },
        move |payload| on_message.call(payload),
    ) {
        Ok(next) => {
            subscription.set(Some(next));
            let mut port = message_port.write();
            if let Some(port) = port.as_mut()
                && let Err(error) =
                    port.post_by_element_id(iframe_id, IframeBridgeMessage::RequestSnapshot)
                && let Some(handler) = on_error
            {
                handler.call(error.to_string());
            }
        }
        Err(BrowserRuntimeError::MissingElement(_)
            | BrowserRuntimeError::MissingContentWindow) => {}
        Err(error) => {
            if let Some(handler) = on_error {
                handler.call(error.to_string());
            }
        }
    }
}

/// Sandboxed `srcdoc` canvas with a source-, origin- and sequence-checked message subscription.
 ///
 /// The host owns the document renderer and interprets the raw JSON messages. The iframe
 /// subscription is recreated after every load so a new document's sequence can start at one.
 #[component]
 pub fn FlyCanvasIframe(
    id: String,
    instance_id: String,
    srcdoc: String,
    expected_origin: String,
    on_message: EventHandler<String>,
    on_error: Option<EventHandler<String>>,
    on_load: Option<EventHandler<()>>,
    title: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    class: Option<String>,
) -> Element {
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    let mut subscription = use_signal(|| None::<fly_web::IframeJsonSubscription>);
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    let mut message_port = use_signal(|| None::<fly_web::IframeMessagePort>);
    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    let mut port_instance_config = use_signal(|| None::<(String, String)>);

    let iframe_id = id.clone();
    let expected_instance = instance_id.clone();
    let expected_origin_for_load = expected_origin.clone();
    let onload = {
        #[cfg(all(target_arch = "wasm32", feature = "web"))]
        {
            move |_| {
                // Every iframe navigation resets the child sequence. Replacing the subscription
                // resets the host sequence fence and the snapshot request repopulates geometry.
                subscribe_canvas_iframe(
                    &iframe_id,
                    &expected_instance,
                    &expected_origin_for_load,
                    subscription,
                    message_port,
                    on_message,
                    on_error,
                );
                if let Some(handler) = on_load {
                    handler.call(());
                }
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "web")))]
        {
            let _ = (&iframe_id, &expected_instance, &expected_origin_for_load);
            let _ = (on_message, on_error);
            move |_| {
                if let Some(handler) = on_load {
                    handler.call(());
                }
            }
        }
    };

    #[cfg(all(target_arch = "wasm32", feature = "web"))]
    {
        let port_config = (id.clone(), instance_id.clone(), expected_origin.clone());
        use_effect(use_reactive!(|port_config| {
            let (effect_iframe_id, effect_instance_id, effect_expected_origin) =
                port_config.clone();
            let existing_config = port_instance_config.read().clone();
            let config_matches = existing_config.as_ref().is_some_and(|(instance, origin)| {
                instance == &effect_instance_id && origin == &effect_expected_origin
            });
            let needs_new_port = !config_matches || message_port.read().is_none();
            if needs_new_port {
                // A new instance/origin gets a fresh outbound sequence. An ID-only change keeps
                // the current source-window subscription; if it replaced the iframe element,
                // that element's load handler will rebind it without resetting the port sequence.
                subscription.set(None);
                message_port.set(None);
                port_instance_config.set(None);
                match fly_web::IframeMessagePort::new(
                    effect_instance_id.clone(),
                    effect_expected_origin.clone(),
                ) {
                    Ok(port) => {
                        message_port.set(Some(port));
                        port_instance_config.set(Some((
                            effect_instance_id.clone(),
                            effect_expected_origin.clone(),
                        )));
                    }
                    Err(error) => {
                        if let Some(handler) = on_error {
                            handler.call(error.to_string());
                        }
                        return;
                    }
                }
                subscribe_canvas_iframe(
                    &effect_iframe_id,
                    &effect_instance_id,
                    &effect_expected_origin,
                    subscription,
                    message_port,
                    on_message,
                    on_error,
                );
            }
        }));
    }

    let title = title.unwrap_or_else(|| "Fly editor canvas".to_string());
    let width = width.unwrap_or(1280);
    let height = height.unwrap_or(800);
    let class = class.unwrap_or_else(|| "block border-0 bg-white".to_string());

    rsx! {
        iframe {
            id: "{id}",
            title: "{title}",
            sandbox: "allow-scripts",
            srcdoc: "{srcdoc}",
            width: "{width}",
            height: "{height}",
            class: "{class}",
            onload: onload,
        }
    }
}

/// Canvas preview container rendering SSR-generated markup and stylesheet.
///
/// The stylesheet is an inline `style` element, so a host that serves the editor under a nonce
/// policy drops it unless the element carries the nonce: the canvas then renders unstyled while
/// the published page looks right. Pass `csp_nonce` from the document's policy; a host without a
/// nonce policy passes nothing and renders as before.
#[component]
pub fn FlyCanvasPreview(
    rendered: Result<fly::RenderedPage, fly::FlyError>,
    preview_failed_label: Option<String>,
    csp_nonce: Option<String>,
) -> Element {
    let failed_label = preview_failed_label.unwrap_or_else(|| "Preview failed".to_string());
    let style_nonce = csp_nonce.filter(|nonce| !nonce.trim().is_empty());

    rsx! {
        div { class: "fly-canvas", "data-fly-canvas": "true",
            match &rendered {
                Ok(page) => rsx! {
                    style { nonce: style_nonce.clone(), "{page.css}" }
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

/// Choose the canonical rendering document for each workspace mode.
///
/// In intent mode the consumer controller owns the document. The local FlyEditor
/// signal was constructed only once and must never shadow newer document props.
/// In standalone mode that signal owns the mutable document and its undo history.
fn workspace_render_document<'a>(
    authoritative: Option<&'a ProjectDocument>,
    local: &'a FlyEditor,
) -> &'a ProjectDocument {
    authoritative.unwrap_or_else(|| local.document())
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
    /// Selection is owned by the host controller in intent mode.
    selected_component_id: Option<String>,
    /// The host owns history in intent mode. Local mode uses FlyEditor history.
    host_can_undo: Option<bool>,
    host_can_redo: Option<bool>,
    /// CSP nonce for the inline styles the workspace renders; see [`FlyCanvasPreview`].
    csp_nonce: Option<String>,
    /// Optional host-owned isolated canvas. When absent, the inline preview is rendered.
    #[props(default)]
    canvas: Option<Element>,
    /// Whether the supplied canvas can report pointer geometry and complete drops.
    /// Defaults to true only when a host canvas is present.
    #[props(default)]
    canvas_interactive: Option<bool>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let workspace_title = messages.workspace_title.clone();
    let toolbar_label = messages.toolbar.clone();
    let undo_label = messages.undo.clone();
    let redo_label = messages.redo.clone();
    let preview_failed_label = messages.preview_failed.clone();
    let workspace_csp_nonce = csp_nonce.filter(|nonce| !nonce.trim().is_empty());

    let intent_mode = on_intent.is_some();
    let canvas_interactive = canvas_interactive.unwrap_or(canvas.is_some());
    let intent_instance = instance_id.unwrap_or_else(|| "fly-workspace".to_string());
    let registries = registries.unwrap_or_else(RegistrySet::with_builtins);
    let summary_assembly = contribution_assembly.clone();
    let palette_access = PaletteBlockAccess::from_optional_assembly(contribution_assembly.as_ref());
    let blocks = palette_block_items_with_access(&registries, &palette_access);

    // Copy only when the consumer owns the state. The locally retained signal
    // is still needed for standalone editing but must not render stale data
    // while on_intent forwards all mutations to the host.
    let authoritative_document = intent_mode.then(|| document.clone());
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
    let on_begin_drag = (intent_mode && canvas_interactive).then_some(begin_palette_drag);
    let cancel_drag = use_callback({
        let instance = intent_instance.clone();
        move |_| {
            if let Some(handler) = on_intent {
                handler.call(BrowserIntentEnvelope::new(
                    &instance,
                    BrowserIntentKind::CancelDrag,
                    serde_json::json!({}),
                ));
            }
        }
    });
    let on_cancel_drag = (intent_mode && canvas_interactive).then_some(cancel_drag);
    let begin_selected_move = use_callback({
        let instance = intent_instance.clone();
        move |component_id: String| {
            if let Some(handler) = on_intent {
                handler.call(BrowserIntentEnvelope::new(
                    &instance,
                    BrowserIntentKind::BeginSelectedMove,
                    serde_json::json!({
                        "selected_component_id": component_id,
                    }),
                ));
            }
        }
    });
    let can_drag_components = capabilities.is_none_or(|profile| profile.drag_drop);
    let on_begin_move =
        (intent_mode && canvas_interactive && can_drag_components).then_some(begin_selected_move);

    let active_page_index = initial_page.unwrap_or(0);
    let page_selection = PageSelection::Index(active_page_index);
    let current_document = {
        let local = editor.read();
        workspace_render_document(authoritative_document.as_ref(), &local).clone()
    };
    let rendered = render_page(
        &current_document,
        &page_selection,
        &RenderPolicy::default(),
    );
    // The canvas previews one active page: the layer panel must not expose
    // components from other pages as if they belong to that canvas.
    let layers = editor_layer_items_for_page(&current_document, active_page_index);
    let tokens = theme_token_items(&current_document);
    let selected = if intent_mode {
        selected_component_id
    } else {
        editor.read().selection().map(ToString::to_string)
    };
    let history_allowed = capabilities.is_none_or(|profile| profile.history);
    let can_undo = history_allowed && if intent_mode {
        host_can_undo.unwrap_or(true)
    } else {
        editor.read().history().can_undo()
    };
    let can_redo = history_allowed && if intent_mode {
        host_can_redo.unwrap_or(true)
    } else {
        editor.read().history().can_redo()
    };

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
                    on_cancel_drag,
                    on_error,
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
                    on_begin_move,
                    on_cancel_drag,
                    on_error,
                }
                if let Some(canvas) = canvas {
                    {canvas}
                } else {
                    FlyCanvasPreview {
                        rendered,
                        preview_failed_label: Some(preview_failed_label),
                        csp_nonce: workspace_csp_nonce.clone(),
                    }
                }
            }
            span { hidden: true, "data-fly-components": "{current_document.project().pages.len()}" }
        }
    }
}


#[cfg(test)]
mod controlled_workspace_regression_tests {
    use super::*;
    use fly::GrapesJsCodec;
    use serde_json::json;

    fn document(content: &str) -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root", "type": "wrapper",
                    "components": [{
                        "id": "hero", "type": "text", "content": content
                    }]
                }
            }]
        })).expect("Fly document")
    }

    #[test]
    fn iframe_envelope_rejects_wrong_instance_protocol_and_replayed_sequences() {
        let payload = serde_json::json!({
            "protocol": fly_web::FLY_IFRAME_PROTOCOL,
            "instance_id": "canvas-a",
            "sequence": 7,
            "message": { "type": "ready" }
        })
        .to_string();

        assert_eq!(
            decode_fly_canvas_envelope(&payload, "canvas-a", Some(6)),
            Some((7, payload.clone()))
        );
        assert!(decode_fly_canvas_envelope(&payload, "canvas-a", Some(7)).is_none());
        assert!(decode_fly_canvas_envelope(&payload, "canvas-b", None).is_none());

        let wrong_protocol = payload.replace(fly_web::FLY_IFRAME_PROTOCOL, "fly/other");
        assert!(decode_fly_canvas_envelope(&wrong_protocol, "canvas-a", None).is_none());
    }

    #[test]
    fn layer_rows_are_scoped_to_the_rendered_page() {
        let project = GrapesJsCodec::decode_value(json!({
            "pages": [
                { "id": "home", "component": {
                    "id": "home-root", "type": "wrapper",
                    "components": [{ "id": "home-hero", "type": "text" }]
                }},
                { "id": "about", "component": {
                    "id": "about-root", "type": "wrapper",
                    "components": [{ "id": "about-hero", "type": "text" }]
                }}
            ]
        })).expect("two-page project");
        let home = editor_layer_items_for_page(&project, 0);
        let about = editor_layer_items_for_page(&project, 1);
        assert!(home.iter().any(|item| item.id == "home-hero"));
        assert!(!home.iter().any(|item| item.id == "about-hero"));
        assert!(about.iter().any(|item| item.id == "about-hero"));
        assert!(!about.iter().any(|item| item.id == "home-hero"));
        assert!(editor_layer_items_for_page(&project, 99).is_empty());
    }

    #[test]
    fn intent_mode_renders_new_host_document_without_reinitializing_local_history() {
        let first = document("before");
        let updated = document("after");
        let local = FlyEditor::new(first.clone(), RegistrySet::with_builtins());
        assert_eq!(workspace_render_document(None, &local), &first);
        assert_eq!(
            workspace_render_document(Some(&updated), &local),
            &updated,
            "incoming controller snapshot is authoritative while intents are forwarded"
        );
        assert_eq!(local.document(), &first, "standalone editor is not reset");
    }


    #[test]
    fn token_form_acknowledges_only_the_exact_rendered_value() {
        let tokens = vec![ThemeTokenItem {
            name: "brand".to_string(),
            value: "#123456".to_string(),
            css_variable: "--brand".to_string(),
        }];
        assert!(token_submission_is_visible(&tokens, "brand", "#123456"));
        assert!(!token_submission_is_visible(&tokens, "brand", "#654321"));
        assert!(!token_submission_is_visible(&tokens, "other", "#123456"));
    }


}
