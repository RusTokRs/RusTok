use leptos::prelude::*;
use rustok_forum_storefront::{ForumView, fetch_storefront_topic_current_revision};
use rustok_reactions_storefront::{ReactionBar, ReactionSubjectUiRef};
use rustok_ui_core::UiRouteContext;
use uuid::Uuid;

use crate::shared::context::enabled_modules::use_is_module_enabled;

#[component]
pub fn ForumStorefrontComposition() -> impl IntoView {
    let reactions_enabled = use_is_module_enabled("reactions");
    let route = use_context::<UiRouteContext>().unwrap_or_default();
    let topic_id = route
        .query
        .get("topic")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let locale = route.locale.clone();

    let subject_resource = Resource::new_blocking(
        move || (reactions_enabled.get(), topic_id.clone(), locale.clone()),
        |(enabled, topic_id, locale)| async move {
            if !enabled {
                return Ok(None);
            }
            let Some(topic_id) = topic_id else {
                return Ok(None);
            };
            load_forum_topic_reaction_subject(topic_id.as_str(), locale).await
        },
    );

    view! {
        <div class="space-y-4">
            <ForumView />
            <Suspense fallback=|| view! { <span class="hidden"></span> }>
                {move || {
                    let subject_resource = subject_resource;
                    Suspend::new(async move {
                        match subject_resource.await {
                            Ok(Some(subject)) => view! {
                                <section
                                    class="rounded-[1.5rem] border border-border bg-card p-5 shadow-sm"
                                    data-storefront-composition="forum-topic-reactions"
                                >
                                    <ReactionBar subject />
                                </section>
                            }
                            .into_any(),
                            Ok(None) => view! { <span class="hidden"></span> }.into_any(),
                            Err(_) => view! {
                                <p
                                    class="rounded-xl border border-border bg-muted/30 px-4 py-3 text-xs text-muted-foreground"
                                    role="status"
                                    data-storefront-composition="forum-topic-reactions-unavailable"
                                >
                                    "Reactions are temporarily unavailable for this topic."
                                </p>
                            }
                            .into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

async fn load_forum_topic_reaction_subject(
    topic_id: &str,
    locale: Option<String>,
) -> Result<Option<ReactionSubjectUiRef>, String> {
    let topic_id = match Uuid::parse_str(topic_id) {
        Ok(topic_id) if !topic_id.is_nil() => topic_id,
        _ => return Ok(None),
    };
    let Some(revision) = fetch_storefront_topic_current_revision(topic_id.to_string(), locale)
        .await
        .map_err(|_| "Forum topic reaction owner revision is unavailable".to_string())?
    else {
        return Ok(None);
    };

    ReactionSubjectUiRef::new("forum", "topic", topic_id, revision)
        .map(Some)
        .map_err(|_| "Forum topic reaction owner revision is invalid".to_string())
}
