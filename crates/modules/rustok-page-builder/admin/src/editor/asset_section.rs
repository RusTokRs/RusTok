use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{AssetCatalog, AssetCommand, EditorCommand};
use fly_ui::{EditorCapability, UiIntent};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::json;

#[cfg(target_arch = "wasm32")]
use crate::asset_provider::{
    AssetProviderPort, RUSTOK_MEDIA_ASSET_PROVIDER, media_provider_asset_value,
};
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
const MEDIA_LIBRARY_PAGE_SIZE: u32 = 12;

#[component]
pub(crate) fn AssetSection(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let assets_label = t(locale.as_deref(), "page_builder.panel.assets", "Assets");
    let add_label = t(locale.as_deref(), "page_builder.action.add", "Add");
    let remove_label = t(locale.as_deref(), "page_builder.action.remove", "Remove");
    let select_label = t(locale.as_deref(), "page_builder.action.select", "Use");
    let asset_id_label = t(locale.as_deref(), "page_builder.field.assetId", "Asset id");
    let asset_url_label = t(
        locale.as_deref(),
        "page_builder.field.assetUrl",
        "Asset URL",
    );
    let add_asset_accessible_label = format!("{add_label}: {assets_label}");
    let asset_id = RwSignal::new(String::new());
    let asset_url = RwSignal::new(String::new());
    let add_runtime = runtime.clone();
    let list_runtime = runtime.clone();
    let media_runtime = runtime;

    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h2 class="font-semibold">{assets_label}</h2>
            <label class="grid gap-1 text-sm">
                <span class="font-medium">{asset_id_label}</span>
                <input
                    class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                    prop:value=move || asset_id.get()
                    on:input=move |event| asset_id.set(event_target_value(&event))
                />
            </label>
            <label class="grid gap-1 text-sm">
                <span class="font-medium">{asset_url_label}</span>
                <input
                    class="w-full rounded border border-input bg-background px-2 py-1 text-sm"
                    prop:value=move || asset_url.get()
                    on:input=move |event| asset_url.set(event_target_value(&event))
                />
            </label>
            <button
                type="button"
                class="rounded border border-border px-2 py-1 text-xs"
                aria-label=add_asset_accessible_label
                on:click=move |_| {
                    let source = asset_url.get_untracked().trim().to_string();
                    if source.is_empty() {
                        add_runtime.fail("asset URL must not be empty");
                        return;
                    }
                    let id = asset_id.get_untracked().trim().to_string();
                    let asset = if id.is_empty() {
                        json!({ "src": source })
                    } else {
                        json!({ "id": id, "src": source })
                    };
                    add_runtime.dispatch(UiIntent::execute(EditorCommand::Asset {
                        command: AssetCommand::Upsert { asset },
                    }));
                }
            >{add_label}</button>

            <div class="space-y-1">
                {move || {
                    let catalog = list_runtime.controller.with(|controller| {
                        AssetCatalog::from_document(controller.editor().document())
                    });
                    catalog.assets.into_iter().map(|asset| {
                        let use_runtime = list_runtime.clone();
                        let use_action_runtime = list_runtime.clone();
                        let remove_runtime = list_runtime.clone();
                        let use_id = asset.id.clone();
                        let remove_id = asset.id.clone();
                        let accessible_name = asset.name.clone().unwrap_or_else(|| asset.id.clone());
                        let select_label = select_label.clone();
                        let remove_label = remove_label.clone();
                        let select_accessible_label = format!("{select_label}: {accessible_name}");
                        let remove_accessible_label = format!("{remove_label}: {accessible_name}");
                        view! {
                            <div class="rounded border border-border p-2 text-xs">
                                <div class="font-medium">{asset.name.clone().unwrap_or_else(|| asset.id.clone())}</div>
                                <div class="break-all text-muted-foreground">{asset.source}</div>
                                <div class="mt-2 flex gap-2">
                                    <button
                                        type="button"
                                        class="rounded border border-border px-2 py-1"
                                        aria-label=select_accessible_label
                                        disabled=move || !use_runtime.capability_enabled(EditorCapability::Properties)
                                        on:click=move |_| {
                                            let intent = use_action_runtime.controller.with(|controller| {
                                                controller.apply_asset_to_selected_intent(&use_id, "src")
                                            });
                                            use_action_runtime.dispatch_result(intent);
                                        }
                                    >{select_label}</button>
                                    <button
                                        type="button"
                                        class="rounded border border-destructive/40 px-2 py-1 text-destructive"
                                        aria-label=remove_accessible_label
                                        on:click=move |_| remove_runtime.dispatch(UiIntent::execute(
                                            EditorCommand::Asset {
                                                command: AssetCommand::Remove {
                                                    asset_id: remove_id.clone(),
                                                },
                                            },
                                        ))
                                    >{remove_label}</button>
                                </div>
                            </div>
                        }
                    }).collect_view()
                }}
            </div>
            <MediaLibraryPanel runtime=media_runtime />
        </section>
    }
}

