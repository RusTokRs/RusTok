use leptos::prelude::*;
use leptos_auth::hooks::use_current_user;
use url::form_urlencoded::Serializer;
use uuid::Uuid;

use crate::access::pages_editor_capability_policy_for_role;
use crate::core::optional_ui_text;

const PAGES_AUTHORING_PATH: &str = "/modules/pages-authoring";
const MAX_LOCALE_LENGTH: usize = 64;

pub(crate) fn authoring_href(page_id: &str, locale: &str) -> Option<String> {
    let page_id = Uuid::parse_str(page_id.trim()).ok()?;
    if page_id.is_nil() {
        return None;
    }

    let locale = optional_ui_text(locale)?;
    if locale.len() > MAX_LOCALE_LENGTH || locale.chars().any(char::is_control) {
        return None;
    }

    let query = Serializer::new(String::new())
        .append_pair("page_id", page_id.hyphenated().to_string().as_str())
        .append_pair("lang", locale.as_str())
        .finish();
    Some(format!("{PAGES_AUTHORING_PATH}?{query}"))
}

#[component]
pub(crate) fn PagesInlineEditLaunch(
    selected_page: Signal<Option<String>>,
    locale: String,
) -> impl IntoView {
    let current_user = use_current_user();

    view! {
        {move || {
            let can_edit = current_user
                .get()
                .as_ref()
                .map(|user| {
                    pages_editor_capability_policy_for_role(Some(user.role.as_str()))
                        .evaluate_detailed()
                        .effective
                        .edit
                })
                .unwrap_or(false);
            if !can_edit {
                return ().into_any();
            }

            let href = selected_page
                .get()
                .as_deref()
                .and_then(|page_id| authoring_href(page_id, locale.as_str()));
            let Some(href) = href else {
                return ().into_any();
            };

            view! {
                <section
                    class="rounded-2xl border border-primary/30 bg-primary/5 px-4 py-3 shadow-sm"
                    data-pages-inline-edit-launch="same-origin"
                >
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <div class="text-sm font-semibold text-card-foreground">
                                "Authenticated inline editor"
                            </div>
                            <p class="mt-1 text-xs text-muted-foreground">
                                "Draft-only. Opens the same-origin authoring route in a new tab and reuses the current direct browser session."
                            </p>
                        </div>
                        <a
                            class="rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:opacity-90"
                            href=href
                            target="_blank"
                            rel="noopener noreferrer"
                            aria-label="Open authenticated inline editor"
                        >
                            "Open inline editor"
                        </a>
                    </div>
                </section>
            }
            .into_any()
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::authoring_href;

    #[test]
    fn builds_only_the_fixed_same_origin_authoring_path() {
        assert_eq!(
            authoring_href("7d4c48cf-f10e-46cf-9755-fbb1d33a7217", "en-US").as_deref(),
            Some(
                "/modules/pages-authoring?page_id=7d4c48cf-f10e-46cf-9755-fbb1d33a7217&lang=en-US"
            )
        );
    }

    #[test]
    fn percent_encodes_locale_without_adding_credentials() {
        let href = authoring_href("7d4c48cf-f10e-46cf-9755-fbb1d33a7217", "pt BR")
            .expect("valid page and locale should produce a link");
        assert!(href.ends_with("&lang=pt+BR"));
        assert!(!href.contains("token"));
        assert!(!href.contains("proof"));
        assert!(!href.contains("authorization"));
    }

    #[test]
    fn rejects_invalid_or_nil_page_identity_and_unbounded_locale() {
        assert!(authoring_href("not-a-uuid", "en").is_none());
        assert!(authoring_href("00000000-0000-0000-0000-000000000000", "en").is_none());
        assert!(authoring_href("7d4c48cf-f10e-46cf-9755-fbb1d33a7217", "").is_none());
        assert!(
            authoring_href(
                "7d4c48cf-f10e-46cf-9755-fbb1d33a7217",
                "x".repeat(65).as_str()
            )
            .is_none()
        );
    }
}
