use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
#[cfg(not(target_arch = "wasm32"))]
use fly::TranslationCatalog;
#[cfg(not(target_arch = "wasm32"))]
use fly::TranslationEntry;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;

#[component]
pub fn SsrTranslationsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.translations.title",
            "Project translations",
        );
        let description = t(
            locale.as_deref(),
            "page_builder.translations.description",
            "Translations are stored losslessly in the canonical project and exposed to bindings under translations.<id>.",
        );
        let empty = t(
            locale.as_deref(),
            "page_builder.translations.empty",
            "No project translations have been defined.",
        );
        let create_title = t(
            locale.as_deref(),
            "page_builder.translations.createTitle",
            "Create or update translation",
        );
        let id_label = t(
            locale.as_deref(),
            "page_builder.translations.idLabel",
            "Translation id",
        );
        let id_placeholder = t(
            locale.as_deref(),
            "page_builder.translations.idPlaceholder",
            "hero-title",
        );
        let values_label = t(
            locale.as_deref(),
            "page_builder.translations.valuesLabel",
            "Locale values JSON",
        );
        let values_help = t(
            locale.as_deref(),
            "page_builder.translations.valuesHelp",
            "Use a JSON object keyed by locale, for example en, ru, or de-DE.",
        );
        let values_placeholder = t(
            locale.as_deref(),
            "page_builder.translations.valuesPlaceholder",
            "{\n  \"en\": \"Welcome\",\n  \"ru\": \"Добро пожаловать\"\n}",
        );
        let fallback_label = t(
            locale.as_deref(),
            "page_builder.translations.fallbackLabel",
            "Entry fallback locale",
        );
        let fallback_placeholder = t(
            locale.as_deref(),
            "page_builder.translations.fallbackPlaceholder",
            "en",
        );
        let save_label = t(
            locale.as_deref(),
            "page_builder.translations.save",
            "Save translation",
        );
        let existing_label = t(
            locale.as_deref(),
            "page_builder.translations.existing",
            "Existing translations",
        );
        let locales_label = t(
            locale.as_deref(),
            "page_builder.translations.locales",
            "Locales",
        );
        let fallback_short = t(
            locale.as_deref(),
            "page_builder.translations.fallback",
            "Fallback",
        );
        let bind_title = t(
            locale.as_deref(),
            "page_builder.translations.bindTitle",
            "Bind selected component",
        );
        let bind_help = t(
            locale.as_deref(),
            "page_builder.translations.bindHelp",
            "Select a component on the canvas, then bind this translation to one field, attribute, or style property.",
        );
        let bind_kind = t(
            locale.as_deref(),
            "page_builder.translations.bindKind",
            "Binding target",
        );
        let bind_name = t(
            locale.as_deref(),
            "page_builder.translations.bindName",
            "Target name",
        );
        let bind_name_placeholder = t(
            locale.as_deref(),
            "page_builder.translations.bindNamePlaceholder",
            "content, aria-label, color...",
        );
        let bind_label = t(
            locale.as_deref(),
            "page_builder.translations.bind",
            "Save and bind translation",
        );
        let remove_label = t(
            locale.as_deref(),
            "page_builder.translations.remove",
            "Remove translation and bindings",
        );
        let field_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.fieldKind",
            "Field",
        );
        let attribute_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.attributeKind",
            "Attribute",
        );
        let style_kind = t(
            locale.as_deref(),
            "page_builder.ssrInspector.inlineStyleKind",
            "Inline style",
        );
        let catalog = runtime
            .controller
            .with(|controller| TranslationCatalog::from_document(controller.editor().document()));

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-translations="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{description}</p>
                </div>

                <details class="rounded border border-border p-2" open>
                    <summary class="cursor-pointer text-xs font-semibold">{create_title}</summary>
                    <form class="mt-3 grid gap-2" data-fly-intent-form="upsert_translation">
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{id_label.clone()}</span>
                            <input
                                name="translation_id"
                                required
                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                placeholder=id_placeholder.clone()
                                autocomplete="off"
                                spellcheck="false"
                            />
                        </label>
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{values_label.clone()}</span>
                            <textarea
                                name="values_json"
                                required
                                class="min-h-36 rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                                placeholder=values_placeholder.clone()
                                spellcheck="false"
                            ></textarea>
                            <span class="text-muted-foreground">{values_help.clone()}</span>
                        </label>
                        <label class="grid gap-1 text-xs">
                            <span class="font-medium">{fallback_label.clone()}</span>
                            <input
                                name="fallback_locale"
                                class="rounded border border-input bg-background px-2 py-1 text-xs"
                                placeholder=fallback_placeholder.clone()
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

                <div class="space-y-2">
                    <h3 class="text-xs font-semibold">{existing_label}</h3>
                    {if catalog.entries.is_empty() {
                        view! {
                            <p class="rounded border border-dashed border-border px-3 py-4 text-xs text-muted-foreground">
                                {empty}
                            </p>
                        }.into_any()
                    } else {
                        catalog.entries.into_iter().map(|entry| {
                            translation_entry_view(
                                entry,
                                id_label.clone(),
                                values_label.clone(),
                                values_help.clone(),
                                fallback_label.clone(),
                                save_label.clone(),
                                locales_label.clone(),
                                fallback_short.clone(),
                                bind_title.clone(),
                                bind_help.clone(),
                                bind_kind.clone(),
                                bind_name.clone(),
                                bind_name_placeholder.clone(),
                                bind_label.clone(),
                                remove_label.clone(),
                                field_kind.clone(),
                                attribute_kind.clone(),
                                style_kind.clone(),
                            )
                        }).collect_view().into_any()
                    }}
                </div>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-translations="disabled"></span> }.into_any()
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
fn translation_entry_view(
    entry: TranslationEntry,
    id_label: String,
    values_label: String,
    values_help: String,
    fallback_label: String,
    save_label: String,
    locales_label: String,
    fallback_short: String,
    bind_title: String,
    bind_help: String,
    bind_kind: String,
    bind_name: String,
    bind_name_placeholder: String,
    bind_label: String,
    remove_label: String,
    field_kind: String,
    attribute_kind: String,
    style_kind: String,
) -> impl IntoView {
    let values_json = serde_json::to_string_pretty(&Value::Object(entry.values.clone()))
        .unwrap_or_else(|_| "{}".to_string());
    let locales = entry.values.keys().cloned().collect::<Vec<_>>().join(", ");
    let fallback = entry.fallback_locale.clone().unwrap_or_default();
    let edit_id = entry.id.clone();
    let edit_values = values_json.clone();
    let edit_fallback = fallback.clone();
    let bind_id = entry.id.clone();
    let bind_values = values_json.clone();
    let bind_fallback = fallback.clone();
    let remove_id = entry.id.clone();

    view! {
        <details class="rounded border border-border p-2" data-fly-translation-id=entry.id.clone()>
            <summary class="cursor-pointer text-xs font-semibold">
                {entry.id.clone()}
                <span class="ml-2 font-normal text-muted-foreground">
                    {format!("{locales_label}: {locales}")}
                    {(!fallback.is_empty()).then(|| format!(" · {fallback_short}: {fallback}"))}
                </span>
            </summary>
            <div class="mt-3 grid gap-3">
                <form class="grid gap-2" data-fly-intent-form="upsert_translation">
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{id_label.clone()}</span>
                        <input
                            name="translation_id"
                            required
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            value=edit_id
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{values_label.clone()}</span>
                        <textarea
                            name="values_json"
                            required
                            class="min-h-36 rounded border border-input bg-background px-2 py-1 font-mono text-xs"
                            spellcheck="false"
                        >{edit_values}</textarea>
                        <span class="text-muted-foreground">{values_help}</span>
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{fallback_label.clone()}</span>
                        <input
                            name="fallback_locale"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            value=edit_fallback
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">
                        {save_label}
                    </button>
                </form>

                <form class="grid gap-2 border-t border-border pt-3" data-fly-intent-form="upsert_translation">
                    <strong class="text-xs">{bind_title}</strong>
                    <p class="text-xs text-muted-foreground">{bind_help}</p>
                    <input type="hidden" name="translation_id" value=bind_id/>
                    <input type="hidden" name="values_json" value=bind_values/>
                    <input type="hidden" name="fallback_locale" value=bind_fallback/>
                    <input
                        type="hidden"
                        name="component_id"
                        data-fly-selected-component-input="true"
                        value=""
                    />
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{bind_kind}</span>
                        <select name="bind_kind" class="rounded border border-input bg-background px-2 py-1 text-xs">
                            <option value="field">{field_kind}</option>
                            <option value="attribute">{attribute_kind}</option>
                            <option value="style">{style_kind}</option>
                        </select>
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{bind_name}</span>
                        <input
                            name="bind_name"
                            required
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder=bind_name_placeholder
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <button type="submit" class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary">
                        {bind_label}
                    </button>
                </form>

                <form class="border-t border-border pt-3" data-fly-intent-form="remove_translation">
                    <input type="hidden" name="translation_id" value=remove_id/>
                    <button type="submit" class="rounded border border-destructive/40 px-2 py-1 text-xs text-destructive">
                        {remove_label}
                    </button>
                </form>
            </div>
        </details>
    }
}
