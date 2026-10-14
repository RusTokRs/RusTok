//! A working Dioxus editor surface on the shared Fly contracts — the parallel twin of
//! the `fly-leptos` editor. Semantics come from `fly`/`fly-ui`; this module only binds
//! them to Dioxus signals and DOM, and stays renderer-agnostic (SSR, web, or desktop).

use dioxus::prelude::*;
use fly::{
    render_page, validate_token_name, validate_token_value, DesignToken, EditorCommand, FlyEditor,
    PageSelection, ProjectDocument, RegistrySet, RenderPolicy, ThemeCommand,
};
use fly_browser::BrowserIntentEnvelope;
use fly_ui::{
    editor_layer_items, palette_block_items_with_access, theme_token_items,
    ContributionAssemblyResult, PaletteBlockAccess,
};

/// Full editor workspace: palette, layer tree, canvas preview, design-token editor, and undo/redo, wired to a
/// `FlyEditor` held in a signal. `on_change` receives the document after every accepted
/// command; persistence and transport belong to the host. Panel labels come from
/// `messages`: the host resolves a `fly_ui::EditorMessages` from its own message
/// catalog for the current locale (English by default), keeping the adapter
/// platform-neutral. When the host passes
/// `on_intent`, the workspace runs in SSR intent mode: palette inserts, selection,
/// undo/redo, and design-token edits are emitted as typed `BrowserIntentEnvelope`s
/// and nothing mutates locally (the host re-renders with the server-returned
/// document).
#[component]
pub fn FlyEditorWorkspace(
    document: ProjectDocument,
    contribution_assembly: Option<ContributionAssemblyResult>,
    instance_id: Option<String>,
    on_intent: Option<EventHandler<BrowserIntentEnvelope>>,
    on_change: Option<EventHandler<ProjectDocument>>,
    messages: Option<fly_ui::EditorMessages>,
) -> Element {
    let messages = messages.unwrap_or_default();
    let workspace_title = messages.workspace_title;
    let toolbar_label = messages.toolbar;
    let undo_label = messages.undo;
    let redo_label = messages.redo;
    let palette_label = messages.palette;
    let layers_label = messages.layers;
    let tokens_label = messages.theme_tokens;
    let remove_label = messages.remove;
    let token_name_label = messages.token_name;
    let token_value_label = messages.token_value;
    let save_token_label = messages.save_token;
    let preview_failed_label = messages.preview_failed;
    let intent_mode = on_intent.is_some();
    let intent_instance = instance_id.unwrap_or_else(|| "fly-workspace".to_string());
    let palette_access = PaletteBlockAccess::from_optional_assembly(contribution_assembly.as_ref());
    let blocks: Vec<(fly_ui::PaletteBlockItem, String)> =
        palette_block_items_with_access(&RegistrySet::with_builtins(), &palette_access)
            .into_iter()
            .map(|item| {
                let contribution_ids = item.contribution_ids.join(",");
                (item, contribution_ids)
            })
            .collect();
    let mut editor = use_signal(|| FlyEditor::new(document, RegistrySet::with_builtins()));
    let mut token_name = use_signal(String::new);
    let mut token_value = use_signal(String::new);
    let mut token_error = use_signal(|| Option::<String>::None);

    let rendered = render_page(
        editor.read().document(),
        &PageSelection::First,
        &RenderPolicy::default(),
    );
    let layers = editor_layer_items(editor.read().document());
    let tokens = theme_token_items(editor.read().document());
    let selected = editor.read().selection().map(ToString::to_string);
    let token_error_message = token_error.read().clone();
    let current_document = editor.read().document().clone();

    rsx! {
        section {
            class: "fly-editor fly-editor--full",
            role: "application",
            aria_label: "{workspace_title}",
            div { class: "fly-editor__toolbar", role: "toolbar", aria_label: "{toolbar_label}",
                button {
                    r#type: "button",
                    onclick: {
                        let instance = intent_instance.clone();
                        move |_| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::undo(&instance));
                                return;
                            }
                            let _ = editor.write().undo();
                            if let Some(handler) = on_change {
                                handler.call(editor.read().document().clone());
                            }
                        }
                    },
                    "{undo_label}"
                }
                button {
                    r#type: "button",
                    onclick: {
                        let instance = intent_instance.clone();
                        move |_| {
                            if let Some(handler) = on_intent {
                                handler.call(BrowserIntentEnvelope::redo(&instance));
                                return;
                            }
                            let _ = editor.write().redo();
                            if let Some(handler) = on_change {
                                handler.call(editor.read().document().clone());
                            }
                        }
                    },
                    "{redo_label}"
                }
            }
            div { class: "fly-editor__body",
                div { class: "fly-palette", role: "list", aria_label: "{palette_label}",
                    for (block, contribution_ids) in blocks.iter() {
                        button {
                            key: "{block.id}",
                            r#type: "button",
                            class: "fly-palette__block",
                            "data-fly-block": "{block.id}",
                            "data-fly-category": "{block.category}",
                            "data-fly-contribution-ids": "{contribution_ids}",
                            onclick: {
                                let block_id = block.id.clone();
                                let access = palette_access.clone();
                                let instance = intent_instance.clone();
                                move |_| {
                                    if !access.allows(&block_id) {
                                        return;
                                    }
                                    if let Some(handler) = on_intent {
                                        handler.call(BrowserIntentEnvelope::insert_block(
                                            &instance, &block_id,
                                        ));
                                        return;
                                    }
                                    let template = editor
                                        .read()
                                        .registries()
                                        .blocks
                                        .get(block_id.as_str())
                                        .map(|definition| definition.component.clone());
                                    if let Some(component) = template {
                                        let _ = editor.write().apply(fly::EditorCommand::Insert {
                                            parent_id: None,
                                            index: usize::MAX,
                                            component,
                                        });
                                        if let Some(handler) = on_change {
                                            handler.call(editor.read().document().clone());
                                        }
                                    }
                                }
                            },
                            "{block.label}"
                        }
                    }
                }
                div { class: "fly-theme-token-editor",
                    dl { class: "fly-theme-tokens", aria_label: "{tokens_label}",
                        for token in tokens.iter() {
                            div { key: "{token.name}", class: "fly-theme-tokens__row",
                                dt { code { "{token.css_variable}" } }
                                dd { "{token.value}" }
                                dd {
                                    button {
                                        r#type: "button",
                                        class: "fly-theme-tokens__remove",
                                        onclick: {
                                            let name = token.name.clone();
                                            let instance = intent_instance.clone();
                                            move |_| {
                                                if let Some(handler) = on_intent {
                                                    handler.call(BrowserIntentEnvelope::remove_theme_token(
                                                        &instance, &name,
                                                    ));
                                                    return;
                                                }
                                                let _ = editor.write().apply(EditorCommand::Theme {
                                                    command: ThemeCommand::Remove { name: name.clone() },
                                                });
                                                if let Some(handler) = on_change {
                                                    handler.call(editor.read().document().clone());
                                                }
                                            }
                                        },
                                        "{remove_label}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "fly-theme-token-editor__form",
                        label {
                            "{token_name_label}"
                            input {
                                r#type: "text",
                                placeholder: "brand",
                                value: "{token_name}",
                                oninput: move |event| token_name.set(event.value()),
                            }
                        }
                        label {
                            "{token_value_label}"
                            input {
                                r#type: "text",
                                placeholder: "#0b5fff",
                                value: "{token_value}",
                                oninput: move |event| token_value.set(event.value()),
                            }
                        }
                        button {
                            r#type: "button",
                            onclick: {
                                let instance = intent_instance.clone();
                                move |_| {
                                    let name = token_name.read().trim().to_string();
                                    let value = token_value.read().trim().to_string();
                                    match validate_token_name(&name)
                                        .and_then(|_| validate_token_value(&name, &value))
                                    {
                                        Ok(()) => {
                                            token_error.set(None);
                                            if let Some(handler) = on_intent {
                                                handler.call(BrowserIntentEnvelope::set_theme_token(
                                                    &instance, &name, &value,
                                                ));
                                                return;
                                            }
                                            let _ = editor.write().apply(EditorCommand::Theme {
                                                command: ThemeCommand::Upsert {
                                                    token: DesignToken { name, value },
                                                },
                                            });
                                            if let Some(handler) = on_change {
                                                handler.call(editor.read().document().clone());
                                            }
                                        }
                                        Err(message) => token_error.set(Some(message)),
                                    }
                                }
                            },
                            "{save_token_label}"
                        }
                        match &token_error_message {
                            Some(message) => rsx! {
                                p { class: "fly-theme-token-editor__error", role: "alert", "{message}" }
                            },
                            None => rsx! {},
                        }
                    }
                }
                ol { class: "fly-layers", aria_label: "{layers_label}",
                    for item in layers.iter() {
                        li { key: "{item.id}-{item.depth}",
                            button {
                                r#type: "button",
                                class: "fly-layers__row",
                                style: format!("padding-left:{}rem", 0.5 + item.depth as f32 * 1.25),
                                disabled: item.opaque || item.id.is_empty(),
                                onclick: {
                                    let item_id = item.id.clone();
                                    let item_selected = selected.clone();
                                    let instance = intent_instance.clone();
                                    move |_| {
                                        let next = if item_selected.as_deref() == Some(item_id.as_str()) {
                                            None
                                        } else {
                                            Some(item_id.clone())
                                        };
                                        if let Some(handler) = on_intent {
                                            handler.call(BrowserIntentEnvelope::select(
                                                &instance,
                                                next.as_deref(),
                                            ));
                                            return;
                                        }
                                        let _ = editor.write().apply(fly::EditorCommand::Select {
                                            component_id: next,
                                        });
                                    }
                                },
                                "{item.label}"
                            }
                        }
                    }
                }
                div { class: "fly-canvas", "data-fly-canvas": "true",
                    match &rendered {
                        Ok(page) => rsx! {
                            style { "{page.css}" }
                            div { dangerous_inner_html: "{page.html}" }
                        },
                        Err(error) => rsx! {
                            div { class: "fly-canvas fly-canvas--error", role: "alert",
                                "{preview_failed_label}: {error}"
                            }
                        },
                    }
                }
            }
            // Keep the signal document reachable for hosts that read it back through props.
            span { hidden: true, "data-fly-components": "{current_document.project.pages.len()}" }
        }
    }
}
