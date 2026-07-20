use crate::AdminCanvasController;
use crate::editor::AdminEditorRuntime;
use crate::i18n::t;
use fly::{
    EditorCommand, FLY_PAGE_METADATA_FIELD, LOCALIZED_FALLBACK_FIELD, LOCALIZED_VALUES_FIELD,
    PageCommand, PageLocator, PagePatch, ProjectPage, normalize_locale_tag, normalize_slug,
};
use fly_ui::UiIntent;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_LOCALIZED_METADATA_BYTES: usize = 256 * 1024;
const LOCALIZABLE_METADATA_FIELDS: &[&str] = &[
    "title",
    "description",
    "slug",
    "canonical_url",
    "open_graph_title",
    "open_graph_description",
    "open_graph_image",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SsrLocalizedPageMetadataRequest {
    pub page_id: String,
    pub metadata_json: String,
    #[serde(default)]
    pub fallback_locale: String,
}

impl AdminCanvasController {
    pub fn ssr_localized_page_metadata_intent(
        &self,
        request: SsrLocalizedPageMetadataRequest,
    ) -> Result<UiIntent, String> {
        let page_id = request.page_id.trim();
        if page_id.is_empty() {
            return Err("localized metadata requires page_id".to_string());
        }
        if request.metadata_json.len() > MAX_LOCALIZED_METADATA_BYTES {
            return Err(format!(
                "localized metadata exceeds {MAX_LOCALIZED_METADATA_BYTES} bytes"
            ));
        }
        let updates = serde_json::from_str::<Value>(&request.metadata_json)
            .map_err(|error| format!("localized metadata JSON is invalid: {error}"))?
            .as_object()
            .cloned()
            .ok_or_else(|| "localized metadata must be a JSON object".to_string())?;
        let fallback_locale = if request.fallback_locale.trim().is_empty() {
            None
        } else {
            Some(
                normalize_locale_tag(&request.fallback_locale).ok_or_else(|| {
                    format!(
                        "localized metadata fallback locale `{}` is invalid",
                        request.fallback_locale.trim()
                    )
                })?,
            )
        };
        let locator = PageLocator::by_id(page_id);
        let page = self
            .editor()
            .document()
            .page(&locator)
            .map_err(|error| error.to_string())?;
        let mut metadata = page
            .extensions
            .get(FLY_PAGE_METADATA_FIELD)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        for (field, value) in updates {
            if !LOCALIZABLE_METADATA_FIELDS.contains(&field.as_str()) {
                return Err(format!(
                    "localized metadata field `{field}` is not supported"
                ));
            }
            if value.is_null() {
                metadata.remove(&field);
                continue;
            }
            let values = normalize_localized_metadata_values(&field, value)?;
            if let Some(fallback_locale) = fallback_locale.as_deref() {
                if !values.contains_key(fallback_locale) {
                    return Err(format!(
                        "localized metadata field `{field}` has no value for fallback locale `{fallback_locale}`"
                    ));
                }
            }
            let mut wrapper = Map::new();
            wrapper.insert(LOCALIZED_VALUES_FIELD.to_string(), Value::Object(values));
            if let Some(fallback_locale) = fallback_locale.as_deref() {
                wrapper.insert(
                    LOCALIZED_FALLBACK_FIELD.to_string(),
                    Value::String(fallback_locale.to_string()),
                );
            }
            metadata.insert(field, Value::Object(wrapper));
        }

        Ok(UiIntent::execute(EditorCommand::Page {
            command: PageCommand::Patch {
                locator,
                patch: PagePatch {
                    fields: Map::from_iter([(
                        FLY_PAGE_METADATA_FIELD.to_string(),
                        Value::Object(metadata),
                    )]),
                    ..PagePatch::default()
                },
            },
        }))
    }
}

#[component]
pub fn SsrLocalizedMetadataPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataTitle",
            "Localized page metadata",
        );
        let help = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataHelp",
            "Provide locale maps for title, description, slug, canonical_url, open_graph_title, open_graph_description, or open_graph_image. Existing unknown metadata is preserved.",
        );
        let values_label = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataValuesLabel",
            "Localized metadata JSON",
        );
        let values_placeholder = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataValuesPlaceholder",
            "{\n  \"title\": {\"en\": \"Home\", \"ru\": \"Главная\"}\n}",
        );
        let fallback_label = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataFallbackLabel",
            "Metadata fallback locale",
        );
        let save_label = t(
            locale.as_deref(),
            "page_builder.translations.localizedMetadataSave",
            "Save localized metadata",
        );
        let page_fallback = t(
            locale.as_deref(),
            "page_builder.ssrInspector.pageFallback",
            "Page",
        );
        let pages = runtime
            .controller
            .with(|controller| controller.editor().document().project.pages.clone());

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-localized-metadata="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{help}</p>
                </div>
                <div class="space-y-2">
                    {pages.into_iter().enumerate().map(|(index, page)| {
                        let page_id = page.id.clone().unwrap_or_else(|| format!("page-{index}"));
                        let values_json = localized_metadata_json(&page);
                        let fallback_locale = localized_metadata_fallback(&page);
                        let page_label = page_label(&page, index, &page_fallback);
                        view! {
                            <details class="rounded border border-border p-2">
                                <summary class="cursor-pointer text-xs font-semibold">{page_label}</summary>
                                <form
                                    class="mt-3 grid gap-2"
                                    data-fly-intent-form="upsert_localized_page_metadata"
                                >
                                    <input type="hidden" name="page_id" value=page_id/>
                                    <label class="grid gap-1 text-xs">
                                        <span class="font-medium">{values_label.clone()}</span>
                                        <textarea
                                            name="metadata_json"
                                            required
                                            class="min-h-44 rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                                            placeholder=values_placeholder.clone()
                                            spellcheck="false"
                                        >{values_json}</textarea>
                                    </label>
                                    <label class="grid gap-1 text-xs">
                                        <span class="font-medium">{fallback_label.clone()}</span>
                                        <input
                                            name="fallback_locale"
                                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                                            value=fallback_locale
                                            autocomplete="off"
                                            spellcheck="false"
                                        />
                                    </label>
                                    <button
                                        type="submit"
                                        class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                                    >{save_label.clone()}</button>
                                </form>
                            </details>
                        }
                    }).collect_view()}
                </div>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-localized-metadata="disabled"></span> }.into_any()
    }
}

