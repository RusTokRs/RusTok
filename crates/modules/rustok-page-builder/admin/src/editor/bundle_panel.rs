use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    BundleDecodePolicy, BundleInspection, BundleMetadata, ValidationLimits,
    decode_project_bundle_value, encode_project_bundle, export_project_bundle,
    inspect_project_bundle,
};
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde_json::Value;

#[component]
pub fn ProjectBundlePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.bundle",
        "Project bundle",
    );
    let export_label = t(
        locale.as_deref(),
        "page_builder.action.exportBundle",
        "Export bundle",
    );
    let inspect_label = t(
        locale.as_deref(),
        "page_builder.action.inspectImport",
        "Inspect import",
    );
    let clear_label = t(locale.as_deref(), "page_builder.action.clear", "Clear");
    let allow_mismatch_label = t(
        locale.as_deref(),
        "page_builder.bundle.allowHashMismatch",
        "Allow hash mismatch for inspection",
    );
    let export_json = RwSignal::new(String::new());
    let import_json = RwSignal::new(String::new());
    let inspection = RwSignal::new(None::<BundleInspection>);
    let allow_hash_mismatch = RwSignal::new(false);
    let export_runtime = runtime.clone();
    let inspect_runtime = runtime.clone();
    let diagnostics_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            <details>
                <summary class="cursor-pointer text-sm font-medium">{export_label.clone()}</summary>
                <div class="mt-2 space-y-2">
                    <button
                        type="button"
                        class="rounded border border-border px-2 py-1 text-xs"
                        on:click=move |_| {
                            let bundle = export_runtime.controller.with(|controller| {
                                export_project_bundle(
                                    controller.editor().document(),
                                    BundleMetadata {
                                        name: controller
                                            .active_page_summary()
                                            .map(|page| page.name),
                                        source_module: Some("page_builder".to_string()),
                                        source_document_id: Some(controller.page_id().to_string()),
                                        source_revision_id: Some(controller.revision_id().to_string()),
                                        ..BundleMetadata::default()
                                    },
                                )
                            });
                            match bundle
                                .and_then(|bundle| encode_project_bundle(&bundle, true))
                                .and_then(|bytes| String::from_utf8(bytes).map_err(|error| {
                                    fly::FlyError::Encode(error.to_string())
                                }))
                            {
                                Ok(json) => {
                                    export_runtime.last_error.set(None);
                                    export_json.set(json);
                                    export_runtime.announce("Project bundle exported");
                                }
                                Err(error) => export_runtime.fail(error.to_string()),
                            }
                        }
                    >{export_label}</button>
                    <textarea
                        class="min-h-48 w-full rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                        readonly
                        prop:value=move || export_json.get()
                    ></textarea>
                </div>
            </details>

            <details class="border-t border-border pt-3">
                <summary class="cursor-pointer text-sm font-medium">{inspect_label.clone()}</summary>
                <div class="mt-2 space-y-2">
                    <textarea
                        class="min-h-48 w-full rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                        placeholder="Paste fly_project_bundle or raw grapesjs JSON"
                        prop:value=move || import_json.get()
                        on:input=move |event| {
                            import_json.set(event_target_value(&event));
                            inspection.set(None);
                        }
                    ></textarea>
                    <label class="flex items-center gap-2 text-xs">
                        <input
                            type="checkbox"
                            prop:checked=move || allow_hash_mismatch.get()
                            on:change=move |event| allow_hash_mismatch.set(event_target_checked(&event))
                        />
                        <span>{allow_mismatch_label}</span>
                    </label>
                    <div class="flex gap-2">
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            on:click=move |_| {
                                let value = serde_json::from_str::<Value>(&import_json.get_untracked());
                                let decoded = value
                                    .map_err(|error| fly::FlyError::Decode(error.to_string()))
                                    .and_then(|value| decode_project_bundle_value(
                                        value,
                                        &BundleDecodePolicy {
                                            allow_hash_mismatch: allow_hash_mismatch.get_untracked(),
                                            ..BundleDecodePolicy::default()
                                        },
                                    ));
                                match decoded {
                                    Ok(decoded) => {
                                        let report = inspect_runtime.controller.with(|controller| {
                                            inspect_project_bundle(
                                                &decoded,
                                                controller.editor().registries(),
                                                ValidationLimits::default(),
                                            )
                                        });
                                        inspect_runtime.last_error.set(None);
                                        inspection.set(Some(report));
                                        inspect_runtime.announce("Import bundle inspected");
                                    }
                                    Err(error) => {
                                        inspection.set(None);
                                        inspect_runtime.fail(error.to_string());
                                    }
                                }
                            }
                        >{inspect_label}</button>
                        <button
                            type="button"
                            class="rounded border border-border px-2 py-1 text-xs"
                            on:click=move |_| {
                                import_json.set(String::new());
                                inspection.set(None);
                                diagnostics_runtime.last_error.set(None);
                            }
                        >{clear_label}</button>
                    </div>
                    {move || inspection.get().map(|inspection| view! {
                        <BundleInspectionView inspection />
                    })}
                </div>
            </details>
        </section>
    }
}

#[component]
fn BundleInspectionView(inspection: BundleInspection) -> impl IntoView {
    let status = if inspection.hash_matches {
        "hash verified"
    } else {
        "hash mismatch"
    };
    view! {
        <div class="space-y-2 rounded border border-border bg-muted/40 p-2 text-xs">
            <div class="flex flex-wrap gap-x-3 gap-y-1">
                <strong>{status}</strong>
                <span>{format!("{} pages", inspection.page_count)}</span>
                <span>{format!("{} nodes", inspection.node_count)}</span>
                <span>{format!("{} assets", inspection.asset_count)}</span>
                <span>{format!("{} style rules", inspection.style_rule_count)}</span>
            </div>
            <code class="block break-all">{format!(
                "declared {} · actual {}",
                inspection.declared_hash,
                inspection.actual_hash,
            )}</code>
            <p>{format!(
                "validation: {} errors / {} warnings · audit: {} errors / {} warnings",
                inspection.validation.errors().count(),
                inspection.validation.warnings().count(),
                inspection.audit_error_count,
                inspection.audit_warning_count,
            )}</p>
            <div class="space-y-1">
                {inspection.validation.diagnostics.into_iter().take(20).map(|diagnostic| view! {
                    <div class="rounded bg-background px-2 py-1">
                        <strong>{diagnostic.code}</strong>
                        <span class="ml-1">{diagnostic.message}</span>
                    </div>
                }).collect_view()}
            </div>
        </div>
    }
}
