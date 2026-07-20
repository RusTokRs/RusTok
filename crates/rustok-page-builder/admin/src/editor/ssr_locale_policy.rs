use crate::AdminCanvasController;
use crate::editor::AdminEditorRuntime;
#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::t;
use fly::{EditorCommand, ProjectLocalePolicy, TranslationCommand, normalize_locale_tag};
use fly_ui::UiIntent;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rustok_ui_core::UiRouteContext;
use serde::{Deserialize, Serialize};
use serde_json::Map;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SsrLocalePolicyRequest {
    #[serde(default)]
    pub default_locale: String,
    #[serde(default)]
    pub supported_locales: String,
    #[serde(default)]
    pub required_locales: String,
    #[serde(default)]
    pub fallback_locales: String,
    #[serde(default)]
    pub enforce_required_locales: bool,
}

impl AdminCanvasController {
    pub fn ssr_locale_policy_intent(
        &self,
        request: SsrLocalePolicyRequest,
    ) -> Result<UiIntent, String> {
        let default_locale = optional_locale(&request.default_locale, "default locale")?;
        let supported_locales = parse_locale_list(&request.supported_locales, "supported locale")?;
        let required_locales = parse_locale_list(&request.required_locales, "required locale")?;
        let fallback_locales = parse_locale_list(&request.fallback_locales, "fallback locale")?;
        let policy = ProjectLocalePolicy {
            default_locale,
            supported_locales,
            required_locales,
            fallback_locales,
            enforce_required_locales: request.enforce_required_locales,
            extensions: ProjectLocalePolicy::from_document(self.editor().document())
                .map(|policy| policy.extensions)
                .unwrap_or_else(Map::new),
        };
        policy.normalized()?;
        Ok(UiIntent::execute(EditorCommand::Translation {
            command: TranslationCommand::SetLocalePolicy {
                policy: Box::new(policy),
            },
        }))
    }

    pub fn ssr_clear_locale_policy_intent(&self) -> Option<UiIntent> {
        ProjectLocalePolicy::from_document(self.editor().document()).map(|_| {
            UiIntent::execute(EditorCommand::Translation {
                command: TranslationCommand::ClearLocalePolicy,
            })
        })
    }
}

#[component]
pub fn SsrLocalePolicyPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let route_context = use_context::<UiRouteContext>().unwrap_or_default();
        let locale = route_context.locale;
        let title = t(
            locale.as_deref(),
            "page_builder.localePolicy.title",
            "Project locale policy",
        );
        let description = t(
            locale.as_deref(),
            "page_builder.localePolicy.description",
            "Define the canonical locale contract stored in the project. Preview locale remains draft-only.",
        );
        let default_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.defaultLabel",
            "Default locale",
        );
        let supported_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.supportedLabel",
            "Supported locales",
        );
        let required_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.requiredLabel",
            "Required locales",
        );
        let fallback_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.fallbackLabel",
            "Fallback locales",
        );
        let list_help = t(
            locale.as_deref(),
            "page_builder.localePolicy.listHelp",
            "Separate locale tags with commas. Tags are normalized before they are stored.",
        );
        let enforce_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.enforceLabel",
            "Block save and publish when required translations are missing",
        );
        let enforce_help = t(
            locale.as_deref(),
            "page_builder.localePolicy.enforceHelp",
            "Keep this disabled while translations are being prepared; missing required locales remain warnings.",
        );
        let save_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.save",
            "Save locale policy",
        );
        let clear_label = t(
            locale.as_deref(),
            "page_builder.localePolicy.clear",
            "Clear locale policy",
        );
        let policy = runtime.controller.with(|controller| {
            ProjectLocalePolicy::from_document(controller.editor().document()).unwrap_or_default()
        });
        let default_locale = policy.default_locale.unwrap_or_default();
        let supported_locales = policy.supported_locales.join(", ");
        let required_locales = policy.required_locales.join(", ");
        let fallback_locales = policy.fallback_locales.join(", ");
        let enforce_required_locales = policy.enforce_required_locales;

        view! {
            <section
                class="space-y-3 rounded-xl border border-border bg-card p-3"
                data-fly-ssr-locale-policy="true"
            >
                <div>
                    <h2 class="font-semibold">{title}</h2>
                    <p class="text-xs text-muted-foreground">{description}</p>
                </div>
                <form class="grid gap-2" data-fly-intent-form="set_locale_policy">
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{default_label}</span>
                        <input
                            name="default_locale"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="en-US"
                            value=default_locale
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{supported_label}</span>
                        <input
                            name="supported_locales"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="en, ru, de-DE"
                            value=supported_locales
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{required_label}</span>
                        <input
                            name="required_locales"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="en, ru"
                            value=required_locales
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </label>
                    <label class="grid gap-1 text-xs">
                        <span class="font-medium">{fallback_label}</span>
                        <input
                            name="fallback_locales"
                            class="rounded border border-input bg-background px-2 py-1 text-xs"
                            placeholder="en"
                            value=fallback_locales
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <span class="text-muted-foreground">{list_help}</span>
                    </label>
                    <label class="flex items-start gap-2 text-xs">
                        <input
                            type="checkbox"
                            name="enforce_required_locales"
                            value="true"
                            checked=enforce_required_locales
                        />
                        <span>
                            <strong class="block">{enforce_label}</strong>
                            <span class="text-muted-foreground">{enforce_help}</span>
                        </span>
                    </label>
                    <button
                        type="submit"
                        class="w-fit rounded border border-primary/40 px-2 py-1 text-xs text-primary"
                    >{save_label}</button>
                </form>
                <form data-fly-intent-form="clear_locale_policy">
                    <button
                        type="submit"
                        class="w-fit rounded border border-destructive/40 px-2 py-1 text-xs text-destructive"
                    >{clear_label}</button>
                </form>
            </section>
        }
        .into_any()
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = runtime;
        view! { <span hidden data-fly-ssr-locale-policy="disabled"></span> }.into_any()
    }
}

