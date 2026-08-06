use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use rustok_forum_storefront::{
    StorefrontForumCategoryRouteDisposition, StorefrontForumCategoryRouteResolution,
};
use rustok_web::CspNonce;

use super::{private_permanent_redirect, private_status_response, render_module_page_with_nonce};

const FORUM_ROUTE_SEGMENT: &str = "forum";

#[derive(Clone, Debug, Eq, PartialEq)]
enum ForumCategoryHostAction {
    Render { category_id: String },
    Redirect(String),
    Invalid,
}

pub(crate) async fn render_forum_category_route_response(
    requested_path: String,
    locale_path_prefix: String,
    effective_locale: String,
    slug: String,
    mut query_params: HashMap<String, String>,
    csp_nonce: Option<&CspNonce>,
) -> Response {
    let resolution = match rustok_forum_storefront::resolve_storefront_category_route(
        locale_path_prefix,
        slug,
    )
    .await
    {
        Ok(Some(resolution)) => resolution,
        Ok(None) => {
            return private_status_response(
                StatusCode::NOT_FOUND,
                "Forum category route not found",
            );
        }
        Err(error) => {
            eprintln!("failed to resolve Forum storefront category route: {error}");
            return private_status_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Forum category route resolution is temporarily unavailable",
            );
        }
    };

    match forum_category_host_action(requested_path.as_str(), &resolution) {
        ForumCategoryHostAction::Redirect(location) => {
            private_permanent_redirect(location.as_str())
        }
        ForumCategoryHostAction::Invalid => private_status_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Forum category route resolution is temporarily unavailable",
        ),
        ForumCategoryHostAction::Render { category_id } => {
            query_params.insert("category".to_string(), category_id);
            query_params.remove("topic");
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

fn safe_owner_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.chars().any(char::is_control)
}

fn forum_category_host_action(
    requested_path: &str,
    resolution: &StorefrontForumCategoryRouteResolution,
) -> ForumCategoryHostAction {
    let canonical = &resolution.canonical;
    if canonical.category_id.trim().is_empty()
        || canonical.locale.trim().is_empty()
        || canonical.slug.trim().is_empty()
        || !safe_owner_path(canonical.path.as_str())
    {
        return ForumCategoryHostAction::Invalid;
    }

    match resolution.disposition {
        StorefrontForumCategoryRouteDisposition::Redirect => {
            ForumCategoryHostAction::Redirect(canonical.path.clone())
        }
        StorefrontForumCategoryRouteDisposition::Canonical
            if requested_path != canonical.path =>
        {
            ForumCategoryHostAction::Redirect(canonical.path.clone())
        }
        StorefrontForumCategoryRouteDisposition::Canonical => ForumCategoryHostAction::Render {
            category_id: canonical.category_id.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_forum_storefront::StorefrontForumCategoryRouteDescriptor;

    fn descriptor() -> StorefrontForumCategoryRouteDescriptor {
        StorefrontForumCategoryRouteDescriptor {
            category_id: "12345678-9abc-4def-8123-456789abcdef".to_string(),
            locale: "en".to_string(),
            slug: "general".to_string(),
            path: "/en/forum/c/general".to_string(),
        }
    }

    fn resolution(
        disposition: StorefrontForumCategoryRouteDisposition,
    ) -> StorefrontForumCategoryRouteResolution {
        StorefrontForumCategoryRouteResolution {
            requested_locale: "en".to_string(),
            requested_slug: "general".to_string(),
            disposition,
            canonical: descriptor(),
        }
    }

    #[test]
    fn canonical_exact_path_renders_owner_category_identity() {
        assert_eq!(
            forum_category_host_action(
                "/en/forum/c/general",
                &resolution(StorefrontForumCategoryRouteDisposition::Canonical),
            ),
            ForumCategoryHostAction::Render {
                category_id: "12345678-9abc-4def-8123-456789abcdef".to_string(),
            }
        );
    }

    #[test]
    fn alias_and_noncanonical_raw_paths_redirect_to_owner_path() {
        for (requested_path, disposition) in [
            (
                "/en/forum/c/old-general",
                StorefrontForumCategoryRouteDisposition::Redirect,
            ),
            (
                "/EN/forum/c/general",
                StorefrontForumCategoryRouteDisposition::Canonical,
            ),
            (
                "/en/forum/c/%67eneral",
                StorefrontForumCategoryRouteDisposition::Canonical,
            ),
        ] {
            assert_eq!(
                forum_category_host_action(requested_path, &resolution(disposition)),
                ForumCategoryHostAction::Redirect("/en/forum/c/general".to_string())
            );
        }
    }

    #[test]
    fn malformed_transport_shapes_fail_closed() {
        let mut missing_path = resolution(StorefrontForumCategoryRouteDisposition::Canonical);
        missing_path.canonical.path.clear();
        let mut missing_identity = resolution(StorefrontForumCategoryRouteDisposition::Redirect);
        missing_identity.canonical.category_id.clear();
        let mut external_target = resolution(StorefrontForumCategoryRouteDisposition::Redirect);
        external_target.canonical.path = "https://example.invalid/forum".to_string();
        let mut protocol_relative = resolution(StorefrontForumCategoryRouteDisposition::Redirect);
        protocol_relative.canonical.path = "//example.invalid/forum".to_string();
        let mut header_injection = resolution(StorefrontForumCategoryRouteDisposition::Redirect);
        header_injection.canonical.path = "/en/forum/c/general\r\nX-Test: injected".to_string();

        for resolution in [
            missing_path,
            missing_identity,
            external_target,
            protocol_relative,
            header_injection,
        ] {
            assert_eq!(
                forum_category_host_action("/en/forum/c/general", &resolution),
                ForumCategoryHostAction::Invalid
            );
        }
    }
}
