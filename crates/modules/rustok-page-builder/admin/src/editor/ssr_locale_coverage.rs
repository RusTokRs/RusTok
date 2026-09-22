use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
#[cfg(test)]
use fly::LocaleCoverageReport;
#[cfg(not(target_arch = "wasm32"))]
use fly::analyze_project_locale_coverage;
#[cfg(not(target_arch = "wasm32"))]
use fly::{LocaleCoverageGap, LocaleCoverageKind, LocaleCoverageSummary};
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;

#[component]
pub fn SsrLocaleCoveragePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.localeCoverage.title",
            "Locale coverage",
        );
        let description = t(
            locale.as_deref(),
            "page_builder.localeCoverage.description",
            "Coverage is calculated from project translations and localized page metadata.",
        );
        let policy_invalid = t(
            locale.as_deref(),
            "page_builder.localeCoverage.policyInvalid",
            "The project locale policy is invalid.",
        );
        let ready = t(
            locale.as_deref(),
            "page_builder.localeCoverage.ready",
            "Required locale coverage is complete.",
        );
        let incomplete = t(
            locale.as_deref(),
            "page_builder.localeCoverage.incomplete",
            "Required locale coverage is incomplete.",
        );
        let strict_on = t(
            locale.as_deref(),
            "page_builder.localeCoverage.strictOn",
            "Strict publish enforcement is enabled.",
        );
        let strict_off = t(
            locale.as_deref(),
            "page_builder.localeCoverage.strictOff",
            "Strict publish enforcement is disabled.",
        );
        let required_badge = t(
            locale.as_deref(),
            "page_builder.localeCoverage.requiredBadge",
            "Required",
        );
        let optional_badge = t(
            locale.as_deref(),
            "page_builder.localeCoverage.optionalBadge",
            "Optional",
        );
        let translations_label = t(
            locale.as_deref(),
            "page_builder.localeCoverage.translations",
            "Translations",
        );
        let metadata_label = t(
            locale.as_deref(),
            "page_builder.localeCoverage.metadata",
            "Metadata",
        );
        let missing_label = t(
            locale.as_deref(),
            "page_builder.localeCoverage.missing",
            "Missing",
        );
        let gaps_title = t(
            locale.as_deref(),
            "page_builder.localeCoverage.gapsTitle",
            "Coverage gaps",
        );
        let no_gaps = t(
            locale.as_deref(),
            "page_builder.localeCoverage.noGaps",
            "No locale coverage gaps were found.",
        );
        let translation_gap = t(
            locale.as_deref(),
            "page_builder.localeCoverage.translationGap",
            "Translation",
        );
        let metadata_gap = t(
            locale.as_deref(),
            "page_builder.localeCoverage.metadataGap",
            "Page metadata",
        );
        let report = runtime
            .controller
            .with(|controller| analyze_project_locale_coverage(controller.editor().document()));
        let policy_valid = report.policy_valid;
        let required_complete = report.required_complete();
        let strict_ready = report.strict_ready();
        let status = if !policy_valid {
            policy_invalid
        } else if required_complete {
            ready
        } else {
            incomplete
        };
        let enforcement = if report.strict_enforcement {
            strict_on
        } else {
            strict_off
        };
        let summaries = report.summaries;
        let gaps = report.gaps;
        let has_gaps = !gaps.is_empty();

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-locale-coverage="true"
                data-policy-valid=policy_valid
                data-required-complete=required_complete
                data-strict-ready=strict_ready
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{description}</p>
                </div>
                <div class="rounded border border-border px-3 py-2 text-xs">
                    <strong class="block">{status}</strong>
                    <span class="text-muted-foreground">{enforcement}</span>
                </div>
                <div class="grid gap-2">
                    {summaries.into_iter().map(|summary| {
                        locale_summary_view(
                            summary,
                            required_badge.clone(),
                            optional_badge.clone(),
                            translations_label.clone(),
                            metadata_label.clone(),
                            missing_label.clone(),
                        )
                    }).collect_view()}
                </div>
                <details class="rounded border border-border p-2" open=has_gaps>
                    <summary class="cursor-pointer text-xs font-semibold">{gaps_title}</summary>
                    <div class="mt-2 space-y-1">
                        {if has_gaps {
                            gaps.into_iter().map(|gap| {
                                locale_gap_view(
                                    gap,
                                    translation_gap.clone(),
                                    metadata_gap.clone(),
                                    required_badge.clone(),
                                    optional_badge.clone(),
                                )
                            }).collect_view().into_any()
                        } else {
                            view! {
                                <p class="text-xs text-muted-foreground">{no_gaps}</p>
                            }.into_any()
                        }}
                    </div>
                </details>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-locale-coverage="disabled"></span> }.into_any()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_summary_view(
    summary: LocaleCoverageSummary,
    required_badge: String,
    optional_badge: String,
    translations_label: String,
    metadata_label: String,
    missing_label: String,
) -> impl IntoView {
    let badge = if summary.required {
        required_badge
    } else {
        optional_badge
    };
    let translation_count = format!(
        "{} / {}",
        summary.translation_present, summary.translation_total
    );
    let metadata_count = format!("{} / {}", summary.metadata_present, summary.metadata_total);
    let missing_count = summary.missing.to_string();
    view! {
        <article
            class="rounded border border-border px-3 py-2 text-xs"
            data-fly-locale-summary=summary.locale.clone()
            data-complete=summary.complete()
            data-required=summary.required
        >
            <div class="flex items-center justify-between gap-2">
                <strong>{summary.locale.clone()}</strong>
                <span class="rounded bg-muted px-2 py-0.5 text-[11px]">{badge}</span>
            </div>
            <dl class="mt-2 grid grid-cols-3 gap-2 text-muted-foreground">
                <div>
                    <dt>{translations_label}</dt>
                    <dd class="font-medium text-foreground">{translation_count}</dd>
                </div>
                <div>
                    <dt>{metadata_label}</dt>
                    <dd class="font-medium text-foreground">{metadata_count}</dd>
                </div>
                <div>
                    <dt>{missing_label}</dt>
                    <dd class="font-medium text-foreground">{missing_count}</dd>
                </div>
            </dl>
        </article>
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_gap_view(
    gap: LocaleCoverageGap,
    translation_gap: String,
    metadata_gap: String,
    required_badge: String,
    optional_badge: String,
) -> impl IntoView {
    let kind = match gap.kind {
        LocaleCoverageKind::Translation => translation_gap,
        LocaleCoverageKind::PageMetadata => metadata_gap,
    };
    let badge = if gap.required {
        required_badge
    } else {
        optional_badge
    };
    view! {
        <article
            class="rounded border border-border px-2 py-1.5 text-xs"
            data-fly-locale-gap=gap.locale.clone()
            data-required=gap.required
        >
            <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                <strong>{gap.locale.clone()}</strong>
                <span>{kind}</span>
                <span class="rounded bg-muted px-1.5 py-0.5 text-[10px]">{badge}</span>
                <span class="font-medium">{gap.label}</span>
            </div>
            <code class="mt-1 block break-all text-[10px] text-muted-foreground">{gap.path}</code>
        </article>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{GrapesJsCodec, ProjectDocument};
    use serde_json::json;

    fn report() -> LocaleCoverageReport {
        let document: ProjectDocument = GrapesJsCodec::decode_value(json!({
            "flyLocales": {
                "supported_locales": ["en", "ru"],
                "required_locales": ["en", "ru"]
            },
            "flyTranslations": [{
                "id": "hero",
                "values": { "en": "Welcome" }
            }],
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }]
        }))
        .expect("document");
        analyze_project_locale_coverage(&document)
    }

    #[test]
    fn coverage_panel_model_exposes_required_gap_paths() {
        let report = report();
        assert!(!report.required_complete());
        assert_eq!(report.required_gaps().count(), 1);
        assert_eq!(
            report.required_gaps().next().unwrap().path,
            "project.translations.hero"
        );
    }
}