/// Media library integration for registered asset providers.
///
/// The panel renders only when a host bound [`AssetProviderPort`] and the canvas
/// registry carries the provider declaration, so the registry entry is consumed
/// rather than decorative. Upload and browsing stay behind the host port; the
/// editor performs no raw transport.
#[cfg(target_arch = "wasm32")]
#[component]
fn MediaLibraryPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    use leptos::html;
    use leptos::task::spawn_local;

    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let Some(port) = use_context::<Arc<dyn AssetProviderPort>>() else {
        return ().into_any();
    };
    let declared = runtime.controller.with(|controller| {
        controller
            .editor()
            .registries()
            .asset_providers
            .contains(RUSTOK_MEDIA_ASSET_PROVIDER)
    });
    if !declared {
        return ().into_any();
    }

    let title = t(
        locale.as_deref(),
        "page_builder.panel.mediaLibrary",
        "Media library",
    );
    let upload_label = t(locale.as_deref(), "page_builder.media.upload", "Upload");
    let use_label = t(locale.as_deref(), "page_builder.action.select", "Use");
    let previous_label = t(
        locale.as_deref(),
        "page_builder.media.previousPage",
        "Previous page",
    );
    let next_label = t(locale.as_deref(), "page_builder.media.nextPage", "Next page");
    let choose_file_first = t(
        locale.as_deref(),
        "page_builder.media.chooseFileFirst",
        "Choose a file first.",
    );
    let upload_failed = t(
        locale.as_deref(),
        "page_builder.media.uploadFailed",
        "Upload failed",
    );
    let library_failed = t(
        locale.as_deref(),
        "page_builder.media.libraryFailed",
        "Media library unavailable",
    );

    let page = RwSignal::new(1u32);
    let refresh = RwSignal::new(0u32);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let file_input: NodeRef<html::Input> = NodeRef::new();

    let library_port = port.clone();
    let library = LocalResource::new(move || {
        let port = library_port.clone();
        let page = page.get();
        let _ = refresh.get();
        async move {
            port.library_page(page, MEDIA_LIBRARY_PAGE_SIZE)
                .await
                .map_err(|failure| failure.message().to_string())
        }
    });

    let upload_port = port.clone();
    let upload_runtime = runtime.clone();
    let on_upload = move |_| {
        let Some(input) = file_input.get() else {
            error.set(Some(choose_file_first.clone()));
            return;
        };
        let port = upload_port.clone();
        let runtime = upload_runtime.clone();
        let failed = upload_failed.clone();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match read_selected_file(input).await {
                Ok(Some(file)) => match port.upload(file.name, file.content_type, file.bytes).await {
                    Ok(item) => {
                        runtime.dispatch(UiIntent::execute(EditorCommand::Asset {
                            command: AssetCommand::Upsert {
                                asset: media_provider_asset_value(&item),
                            },
                        }));
                        refresh.update(|value| *value = value.wrapping_add(1));
                    }
                    Err(failure) => error.set(Some(format!("{failed}: {}", failure.message()))),
                },
                Ok(None) => error.set(Some(choose_file_first.clone())),
                Err(failure) => error.set(Some(format!("{failed}: {failure}"))),
            }
            busy.set(false);
        });
    };

    let use_runtime = runtime.clone();
    view! {
        <section class="space-y-2 border-t border-border pt-3">
            <h3 class="text-sm font-semibold">{title}</h3>
            {move || error.get().map(|message| view! {
                <div class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive" role="alert">
                    {message}
                </div>
            })}
            <div class="flex flex-wrap items-center gap-2">
                <input
                    node_ref=file_input
                    type="file"
                    accept="image/*,video/*,audio/*,application/pdf"
                    class="text-xs"
                    aria-label=upload_label.clone()
                />
                <button
                    type="button"
                    class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=on_upload
                >{upload_label.clone()}</button>
            </div>
            <Suspense fallback=move || {
                view! {
                    <div class="space-y-1" aria-label="Loading media library">
                        <div class="h-10 animate-pulse rounded bg-muted"></div>
                        <div class="h-10 animate-pulse rounded bg-muted"></div>
                    </div>
                }
            }>
                {move || {
                    library.get().map(|result| match result {
                        Ok(page_items) if page_items.items.is_empty() => view! {
                            <p class="text-xs text-muted-foreground">"No media items yet."</p>
                        }.into_any(),
                        Ok(page_items) => view! {
                            <ul class="space-y-1">
                                {page_items.items.into_iter().map(|item| {
                                    let use_button_runtime = use_runtime.clone();
                                    let use_select_label = use_label.clone();
                                    let item_name = item.original_name.clone();
                                    let accessible_name = format!("{use_select_label}: {item_name}");
                                    let dimensions = match (item.width, item.height) {
                                        (Some(width), Some(height)) => format!("{width}x{height}"),
                                        _ => String::new(),
                                    };
                                    view! {
                                        <li class="flex flex-wrap items-center justify-between gap-2 rounded border border-border p-2 text-xs">
                                            <div class="min-w-0">
                                                <div class="truncate font-medium">{item_name}</div>
                                                <div class="truncate text-muted-foreground">
                                                    {format!("{} {}", item.mime_type, dimensions)}
                                                </div>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded border border-border px-2 py-1"
                                                aria-label=accessible_name
                                                disabled=move || !use_button_runtime.capability_enabled(EditorCapability::Properties)
                                                on:click=move |_| {
                                                    use_button_runtime.dispatch(UiIntent::execute(
                                                        EditorCommand::Asset {
                                                            command: AssetCommand::Upsert {
                                                                asset: media_provider_asset_value(&item),
                                                            },
                                                        },
                                                    ));
                                                }
                                            >{use_select_label}</button>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                            <div class="flex items-center gap-2 pt-1">
                                <button
                                    type="button"
                                    class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                                    aria-label=previous_label.clone()
                                    disabled=move || busy.get() || page.get() <= 1
                                    on:click=move |_| page.update(|value| *value = value.saturating_sub(1).max(1))
                                >{previous_label.clone()}</button>
                                <button
                                    type="button"
                                    class="rounded border border-border px-2 py-1 text-xs disabled:opacity-50"
                                    aria-label=next_label.clone()
                                    disabled=move || busy.get()
                                    on:click=move |_| page.update(|value| *value = value.saturating_add(1))
                                >{next_label.clone()}</button>
                            </div>
                        }.into_any(),
                        Err(message) => view! {
                            <div class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive" role="alert">
                                {format!("{library_failed}: {message}")}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </section>
    }
    .into_any()
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn MediaLibraryPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let _ = runtime;
    ().into_any()
}

#[cfg(target_arch = "wasm32")]
struct SelectedUploadFile {
    name: String,
    content_type: String,
    bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
async fn read_selected_file(
    input: web_sys::HtmlInputElement,
) -> Result<Option<SelectedUploadFile>, String> {
    use wasm_bindgen_futures::JsFuture;

    let Some(files) = input.files() else {
        return Ok(None);
    };
    let Some(file) = files.get(0) else {
        return Ok(None);
    };
    let buffer = JsFuture::from(file.array_buffer())
        .await
        .map_err(|err| format!("{err:?}"))?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    let content_type = if file.type_().is_empty() {
        "application/octet-stream".to_string()
    } else {
        file.type_()
    };

    Ok(Some(SelectedUploadFile {
        name: file.name(),
        content_type,
        bytes,
    }))
}
