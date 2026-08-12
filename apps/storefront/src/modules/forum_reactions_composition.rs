use leptos::prelude::*;
use rustok_forum_storefront::{
    ForumView, fetch_storefront_reply_current_revision, fetch_storefront_topic_current_revision,
};
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
    let reply_id = explicit_forum_reply_id(&route, topic_id.as_ref());
    let topic_reaction_id = reply_id.is_none().then_some(topic_id).flatten();
    let subject_topic_id = topic_reaction_id;
    let subject_reply_id = reply_id;
    let topic_locale = route.locale.clone();
    let reply_locale = route.locale.clone();

    let topic_revision_resource = Resource::new_blocking(
        move || {
            (
                reactions_enabled.get(),
                topic_reaction_id,
                topic_locale.clone(),
            )
        },
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

    let reply_revision_resource = Resource::new_blocking(
        move || (reactions_enabled.get(), reply_id, reply_locale.clone()),
        |(enabled, reply_id, locale)| async move {
            if !enabled {
                return Ok(None);
            }
            let Some(reply_id) = reply_id else {
                return Ok(None);
            };
            fetch_storefront_reply_current_revision(reply_id.to_string(), locale)
                .await
                .map_err(|_| ())
        },
    );

    view! {
        <div class="space-y-4">
            <ForumView />
            <Suspense fallback=|| ()>
                {move || {
                    let topic_revision_resource = topic_revision_resource;
                    let topic_id = subject_topic_id;
                    Suspend::new(async move {
                        let Some(topic_id) = topic_id else {
                            return ().into_any();
                        };
                        let Ok(Some(revision)) = topic_revision_resource.await else {
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
            <Suspense fallback=|| ()>
                {move || {
                    let reply_revision_resource = reply_revision_resource;
                    let reply_id = subject_reply_id;
                    Suspend::new(async move {
                        let Some(reply_id) = reply_id else {
                            return ().into_any();
                        };
                        let Ok(Some(revision)) = reply_revision_resource.await else {
                            return ().into_any();
                        };
                        let Ok(subject) = ReactionSubjectUiRef::new(
                            "forum",
                            "reply",
                            reply_id,
                            revision,
                        ) else {
                            return ().into_any();
                        };

                        view! {
                            <section
                                class="rounded-[1.5rem] border border-border bg-card p-5 shadow-sm"
                                data-storefront-composition="forum-reply-reactions"
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
    parse_non_nil_uuid(route.query_value("topic")?)
}

fn explicit_forum_reply_id(route: &UiRouteContext, topic_id: Option<&Uuid>) -> Option<Uuid> {
    if route.route_segment.as_deref() != Some(FORUM_ROUTE_SEGMENT) || topic_id.is_none() {
        return None;
    }
    parse_non_nil_uuid(route.query_value("reply")?)
}

fn parse_non_nil_uuid(value: &str) -> Option<Uuid> {
    let value = value.trim();
    let id = Uuid::parse_str(value).ok()?;
    (!id.is_nil()).then_some(id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn forum_route(topic: Option<&str>, reply: Option<&str>) -> UiRouteContext {
        let mut query = BTreeMap::new();
        if let Some(topic) = topic {
            query.insert("topic".to_string(), topic.to_string());
        }
        if let Some(reply) = reply {
            query.insert("reply".to_string(), reply.to_string());
        }
        UiRouteContext {
            route_segment: Some("forum".to_string()),
            query,
            ..Default::default()
        }
    }

    #[test]
    fn explicit_topic_is_scoped_to_forum_module_route() {
        let topic_id = "12345678-9abc-4def-8123-456789abcdef";
        let forum_route = forum_route(Some(topic_id), None);
        assert_eq!(
            explicit_forum_topic_id(&forum_route).map(|id| id.to_string()),
            Some(topic_id.to_string())
        );

        let home_slot = UiRouteContext {
            route_segment: None,
            query: forum_route.query,
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
            assert!(explicit_forum_topic_id(&forum_route(topic, None)).is_none());
        }
    }

    #[test]
    fn explicit_reply_requires_forum_topic_context_and_non_nil_identity() {
        let topic_id = "12345678-9abc-4def-8123-456789abcdef";
        let reply_id = "87654321-cba9-4fed-8123-fedcba987654";
        let route = forum_route(Some(topic_id), Some(reply_id));
        let topic = explicit_forum_topic_id(&route);
        assert_eq!(
            explicit_forum_reply_id(&route, topic.as_ref()).map(|id| id.to_string()),
            Some(reply_id.to_string())
        );

        let reply_without_topic = forum_route(None, Some(reply_id));
        assert!(explicit_forum_reply_id(&reply_without_topic, None).is_none());
        let invalid_reply = forum_route(Some(topic_id), Some("not-a-uuid"));
        let topic = explicit_forum_topic_id(&invalid_reply);
        assert!(explicit_forum_reply_id(&invalid_reply, topic.as_ref()).is_none());
        let nil_reply = forum_route(Some(topic_id), Some("00000000-0000-0000-0000-000000000000"));
        let topic = explicit_forum_topic_id(&nil_reply);
        assert!(explicit_forum_reply_id(&nil_reply, topic.as_ref()).is_none());
    }
}
