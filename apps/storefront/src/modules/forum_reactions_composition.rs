use leptos::prelude::*;
use rustok_forum_storefront::{ForumView, fetch_storefront_topic_current_revision};
use rustok_reactions_storefront::{ReactionBar, ReactionSubjectUiRef};
use rustok_ui_core::UiRouteContext;
use uuid::Uuid;

use crate::shared::context::enabled_modules::use_is_module_enabled;

const FORUM_ROUTE_SEGMENT: &str = "forum";

#[component]
pub fn ForumStorefrontComposition() -> impl IntoView {
    let reactions_enabled = use_is_module_enabled("reactions");
    let route = use_context::<UiRouteContext>().unwrap_or_default();
    let topic_id = explicit_forum_topic_id(&route);
    let subject_topic_id = topic_id.clone();
    let locale = route.locale.clone();

    let revision_resource = Resource::new_blocking(
        move || (reactions_enabled.get(), topic_id.clone(), locale.clone()),
        |(enabled, topic_id, locale)| async move {
            if !enabled {
                return Ok(None);
            }
            let Some(topic_id) = topic_id else {
                return Ok(None);
            };
            fetch_storefront_topic_current_revision(topic_id.to_string(), locale)
                .await
                .map_err(|_| ())
        },
    );

    view! {
        <div class="space-y-4">
            <ForumView />
            <Suspense fallback=|| ()>
                {move || {
                    let revision_resource = revision_resource;
                    let topic_id = subject_topic_id.clone();
                    Suspend::new(async move {
                        let Some(topic_id) = topic_id else {
                            return ().into_any();
                        };
                        let Ok(Some(revision)) = revision_resource.await else {
                            return ().into_any();
                        };
                        let Ok(subject) = ReactionSubjectUiRef::new(
                            "forum",
                            "topic",
                            topic_id,
                            revision,
                        ) else {
                            return ().into_any();
                        };

                        view! {
                            <section
                                class="rounded-[1.5rem] border border-border bg-card p-5 shadow-sm"
                                data-storefront-composition="forum-topic-reactions"
                            >
                                <ReactionBar subject />
                            </section>
                        }
                        .into_any()
                    })
                }}
            </Suspense>
        </div>
    }
}

fn explicit_forum_topic_id(route: &UiRouteContext) -> Option<Uuid> {
    if route.route_segment.as_deref() != Some(FORUM_ROUTE_SEGMENT) {
        return None;
    }
    let value = route.query_value("topic")?.trim();
    let topic_id = Uuid::parse_str(value).ok()?;
    (!topic_id.is_nil()).then_some(topic_id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn explicit_topic_is_scoped_to_forum_module_route() {
        let topic_id = "12345678-9abc-4def-8123-456789abcdef";
        let mut query = BTreeMap::new();
        query.insert("topic".to_string(), topic_id.to_string());
        let forum_route = UiRouteContext {
            route_segment: Some("forum".to_string()),
            query: query.clone(),
            ..Default::default()
        };
        assert_eq!(
            explicit_forum_topic_id(&forum_route).map(|id| id.to_string()),
            Some(topic_id.to_string())
        );

        let home_slot = UiRouteContext {
            route_segment: None,
            query,
            ..Default::default()
        };
        assert!(explicit_forum_topic_id(&home_slot).is_none());
    }

    #[test]
    fn explicit_topic_rejects_missing_invalid_and_nil_identity() {
        for topic in [
            None,
            Some("not-a-uuid"),
            Some("00000000-0000-0000-0000-000000000000"),
        ] {
            let mut query = BTreeMap::new();
            if let Some(topic) = topic {
                query.insert("topic".to_string(), topic.to_string());
            }
            let route = UiRouteContext {
                route_segment: Some("forum".to_string()),
                query,
                ..Default::default()
            };
            assert!(explicit_forum_topic_id(&route).is_none());
        }
    }
}