fn normalize_localized_metadata_values(
    field: &str,
    value: Value,
) -> Result<Map<String, Value>, String> {
    let values = value
        .as_object()
        .cloned()
        .ok_or_else(|| format!("localized metadata field `{field}` must be a locale object"))?;
    let mut normalized = Map::new();
    for (locale, value) in values {
        let locale = normalize_locale_tag(&locale).ok_or_else(|| {
            format!("localized metadata field `{field}` locale `{locale}` is invalid")
        })?;
        let text = value.as_str().ok_or_else(|| {
            format!("localized metadata field `{field}` locale `{locale}` must be a string")
        })?;
        let text = if field == "slug" {
            normalize_slug(text.to_string())
        } else {
            text.trim().to_string()
        };
        if text.is_empty() {
            return Err(format!(
                "localized metadata field `{field}` locale `{locale}` must not be empty"
            ));
        }
        normalized.insert(locale, Value::String(text));
    }
    if normalized.is_empty() {
        return Err(format!(
            "localized metadata field `{field}` must contain at least one locale"
        ));
    }
    Ok(normalized)
}

fn localized_metadata_json(page: &ProjectPage) -> String {
    let mut localized = Map::new();
    let metadata = page
        .extensions
        .get(FLY_PAGE_METADATA_FIELD)
        .and_then(Value::as_object);
    if let Some(metadata) = metadata {
        for field in LOCALIZABLE_METADATA_FIELDS {
            let Some(values) = metadata
                .get(*field)
                .and_then(Value::as_object)
                .and_then(|wrapper| wrapper.get(LOCALIZED_VALUES_FIELD))
                .and_then(Value::as_object)
            else {
                continue;
            };
            localized.insert((*field).to_string(), Value::Object(values.clone()));
        }
    }
    serde_json::to_string_pretty(&Value::Object(localized)).unwrap_or_else(|_| "{}".to_string())
}

