use super::{short_checksum, tr};
use crate::Locale;
use crate::entities::module::model::MarketplaceModuleVersion;
use leptos::prelude::*;

#[component]
pub fn VersionTrailView(
    locale: Locale,
    version_trail: Vec<MarketplaceModuleVersion>,
    loading: Signal<bool>,
) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-border bg-background/70 p-4">
            <div class="flex items-center gap-2">
                <p class="text-xs uppercase tracking-wide text-muted-foreground">
                    {tr(locale, "Version history", "История версий")}
                </p>
                <Show when=move || loading.get()>
                    <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                        {tr(locale, "Refreshing", "Обновление")}
                    </span>
                </Show>
            </div>
            {if version_trail.is_empty() {
                view! {
                    <p class="mt-3 text-sm text-muted-foreground">
                        {tr(locale, "No version history has been published for this module yet.", "Для этого модуля история версий пока не опубликована.")}
                    </p>
                }
                    .into_any()
            } else {
                view! {
                    <div class="mt-3 space-y-3">
                        {version_trail.into_iter().map(|version| {
                            let checksum = short_checksum(version.checksum_sha256.as_deref());
                            view! {
                                <div class="flex flex-col gap-2 rounded-lg border border-border px-3 py-3 text-sm">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class="inline-flex items-center rounded-full border border-border px-2.5 py-0.5 text-xs font-medium text-muted-foreground">
                                            {format!("v{}", version.version)}
                                        </span>
                                        {version.yanked.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-destructive px-2.5 py-0.5 text-xs font-semibold text-destructive-foreground">
                                                {tr(locale, "Yanked", "Отозван")}
                                            </span>
                                        })}
                                        {version.signature_present.then(|| view! {
                                            <span class="inline-flex items-center rounded-full bg-secondary px-2.5 py-0.5 text-xs font-semibold text-secondary-foreground">
                                                {tr(locale, "Signed", "Подписан")}
                                            </span>
                                        })}
                                        <span class="text-xs text-muted-foreground">
                                            {version.published_at.unwrap_or_else(|| tr(locale, "Unknown", "Неизвестно").to_string())}
                                        </span>
                                    </div>
                                    {version.changelog.map(|changelog| view! {
                                        <p class="text-sm text-muted-foreground">{changelog}</p>
                                    })}
                                    {checksum.map(|checksum| view! {
                                        <div class="text-xs text-muted-foreground">
                                            <span class="font-mono">{format!("sha256 {}", checksum)}</span>
                                        </div>
                                    })}
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}
