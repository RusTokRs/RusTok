use crate::editor::AdminEditorRuntime;
use crate::{PageBuilderContributionHostContext, PageBuilderContributionPreviewRequest};
use fly_ui::{ContributionAssemblyResult, Presentation};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{Map, Value};
use std::sync::Arc;

const MAX_PREVIEW_JSON_BYTES: usize = 16 * 1024;

#[component]
pub fn ContributionPreviewPanel(
    runtime: AdminEditorRuntime,
    #[prop(optional_no_strip)] contribution_assembly: Option<Arc<ContributionAssemblyResult>>,
) -> impl IntoView {
    let Some(host) = use_context::<PageBuilderContributionHostContext>() else {
        return ().into_any();
    };
    if host.is_empty() {
        return ().into_any();
    }
    let Some(assembly) = contribution_assembly else {
        return ().into_any();
    };

    let busy = RwSignal::new(false);
    let result = RwSignal::new(None::<Result<String, String>>);
    let request_runtime = runtime.clone();
    let request_host = host.clone();
    let request_assembly = assembly.clone();
    let selected_request = Signal::derive(move || {
        selected_preview_request(&request_runtime, &request_assembly, &request_host)
    });

    let click_host = host;
    let on_preview = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(request) = selected_request.get_untracked() else {
            result.set(Some(Err(
                "Selected component has no admitted owner preview contract".to_string(),
            )));
            return;
        };
        let Some(port) = click_host.preview_port(&request.provider) else {
            result.set(Some(Err(format!(
                "Preview provider `{}` is not mounted",
                request.provider
            ))));
            return;
        };
        busy.set(true);
        result.set(None);
        spawn_local(async move {
            match port.preview(request).await {
                Ok(value) => result.set(Some(Ok(preview_json_summary(&value)))),
                Err(error) => result.set(Some(Err(error.to_string()))),
            }
            busy.set(false);
        });
    };

    view! {
        <section
            class="rounded-xl border border-border bg-card p-3"
            data-page-builder-contribution-preview="true"
        >
            <div class="flex items-start justify-between gap-3">
                <div>
                    <h2 class="text-sm font-semibold text-card-foreground">"Component owner preview"</h2>
                    <p class="mt-1 text-xs text-muted-foreground">
                        "Loads selected dynamic component data through its owner transport."
                    </p>
                </div>
                <button
                    type="button"
                    class="rounded border border-border px-3 py-1.5 text-sm disabled:opacity-50"
                    disabled=move || busy.get() || selected_request.get().is_none()
                    on:click=on_preview
                >
                    {move || if busy.get() { "Loading..." } else { "Refresh" }}
                </button>
            </div>
            {move || match selected_request.get() {
                Some(request) => view! {
                    <p class="mt-2 text-xs text-muted-foreground" data-page-builder-contribution-preview-provider=request.provider.clone()>
                        {format!("{} · {}", request.provider, request.component_type)}
                    </p>
                }.into_any(),
                None => view! {
                    <p class="mt-2 text-xs text-muted-foreground">
                        "Select a component with an admitted preview renderer."
                    </p>
                }.into_any(),
            }}
            {move || match result.get() {
                Some(Ok(payload)) => view! {
                    <pre
                        class="mt-3 max-h-72 overflow-auto whitespace-pre-wrap rounded-lg bg-muted/40 p-3 text-xs text-foreground"
                        data-page-builder-contribution-preview-result="ready"
                    >{payload}</pre>
                }.into_any(),
                Some(Err(error)) => view! {
                    <div
                        class="mt-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                        role="alert"
                        data-page-builder-contribution-preview-result="error"
                    >{error}</div>
                }.into_any(),
                None => ().into_any(),
            }}
        </section>
    }
    .into_any()
}

fn selected_preview_request(
    runtime: &AdminEditorRuntime,
    assembly: &ContributionAssemblyResult,
    host: &PageBuilderContributionHostContext,
) -> Option<PageBuilderContributionPreviewRequest> {
    runtime.controller.with(|controller| {
        let component_id = controller.ui().state.selection.component_id.as_deref()?;
        let component = controller.editor().document().component(component_id)?;
        let provider = component.provider.as_deref()?.trim();
        let component_type = component.component_type().trim();
        if provider.is_empty() || component_type.is_empty() || host.preview_port(provider).is_none()
        {
            return None;
        }
        let admitted = assembly.registry.iter().any(|(_, contribution)| {
            contribution.renderers.iter().any(|renderer| {
                renderer.provider == provider
                    && renderer.component_type == component_type
                    && renderer.presentations.contains(&Presentation::Preview)
            })
        });
        if !admitted {
            return None;
        }
        let props = component
            .extensions
            .get("props")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        if !props.is_object() {
            return None;
        }

        Some(PageBuilderContributionPreviewRequest {
            provider: provider.to_string(),
            component_type: component_type.to_string(),
            component_id: component_id.to_string(),
            presentation: Presentation::Preview,
            props,
        })
    })
}

fn preview_json_summary(value: &Value) -> String {
    let text = serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("Preview response could not serialize: {error}"));
    if text.len() <= MAX_PREVIEW_JSON_BYTES {
        return text;
    }
    let mut end = MAX_PREVIEW_JSON_BYTES;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}\n… preview truncated …", &text[..end])
}
