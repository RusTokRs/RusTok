use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    RuntimeContextExamplePolicy, export_runtime_context_json_schema,
    generate_runtime_context_example,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn ContextContractToolsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.contextContractTools",
        "Context contract tools",
    );
    let apply_example_label = t(
        locale.as_deref(),
        "page_builder.action.applyGeneratedContext",
        "Use generated example",
    );
    let apply_runtime = runtime.clone();
    let report_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            {move || {
                let (schema, example) = report_runtime.controller.with(|controller| {
                    let document = controller.editor().document();
                    (
                        export_runtime_context_json_schema(document),
                        generate_runtime_context_example(
                            document,
                            RuntimeContextExamplePolicy::default(),
                        ),
                    )
                });
                let schema_json = serde_json::to_string_pretty(&schema.schema)
                    .unwrap_or_else(|_| "{}".to_string());
                let input_example = example.input_context.clone();
                let issue_count = schema.diagnostics.len() + example.diagnostics.len();
                view! {
                    <div class="space-y-2 text-xs">
                        <p class="break-all text-muted-foreground">
                            {format!("Contract hash: {}", schema.contract_hash)}
                        </p>
                        <p class="text-muted-foreground">
                            {format!("{issue_count} contract/example diagnostic(s)")}
                        </p>
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            on:click={
                                let apply_runtime = apply_runtime.clone();
                                move |_| {
                                    apply_runtime.set_runtime_context(input_example.clone());
                                    apply_runtime.announce("Generated runtime context example applied");
                                }
                            }
                        >{apply_example_label.clone()}</button>
                        <details>
                            <summary class="cursor-pointer font-medium">"JSON Schema"</summary>
                            <textarea
                                class="mt-2 min-h-48 w-full rounded border border-input bg-muted/30 px-2 py-1 font-mono text-[11px]"
                                readonly
                                prop:value=schema_json
                            ></textarea>
                        </details>
                    </div>
                }
            }}
        </section>
    }
}
