use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
#[cfg(not(target_arch = "wasm32"))]
use fly::{RUNTIME_FALLBACK_LOCALES_FIELD, RUNTIME_LOCALE_FIELD};
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;

#[component]
pub fn SsrLocalePanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let ui_locale = route_context.locale;
        let title = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.localeTitle",
            "Preview locale",
        );
        let help = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.localeHelp",
            "Localized runtime values resolve using the active locale, then the fallback chain. Locale settings are stored only in the server draft.",
        );
        let locale_label = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.localeLabel",
            "Active locale",
        );
        let locale_placeholder = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.localePlaceholder",
            "en-US",
        );
        let fallback_label = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.fallbackLocalesLabel",
            "Fallback locales",
        );
        let fallback_placeholder = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.fallbackLocalesPlaceholder",
            "en, de-DE",
        );
        let apply_locale = t(
            ui_locale.as_deref(),
            "page_builder.ssrInspector.applyLocale",
            "Apply preview locale",
        );
        let context = runtime.runtime_context.get_untracked();
        let active_locale = context
            .get(RUNTIME_LOCALE_FIELD)
            .or_else(|| context.get("locale"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let fallback_locales = context
            .get(RUNTIME_FALLBACK_LOCALES_FIELD)
            .or_else(|| context.get("fallback_locales"))
            .map(locale_list_text)
            .unwrap_or_default();

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-locale="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{help}</p>
                </div>
                <form class="grid gap-2" data-fly-intent-form="set_runtime_locale">
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{locale_label}</span>
                        <input
                            name="locale"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder=locale_placeholder
                            value=active_locale
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{fallback_label}</span>
                        <input
                            name="fallback_locales"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder=fallback_placeholder
                            value=fallback_locales
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <button
                        type="submit"
                        class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                    >
                        {apply_locale}
                    </button>
                </form>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-locale="disabled"></span> }.into_any()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_list_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fallback_locale_values_are_rendered_as_editable_text() {
        assert_eq!(locale_list_text(&json!(["ru", "en-US"])), "ru, en-US");
        assert_eq!(locale_list_text(&json!("en")), "en");
        assert_eq!(locale_list_text(&json!({})), "");
    }
}