fn localized_metadata_fallback(page: &ProjectPage) -> String {
    let Some(metadata) = page
        .extensions
        .get(FLY_PAGE_METADATA_FIELD)
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    for field in LOCALIZABLE_METADATA_FIELDS {
        if let Some(fallback) = metadata
            .get(*field)
            .and_then(Value::as_object)
            .and_then(|wrapper| wrapper.get(LOCALIZED_FALLBACK_FIELD))
            .and_then(Value::as_str)
        {
            return fallback.to_string();
        }
    }
    String::new()
}

fn page_label(page: &ProjectPage, index: usize, fallback: &str) -> String {
    page.extensions
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| page.id.as_deref())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{fallback} {}", index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{GrapesJsCodec, PageMetadata, materialize_localized_page_metadata};
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "flyPageMeta": {
                        "providerFuture": { "enabled": true },
                        "title": "Home"
                    },
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn localized_metadata_form_preserves_unknown_fields_and_normalizes_slug() {
        let mut controller = controller();
        let intent = controller
            .ssr_localized_page_metadata_intent(SsrLocalizedPageMetadataRequest {
                page_id: "home".to_string(),
                metadata_json: json!({
                    "title": { "en": "Home", "ru": "Главная" },
                    "slug": { "en": "Hello World", "ru": "Привет Мир" }
                })
                .to_string(),
                fallback_locale: "en".to_string(),
            })
            .expect("localized metadata intent");
        controller
            .dispatch(intent)
            .expect("localized metadata patch");
        let metadata =
            &controller.editor().document().project.pages[0].extensions[FLY_PAGE_METADATA_FIELD];
        assert_eq!(metadata["providerFuture"]["enabled"], true);
        assert_eq!(
            metadata["slug"][LOCALIZED_VALUES_FIELD]["en"],
            "hello-world"
        );
        assert_eq!(metadata["slug"][LOCALIZED_VALUES_FIELD]["ru"], "привет-мир");
        let materialized = materialize_localized_page_metadata(
            controller.editor().document(),
            &json!({ "$locale": "ru" }),
        );
        let resolved = PageMetadata::from_page(&materialized.document.project.pages[0]);
        assert_eq!(resolved.title.as_deref(), Some("Главная"));
        assert_eq!(resolved.slug.as_deref(), Some("привет-мир"));
    }

    #[test]
    fn unsupported_fields_and_missing_fallback_values_are_rejected() {
        let controller = controller();
        assert!(
            controller
                .ssr_localized_page_metadata_intent(SsrLocalizedPageMetadataRequest {
                    page_id: "home".to_string(),
                    metadata_json: json!({ "unknown": { "en": "value" } }).to_string(),
                    fallback_locale: String::new(),
                })
                .is_err()
        );
        assert!(
            controller
                .ssr_localized_page_metadata_intent(SsrLocalizedPageMetadataRequest {
                    page_id: "home".to_string(),
                    metadata_json: json!({ "title": { "ru": "Главная" } }).to_string(),
                    fallback_locale: "en".to_string(),
                })
                .is_err()
        );
    }

    #[test]
    fn panel_round_trip_extracts_only_localizable_wrappers() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "flyPageMeta": {
                    "title": {
                        "$localized": { "en": "Home", "ru": "Главная" },
                        "$fallback": "en"
                    },
                    "providerFuture": true
                },
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("document");
        let page = &document.project.pages[0];
        assert_eq!(
            serde_json::from_str::<Value>(&localized_metadata_json(page)).unwrap(),
            json!({ "title": { "en": "Home", "ru": "Главная" } })
        );
        assert_eq!(localized_metadata_fallback(page), "en");
    }
}
