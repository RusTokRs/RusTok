use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use rustok_forum_storefront::{
    StorefrontForumTopicRouteDisposition, StorefrontForumTopicRouteResolution,
};
use rustok_web::CspNonce;

use crate::shared::context::seo_page_context::fetch_seo_page_context;

use super::{private_permanent_redirect, private_status_response, render_module_page_with_nonce};

const FORUM_ROUTE_SEGMENT: &str = "forum";

#[derive(Clone, Debug, Eq, PartialEq)]
enum ForumTopicHostAction {
    Render {
        topic_id: String,
        canonical_path: String,
    },
    Redirect(String),
    Gone,
    Invalid,
}

pub(crate) async fn render_forum_topic_route_response(
    requested_path: String,
    locale_path_prefix: String,
    effective_locale: String,
    short_id: String,
    slug: String,
    mut query_params: HashMap<String, String>,
    csp_nonce: Option<&CspNonce>,
) -> Response {
    let resolution = match rustok_forum_storefront::resolve_storefront_topic_route(
        locale_path_prefix,
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
        ForumTopicHostAction::Gone => private_status_response(
            StatusCode::GONE,
            "This Forum topic route is no longer available",
        ),
        ForumTopicHostAction::Invalid => private_status_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Forum topic route resolution is temporarily unavailable",
        ),
        ForumTopicHostAction::Render {
            topic_id,
            canonical_path: _,
        } => {
            query_params.insert("topic".to_string(), topic_id);
            query_params.remove("category");
            let seo_context = match fetch_seo_page_context(
                effective_locale.as_str(),
                FORUM_ROUTE_SEGMENT,
                &query_params,
            )
            .await
            {
                Ok(context) => context,
                Err(error) => {
                    eprintln!("failed to resolve Forum topic SEO context: {error}");
                    None
                }
            };
            Html(
                render_module_page_with_nonce(
                    effective_locale.as_str(),
                    FORUM_ROUTE_SEGMENT,
                    query_params,
                    seo_context.as_ref(),
                    csp_nonce,
                    None,
                )
                .await,
            )
            .into_response()
        }
    }
}

fn safe_route_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment == segment.trim()
        && !matches!(segment, "." | "..")
        && !segment.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '/' | '\\' | '?' | '#' | '%')
        })
}

fn valid_topic_descriptor(
    canonical: &rustok_forum_storefront::StorefrontForumTopicRouteDescriptor,
) -> bool {
    let locale = canonical.locale.trim();
    let short_id = canonical.short_id.trim();
    let slug = canonical.slug.trim();
    let path = &canonical.path;

    !canonical.topic_id.trim().is_empty()
        && canonical.locale == locale
        && canonical.short_id == short_id
        && canonical.slug == slug
        && rustok_api::normalize_locale_tag(locale).as_deref() == Some(locale)
        && short_id.len() == 12
        && short_id
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && safe_route_segment(slug)
        && !path.starts_with("//")
        && !path.chars().any(char::is_control)
        && canonical.path == format!("/{locale}/forum/t/{short_id}/{slug}")
}

