use super::{latest_active_registry_version, looks_like_image_url, looks_like_svg_url, tr};
use crate::Locale;
use crate::entities::module::MarketplaceModule;

#[derive(Clone)]
pub struct MetadataChecklistItem {
    pub label: &'static str,
    pub state: &'static str,
    pub priority: &'static str,
    pub summary: &'static str,
    pub detail: String,
}

pub fn metadata_status_badge_classes(state: &str) -> &'static str {
    match state {
        "ready" => {
            "inline-flex items-center rounded-full border border-emerald-500/40 bg-emerald-500/10 px-2 py-0.5 font-medium text-emerald-700"
        }
        "warn" => {
            "inline-flex items-center rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 font-medium text-amber-700"
        }
        _ => {
            "inline-flex items-center rounded-full border border-border px-2 py-0.5 font-medium text-muted-foreground"
        }
    }
}

pub fn metadata_status_panel_classes(state: &str) -> &'static str {
    match state {
        "ready" => "border-emerald-500/30 bg-emerald-500/5",
        "warn" => "border-amber-500/30 bg-amber-500/5",
        _ => "border-border bg-background",
    }
}

pub fn marketplace_metadata_checklist(
    module: &MarketplaceModule,
    locale: Locale,
) -> Vec<MetadataChecklistItem> {
    let description_length = module.description.trim().chars().count();
    let icon_url = module
        .icon_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let banner_url = module
        .banner_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let screenshots_count = module
        .screenshots
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .count();
    let publisher = module
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let latest_release = latest_active_registry_version(module).cloned();
    let latest_release_version = latest_release
        .as_ref()
        .map(|version| version.version.as_str());
    let latest_release_date = latest_release
        .as_ref()
        .and_then(|version| version.published_at.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let has_yanked_only_versions = !module.versions.is_empty() && latest_release.is_none();
    let has_registry_publish_signal = module.checksum_sha256.is_some() || latest_release.is_some();

    vec![
        if description_length >= 20 {
            MetadataChecklistItem {
                label: tr(locale, "Description", "Описание"),
                state: "ready",
                priority: "required",
                summary: tr(locale, "Ready", "Готово"),
                detail: format!(
                    "{} {}",
                    description_length,
                    tr(
                        locale,
                        "characters available for catalog detail.",
                        "символов доступно для карточки каталога.",
                    )
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Description", "Описание"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Needs at least 20 characters to satisfy local manifest validation.",
                    "Нужно минимум 20 символов, чтобы пройти локальную валидацию manifest.",
                )
                .to_string(),
            }
        },
        match icon_url {
            Some(value) if looks_like_svg_url(value) => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: tr(
                    locale,
                    "Absolute SVG icon is present for registry cards and detail previews.",
                    "Абсолютный SVG-URL иконки задан для карточек registry и detail preview.",
                )
                .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Icon URL should be an absolute http(s) SVG asset.",
                    "URL иконки должен быть абсолютным http(s) SVG-ресурсом.",
                )
                .to_string(),
            },
            None => MetadataChecklistItem {
                label: tr(locale, "Icon asset", "Иконка"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail: tr(
                    locale,
                    "Add an SVG icon URL so registry lists and cards have a visual identity.",
                    "Добавьте SVG-URL иконки, чтобы у карточек и списков registry была визуальная идентичность.",
                )
                .to_string(),
            },
        },
        match banner_url {
            Some(value) if looks_like_image_url(value) => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: tr(
                    locale,
                    "Banner image is present for richer marketplace detail layouts.",
                    "Изображение баннера доступно для более богатого detail layout в marketplace.",
                )
                .to_string(),
            },
            Some(_) => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "warn",
                priority: "required",
                summary: tr(locale, "Required", "Обязательно"),
                detail: tr(
                    locale,
                    "Banner URL should be an absolute http(s) image asset.",
                    "URL баннера должен быть абсолютным http(s) image-ресурсом.",
                )
                .to_string(),
            },
            None => MetadataChecklistItem {
                label: tr(locale, "Banner asset", "Баннер"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail:
                    tr(
                        locale,
                        "Optional for local validation, but useful for richer registry presentation.",
                        "Для локальной валидации необязательно, но полезно для richer presentation в registry.",
                    )
                    .to_string(),
            },
        },
        if screenshots_count > 0 {
            MetadataChecklistItem {
                label: tr(locale, "Screenshots", "Скриншоты"),
                state: "ready",
                priority: "recommended",
                summary: tr(locale, "Ready", "Готово"),
                detail: format!(
                    "{} {}",
                    screenshots_count,
                    tr(locale, "screenshot(s) available for discovery UX.", "скриншотов доступно для discovery UX.")
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Screenshots", "Скриншоты"),
                state: "warn",
                priority: "recommended",
                summary: tr(locale, "Recommended", "Рекомендуется"),
                detail:
                    tr(
                        locale,
                        "Add one or more screenshots to make module capabilities easier to evaluate.",
                        "Добавьте один или несколько скриншотов, чтобы возможности модуля было проще оценивать.",
                    )
                    .to_string(),
            }
        },
        if let Some(publisher) = publisher {
            MetadataChecklistItem {
                label: tr(locale, "Publisher identity", "Идентичность издателя"),
                state: "ready",
                priority: "info",
                summary: tr(locale, "Known", "Известен"),
                detail: format!(
                    "{} {publisher}.",
                    tr(locale, "Publisher is exposed as", "Издатель указан как")
                ),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Publisher identity", "Идентичность издателя"),
                state: "info",
                priority: "info",
                summary: tr(locale, "Local only", "Только локально"),
                detail: tr(
                    locale,
                    "Workspace modules can stay unpublished; external registry entries should declare a publisher.",
                    "Workspace-модули могут оставаться неопубликованными; внешние записи registry должны указывать publisher.",
                )
                .to_string(),
            }
        },
        if has_registry_publish_signal {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "ready",
                priority: "info",
                summary: tr(locale, "Present", "Есть"),
                detail: match (latest_release_version, latest_release_date) {
                    (Some(version), Some(date)) => {
                        format!(
                            "{} v{version} {} {date}.",
                            tr(locale, "Latest non-yanked release is", "Последний неотозванный релиз"),
                            tr(locale, "published at", "опубликован")
                        )
                    }
                    (Some(version), None) => {
                        format!(
                            "{} v{version}, {}.",
                            tr(locale, "Latest non-yanked release is", "Последний неотозванный релиз"),
                            tr(locale, "but publish date is missing", "но дата публикации отсутствует")
                        )
                    }
                    (None, _) => {
                        tr(
                            locale,
                            "Checksum is present even though no active version entry is visible.",
                            "Контрольная сумма есть, хотя активная запись версии не видна.",
                        )
                        .to_string()
                    }
                },
            }
        } else if has_yanked_only_versions {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "warn",
                priority: "info",
                summary: tr(locale, "Only yanked", "Только отозванные"),
                detail:
                    tr(
                        locale,
                        "Version history exists, but every visible release is yanked, so there is no active publish trail.",
                        "История версий существует, но все видимые релизы отозваны, поэтому активной publish-цепочки нет.",
                    )
                    .to_string(),
            }
        } else {
            MetadataChecklistItem {
                label: tr(locale, "Release trail", "История релизов"),
                state: "info",
                priority: "info",
                summary: tr(locale, "Not published", "Не опубликован"),
                detail:
                    tr(
                        locale,
                        "No checksum or active version history is visible yet, which is expected for workspace-only modules.",
                        "Контрольная сумма и активная история версий пока не видны, что нормально для workspace-only модулей.",
                    )
                    .to_string(),
            }
        },
    ]
}
