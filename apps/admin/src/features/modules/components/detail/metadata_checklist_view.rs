use super::metadata::{
    MetadataChecklistItem, metadata_status_badge_classes, metadata_status_panel_classes,
};
use super::tr;
use crate::Locale;
use leptos::prelude::*;

#[component]
pub fn MetadataChecklistView(
    locale: Locale,
    module_source: String,
    metadata_checklist: Vec<MetadataChecklistItem>,
    metadata_required_issues: usize,
    metadata_recommended_gaps: usize,
    metadata_ready_count: usize,
) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-border bg-background/70 p-4">
            <div class="flex flex-wrap items-center gap-2">
                <p class="text-xs uppercase tracking-wide text-muted-foreground">
                    {tr(locale, "Registry readiness", "Готовность к registry")}
                </p>
                <span class=metadata_status_badge_classes(if metadata_required_issues > 0 { "warn" } else { "ready" })>
                    {if metadata_required_issues > 0 {
                        format!("{} required issue(s)", metadata_required_issues)
                    } else {
                        tr(locale, "No required metadata gaps", "Обязательных пробелов в метаданных нет").to_string()
                    }}
                </span>
                <span class=metadata_status_badge_classes(if metadata_recommended_gaps > 0 { "warn" } else { "ready" })>
                    {if metadata_recommended_gaps > 0 {
                        format!("{} recommended gap(s)", metadata_recommended_gaps)
                    } else {
                        tr(locale, "Recommended visuals look complete", "Рекомендуемые визуальные материалы заполнены").to_string()
                    }}
                </span>
                <span class=metadata_status_badge_classes("info")>
                    {format!("{} ready signal(s)", metadata_ready_count)}
                </span>
            </div>
            <div class="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                {metadata_checklist.into_iter().map(|item| {
                    view! {
                        <div class=format!(
                            "rounded-lg border p-3 text-sm {}",
                            metadata_status_panel_classes(item.state)
                        )>
                            <div class="flex flex-wrap items-center justify-between gap-2">
                                <p class="font-medium text-card-foreground">{item.label}</p>
                                <span class=metadata_status_badge_classes(item.state)>
                                    {item.summary}
                                </span>
                            </div>
                            <p class="mt-2 text-xs text-muted-foreground">{item.detail}</p>
                        </div>
                    }
                }).collect_view()}
            </div>
            <p class="mt-3 text-xs text-muted-foreground">
                {if module_source.eq_ignore_ascii_case("path") {
                    tr(locale, "Workspace path modules can stay unpublished; this checklist is meant to surface what is already registry-ready versus what still needs operator follow-up.", "Workspace path-модули могут оставаться неопубликованными; этот checklist показывает, что уже готово для registry, а что ещё требует внимания оператора.")
                } else {
                    tr(locale, "Registry-backed modules should ideally arrive here with the required metadata already satisfied.", "Registry-backed модули в идеале должны приходить сюда уже с заполненными обязательными метаданными.")
                }}
            </p>
        </div>
    }
}