fn forum_topic_host_action(
    requested_path: &str,
    resolution: &StorefrontForumTopicRouteResolution,
) -> ForumTopicHostAction {
    match (resolution.disposition, resolution.canonical.as_ref()) {
        (StorefrontForumTopicRouteDisposition::Gone, None) => ForumTopicHostAction::Gone,
        (StorefrontForumTopicRouteDisposition::Gone, Some(_))
        | (StorefrontForumTopicRouteDisposition::Canonical, None)
        | (StorefrontForumTopicRouteDisposition::Redirect, None) => ForumTopicHostAction::Invalid,
        (StorefrontForumTopicRouteDisposition::Redirect, Some(canonical))
            if valid_topic_descriptor(canonical) =>
        {
            ForumTopicHostAction::Redirect(canonical.path.clone())
        }
        (StorefrontForumTopicRouteDisposition::Canonical, Some(canonical))
            if valid_topic_descriptor(canonical) && requested_path != canonical.path =>
        {
            ForumTopicHostAction::Redirect(canonical.path.clone())
        }
        (StorefrontForumTopicRouteDisposition::Canonical, Some(canonical))
            if valid_topic_descriptor(canonical) =>
        {
            ForumTopicHostAction::Render {
                topic_id: canonical.topic_id.clone(),
                canonical_path: canonical.path.clone(),
            }
        }
        _ => ForumTopicHostAction::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_forum_storefront::StorefrontForumTopicRouteDescriptor;

    fn descriptor() -> StorefrontForumTopicRouteDescriptor {
        StorefrontForumTopicRouteDescriptor {
            topic_id: "12345678-9abc-4def-8123-456789abcdef".to_string(),
            locale: "en".to_string(),
            short_id: "123456789abc".to_string(),
            slug: "welcome".to_string(),
            path: "/en/forum/t/123456789abc/welcome".to_string(),
        }
    }

    fn resolution(
        disposition: StorefrontForumTopicRouteDisposition,
        canonical: Option<StorefrontForumTopicRouteDescriptor>,
    ) -> StorefrontForumTopicRouteResolution {
        StorefrontForumTopicRouteResolution {
            requested_locale: "en".to_string(),
            requested_short_id: "123456789abc".to_string(),
            requested_slug: "welcome".to_string(),
            disposition,
            canonical,
        }
    }

    #[test]
    fn canonical_exact_path_renders_owner_topic_identity() {
        assert_eq!(
            forum_topic_host_action(
                "/en/forum/t/123456789abc/welcome",
                &resolution(
                    StorefrontForumTopicRouteDisposition::Canonical,
                    Some(descriptor()),
                ),
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
            (
                "/en/forum/t/123456789abc/%77elcome",
                StorefrontForumTopicRouteDisposition::Canonical,
            ),
        ] {
            assert_eq!(
                forum_topic_host_action(
                    requested_path,
                    &resolution(disposition, Some(descriptor())),
                ),
                ForumTopicHostAction::Redirect("/en/forum/t/123456789abc/welcome".to_string())
            );
        }
    }

    #[test]
    fn authorized_gone_has_terminal_host_action() {
        assert_eq!(
            forum_topic_host_action(
                "/en/forum/t/123456789abc/retired-topic",
                &resolution(StorefrontForumTopicRouteDisposition::Gone, None),
            ),
            ForumTopicHostAction::Gone
        );
    }

    #[test]
    fn malformed_transport_shapes_fail_closed() {
        let mut external = descriptor();
        external.path = "https://example.invalid/topic".to_string();
        let mut protocol_relative = descriptor();
        protocol_relative.path = "//example.invalid/topic".to_string();
        let mut header_injection = descriptor();
        header_injection.path = "/en/forum/t/123456789abc/welcome\r\nX-Test: injected".to_string();
        let mut missing_identity = descriptor();
        missing_identity.topic_id.clear();
        let mut internal_target = descriptor();
        internal_target.path = "/admin".to_string();
        let mut mismatched_slug = descriptor();
        mismatched_slug.path = "/en/forum/t/123456789abc/another-topic".to_string();
        let mut query_target = descriptor();
        query_target.path = "/en/forum/t/123456789abc/welcome?preview=true".to_string();
        let mut invalid_locale = descriptor();
        invalid_locale.locale = "EN".to_string();
        invalid_locale.path = "/EN/forum/t/123456789abc/welcome".to_string();
        let mut uppercase_short_id = descriptor();
        uppercase_short_id.short_id = "123456789ABC".to_string();
        uppercase_short_id.path = "/en/forum/t/123456789ABC/welcome".to_string();
        let mut short_short_id = descriptor();
        short_short_id.short_id = "1234".to_string();
        short_short_id.path = "/en/forum/t/1234/welcome".to_string();
        let mut invalid_slug = descriptor();
        invalid_slug.slug = "../admin".to_string();
        invalid_slug.path = "/en/forum/t/123456789abc/../admin".to_string();

        for resolution in [
            resolution(
                StorefrontForumTopicRouteDisposition::Gone,
                Some(descriptor()),
            ),
            resolution(StorefrontForumTopicRouteDisposition::Canonical, None),
            resolution(StorefrontForumTopicRouteDisposition::Redirect, None),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(external),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(protocol_relative),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(header_injection),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Canonical,
                Some(missing_identity),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(internal_target),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(mismatched_slug),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(query_target),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(invalid_locale),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(uppercase_short_id),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(short_short_id),
            ),
            resolution(
                StorefrontForumTopicRouteDisposition::Redirect,
                Some(invalid_slug),
            ),
        ] {
            assert_eq!(
                forum_topic_host_action("/en/forum/t/123456789abc/welcome", &resolution,),
                ForumTopicHostAction::Invalid
            );
        }
    }
}
