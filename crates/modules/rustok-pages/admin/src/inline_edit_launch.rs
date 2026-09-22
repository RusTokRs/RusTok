use leptos::prelude::*;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};
use url::form_urlencoded::Serializer;
use uuid::Uuid;

use crate::access::pages_editor_capability_policy_for_role;
use crate::core::optional_ui_text;
use crate::transport;

const PAGES_AUTHORING_PATH: &str = "/modules/pages-authoring";
const MAX_LOCALE_LENGTH: usize = 64;
const SAME_ORIGIN_BUILD_FLAG: Option<&str> =
    option_env!("RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN");

fn same_origin_launch_enabled(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

pub(crate) fn authoring_href(page_id: &str, locale: &str) -> Option<String> {
    let page_id = Uuid::parse_str(page_id.trim()).ok()?;
    if page_id.is_nil() {
        return None;
    }

    let locale = optional_ui_text(locale)?;
    if locale.len() > MAX_LOCALE_LENGTH || locale.chars().any(char::is_control) {
        return None;
    }

    let canonical_page_id = page_id.hyphenated().to_string();
    let query = Serializer::new(String::new())
        .append_pair("page_id", canonical_page_id.as_str())
        .append_pair("lang", locale.as_str())
        .finish();
    Some(format!("{PAGES_AUTHORING_PATH}?{query}"))
}

#[component]
pub(crate) fn PagesInlineEditLaunch(selected_page: Signal<Option<String>>) -> impl IntoView {
    let current_user = use_current_user();
    let token = use_token();
    let tenant = use_tenant();
    let launch_resource = LocalResource::new(move || {
        let selected_page = selected_page.get();
        let user = current_user.get();
        let token = token.get();
        let tenant = tenant.get();
        async move {
            if !same_origin_launch_enabled(SAME_ORIGIN_BUILD_FLAG) {
                return Ok::<Option<String>, transport::TransportError>(None);
            }

            let can_edit = user
                .as_ref()
                .map(|user| {
                    pages_editor_capability_policy_for_role(Some(user.role.as_str()))
                        .evaluate_detailed()
                        .effective
                        .edit
                })
                .unwrap_or(false);
            if !can_edit {
                return Ok(None);
            }

            let Some(page_id) = selected_page
                .as_deref()
                .and_then(|value| Uuid::parse_str(value.trim()).ok())
                .filter(|page_id| !page_id.is_nil())
                .map(|page_id| page_id.hyphenated().to_string())
            else {
                return Ok(None);
            };
            let Some(page) = transport::fetch_page(token, tenant, page_id).await? else {
                return Ok(None);
            };
            if page.status.eq_ignore_ascii_case("published") {
                return Ok(None);
            }

            let Some(locale) = page
                .translation
                .as_ref()
                .map(|translation| translation.locale.as_str())
                .or_else(|| page.body.as_ref().map(|body| body.locale.as_str()))
            else {
                return Ok(None);
            };
            Ok(authoring_href(page.id.as_str(), locale))
        }
    });

    view! {
        {move || {
            launch_resource.get().and_then(|result| match result {
                Ok(Some(href)) => Some(view! {
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
                                    "Draft-only. Opens the exact-locale same-origin authoring route in a new tab and reuses the current direct browser session."
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
                }.into_any()),
                Ok(None) | Err(_) => None,
            })
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::{authoring_href, same_origin_launch_enabled};

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

    #[test]
    fn same_origin_build_acknowledgement_is_explicit() {
        assert!(same_origin_launch_enabled(Some("true")));
        assert!(same_origin_launch_enabled(Some(" TRUE ")));
        assert!(!same_origin_launch_enabled(Some("1")));
        assert!(!same_origin_launch_enabled(Some("false")));
        assert!(!same_origin_launch_enabled(None));
    }
}