fn optional_locale(value: &str, label: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        normalize_locale_tag(value)
            .map(Some)
            .ok_or_else(|| format!("{label} `{value}` is invalid"))
    }
}

fn parse_locale_list(value: &str, label: &str) -> Result<Vec<String>, String> {
    let mut locales = Vec::new();
    for value in value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let locale =
            normalize_locale_tag(value).ok_or_else(|| format!("{label} `{value}` is invalid"))?;
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }
    Ok(locales)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AdminCanvasController;
    use fly::FLY_LOCALES_FIELD;
    use serde_json::json;

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    fn incomplete_translation_controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "flyTranslations": [{
                    "id": "hero",
                    "values": { "en": "Welcome" }
                }],
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn locale_lists_are_normalized_and_deduplicated() {
        assert_eq!(
            parse_locale_list(" RU_ru, en; EN\nde-DE ", "locale").unwrap(),
            vec!["ru-ru", "en", "de-de"]
        );
    }

    #[test]
    fn locale_policy_form_participates_in_editor_history() {
        let mut controller = controller();
        let intent = controller
            .ssr_locale_policy_intent(SsrLocalePolicyRequest {
                default_locale: "ru".to_string(),
                supported_locales: "ru, en".to_string(),
                required_locales: "ru, en".to_string(),
                fallback_locales: "en".to_string(),
                enforce_required_locales: false,
            })
            .expect("locale policy intent");
        controller.dispatch(intent).expect("locale policy command");
        assert_eq!(
            controller.editor().document().project.extensions[FLY_LOCALES_FIELD]["default_locale"],
            "ru"
        );
        controller
            .dispatch(UiIntent::Undo)
            .expect("undo locale policy");
        assert!(
            !controller
                .editor()
                .document()
                .project
                .extensions
                .contains_key(FLY_LOCALES_FIELD)
        );
    }

    #[test]
    fn clearing_missing_policy_is_an_idempotent_no_op() {
        let controller = controller();
        assert!(controller.ssr_clear_locale_policy_intent().is_none());
        assert_eq!(controller.editor().history().undo_len(), 0);
    }

    #[test]
    fn strict_policy_rolls_back_when_required_translation_is_missing() {
        let mut controller = incomplete_translation_controller();
        let before = controller.editor().document().hash();
        let intent = controller
            .ssr_locale_policy_intent(SsrLocalePolicyRequest {
                default_locale: "en".to_string(),
                supported_locales: "en, ru".to_string(),
                required_locales: "en, ru".to_string(),
                fallback_locales: "en".to_string(),
                enforce_required_locales: true,
            })
            .expect("strict locale policy intent");
        controller
            .dispatch(intent)
            .expect_err("missing required locale must block the transaction");
        assert_eq!(controller.editor().document().hash(), before);
        assert_eq!(controller.editor().history().undo_len(), 0);
        assert!(
            !controller
                .editor()
                .document()
                .project
                .extensions
                .contains_key(FLY_LOCALES_FIELD)
        );
    }
}
