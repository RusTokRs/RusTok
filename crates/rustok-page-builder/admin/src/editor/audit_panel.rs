use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{AuditSeverity, audit_page};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
pub fn AuditPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale;
    let title = t(
        locale.as_deref(),
        "page_builder.panel.audit",
        "Accessibility audit",
    );
    let clean = t(
        locale.as_deref(),
        "page_builder.audit.clean",
        "No accessibility or SEO issues found for this page.",
    );
    let select = t(
        locale.as_deref(),
        "page_builder.audit.select",
        "Select component",
    );
    let errors_label = t(locale.as_deref(), "page_builder.audit.errors", "errors");
    let warnings_label = t(locale.as_deref(), "page_builder.audit.warnings", "warnings");
    let info_label = t(locale.as_deref(), "page_builder.audit.info", "info");
    let components_label = t(
        locale.as_deref(),
        "page_builder.audit.components",
        "components",
    );
    let error_label = t(locale.as_deref(), "page_builder.audit.error", "Error");
    let warning_label = t(locale.as_deref(), "page_builder.audit.warning", "Warning");
    let informational_label = t(
        locale.as_deref(),
        "page_builder.audit.informational",
        "Info",
    );
    let panel_runtime = runtime;

    view! {
        <section class="space-y-3 rounded-xl border border-border bg-card p-3">
            <h2 class="font-semibold">{title}</h2>
            {move || {
                let report = panel_runtime.controller.with(|controller| {
                    audit_page(
                        controller.editor().document(),
                        &controller.active_page_locator(),
                    )
                });
                let summary = format!(
                    "{} {errors_label} · {} {warnings_label} · {} {info_label} · {} {components_label}",
                    report.error_count,
                    report.warning_count,
                    report.info_count,
                    report.component_count,
                );
                if report.diagnostics.is_empty() {
                    return view! {
                        <div class="space-y-2">
                            <p class="text-xs text-muted-foreground">{summary}</p>
                            <p class="rounded bg-emerald-500/10 px-2 py-2 text-sm text-emerald-700">
                                {clean.clone()}
                            </p>
                        </div>
                    }
                    .into_any();
                }

                view! {
                    <div class="space-y-2">
                        <p class="text-xs text-muted-foreground">{summary}</p>
                        {report.diagnostics.into_iter().map(|diagnostic| {
                            let (severity_class, severity_label) = match diagnostic.severity {
                                AuditSeverity::Error => (
                                    "border-destructive/40 bg-destructive/10",
                                    error_label.clone(),
                                ),
                                AuditSeverity::Warning => (
                                    "border-amber-500/40 bg-amber-500/10",
                                    warning_label.clone(),
                                ),
                                AuditSeverity::Info => (
                                    "border-border bg-muted/50",
                                    informational_label.clone(),
                                ),
                            };
                            let runtime = panel_runtime.clone();
                            let component_id = diagnostic.component_id.clone();
                            let select = select.clone();
                            view! {
                                <article class=format!("space-y-1 rounded border p-2 text-xs {severity_class}")>
                                    <div class="flex items-start justify-between gap-2">
                                        <strong>{diagnostic.code}</strong>
                                        <span class="uppercase text-muted-foreground">{severity_label}</span>
                                    </div>
                                    <p>{diagnostic.message}</p>
                                    <code class="block break-all text-muted-foreground">{diagnostic.path}</code>
                                    {diagnostic.suggestion.map(|suggestion| view! {
                                        <p class="text-muted-foreground">{suggestion}</p>
                                    })}
                                    {component_id.map(|component_id| view! {
                                        <button
                                            type="button"
                                            class="rounded border border-border px-2 py-1"
                                            on:click=move |_| runtime.dispatch(
                                                UiIntent::Select(Some(component_id.clone()))
                                            )
                                        >{select}</button>
                                    })}
                                </article>
                            }
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }}
        </section>
    }
}
