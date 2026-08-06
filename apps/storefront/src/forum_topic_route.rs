use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use rustok_forum_storefront::{
    StorefrontForumTopicRouteDisposition, StorefrontForumTopicRouteResolution,
};
use rustok_web::CspNonce;

use super::{private_permanent_redirect, private_status_response, render_module_page_with_nonce};

const FORUM_ROUTE_SEGMENT: &str = "forum";

#[derive(Clone, Debug, Eq, PartialEq)]
enum ForumTopicHostAction {
    Render {
        topic_id: String,
        canonical_path: String,
    },
    Redirect(String),
}

pub(crate) async fn render_forum_topic_route_response(
    locale_path_prefix: String,
    effective_locale: String,
    short_id: String,
    slug: String,
    mut query_params: HashMap<String, String>,
    csp_nonce: Option<&CspNonce>,
) -> Response {
    let requested_path = format!("/{locale_path_prefix}/forum/t/{short_id}/{slug}");
    let resolution = match rustok_forum_storefront::resolve_storefront_topic_route(
        locale_path_prefix.clone(),
        short_id,
        slug,
    )
    .await
    {
        Ok(Some(resolution)) => resolution,
        Ok(None) => {
            return private_status_response(StatusCode::NOT_FOUND, "Forum topic route not found");
        }
        Err(error) => {
            eprintln!("failed to resolve Forum storefront topic route: {error}");
            return private_status_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Forum topic route resolution is temporarily unavailable",
            );
        }
    };

    match forum_topic_host_action(requested_path.as_str(), &resolution) {
        ForumTopicHostAction::Redirect(location) => private_permanent_redirect(location.as_str()),
        ForumTopicHostAction::Render {
            topic_id,
            canonical_path: _,
        } => {
            query_params.insert("topic".to_string(), topic_id);
            Html(
                render_module_page_with_nonce(
                    effective_locale.as_str(),
                    FORUM_ROUTE_SEGMENT,
                    query_params,
                    None,
                    csp_nonce,
                    None,
                )
                .await,
            )
            .into_response()
        }
    }
}

fn forum_topic_host_action(
    requested_path: &str,
    resolution: &StorefrontForumTopicRouteResolution,
) -> ForumTopicHostAction {
    let canonical_path = resolution.canonical.path.clone();
    if resolution.disposition == StorefrontForumTopicRouteDisposition::Redirect
        || requested_path != canonical_path
    {
        ForumTopicHostAction::Redirect(canonical_path)
    } else {
        ForumTopicHostAction::Render {
            topic_id: resolution.canonical.topic_id.clone(),
            canonical_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_forum_storefront::StorefrontForumTopicRouteDescriptor;

    fn resolution(
        disposition: StorefrontForumTopicRouteDisposition,
    ) -> StorefrontForumTopicRouteResolution {
        StorefrontForumTopicRouteResolution {
            requested_locale: "en".to_string(),
            requested_short_id: "123456789abc".to_string(),
            requested_slug: "welcome".to_string(),
            disposition,
            canonical: StorefrontForumTopicRouteDescriptor {
                topic_id: "12345678-9abc-4def-8123-456789abcdef".to_string(),
                locale: "en".to_string(),
                short_id: "123456789abc".to_string(),
                slug: "welcome".to_string(),
                path: "/en/forum/t/123456789abc/welcome".to_string(),
            },
        }
    }

    #[test]
    fn canonical_exact_path_renders_owner_topic_identity() {
        assert_eq!(
            forum_topic_host_action(
                "/en/forum/t/123456789abc/welcome",
                &resolution(StorefrontForumTopicRouteDisposition::Canonical),
            ),
            ForumTopicHostAction::Render {
                topic_id: "12345678-9abc-4def-8123-456789abcdef".to_string(),
                canonical_path: "/en/forum/t/123456789abc/welcome".to_string(),
            }
        );
    }

    #[test]
    fn alias_and_noncanonical_raw_paths_redirect_to_owner_path() {
        for (requested_path, disposition) in [
            (
                "/en/forum/t/123456789abc/old-welcome",
                StorefrontForumTopicRouteDisposition::Redirect,
            ),
            (
                "/EN/forum/t/123456789ABC/welcome",
                StorefrontForumTopicRouteDisposition::Canonical,
            ),
        ] {
            assert_eq!(
                forum_topic_host_action(requested_path, &resolution(disposition)),
                ForumTopicHostAction::Redirect(
                    "/en/forum/t/123456789abc/welcome".to_string()
                )
            );
        }
    }
}
