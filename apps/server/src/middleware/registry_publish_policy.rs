use axum::{
    Json,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::{AuthContextExtension, Permission, has_effective_permission};

use crate::services::marketplace_catalog::RegistryPublishRequest;

const PUBLISH_PATH: &str = "/v2/catalog/publish";
const MAX_PUBLISH_BODY_BYTES: usize = 256 * 1024;

/// Validate and authorize registry classification metadata before persistence.
///
/// `ownership` and `trust_level` are copied into the release/catalog model, so
/// they are security classifications rather than presentation fields. Live
/// first-party publication requires a user session and request-effective
/// `modules:manage`; a scoped OAuth token cannot recover broader DB authority.
pub async fn enforce(mut request: Request<Body>, next: Next) -> Response {
    if request.method() != Method::POST || request.uri().path() != PUBLISH_PATH {
        return next.run(request).await;
    }

    let (mut parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_PUBLISH_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return bad_request("Registry publish body is invalid or too large"),
    };
    let mut publish = match serde_json::from_slice::<RegistryPublishRequest>(&bytes) {
        Ok(publish) => publish,
        Err(_) => return bad_request("Registry publish body must be valid JSON"),
    };

    let ownership = match normalize_ownership(&publish.module.ownership) {
        Some(value) => value,
        None => {
            return bad_request("Registry module ownership must be `first_party` or `third_party`");
        }
    };
    let trust_level = match normalize_trust_level(&publish.module.trust_level) {
        Some(value) => value,
        None => {
            return bad_request(
                "Registry module trust_level must be `core`, `verified`, `unverified`, or `private`",
            );
        }
    };
    publish.module.ownership = ownership.to_string();
    publish.module.trust_level = trust_level.to_string();

    if !publish.dry_run {
        if ownership != "first_party" {
            return bad_request(
                "Live reference-registry publication currently supports only first_party ownership",
            );
        }
        let auth = match parts.extensions.get::<AuthContextExtension>() {
            Some(extension) => &extension.0,
            None => return unauthorized("Live registry publication requires authentication"),
        };
        if auth.client_id.is_some() && auth.session_id.is_nil() {
            return forbidden("Live registry publication requires a user session");
        }
        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return forbidden(
                "modules:manage is required to classify and publish first-party registry modules",
            );
        }
    }

    let normalized = match serde_json::to_vec(&publish) {
        Ok(bytes) => bytes,
        Err(_) => return internal_error("Failed to normalize registry publish request"),
    };
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    request = Request::from_parts(parts, Body::from(normalized));
    next.run(request).await
}

fn normalize_ownership(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "first_party" => Some("first_party"),
        "third_party" => Some("third_party"),
        _ => None,
    }
}

fn normalize_trust_level(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "core" => Some("core"),
        "verified" => Some("verified"),
        "unverified" => Some("unverified"),
        "private" => Some("private"),
        _ => None,
    }
}

fn response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

fn bad_request(message: &str) -> Response {
    response(StatusCode::BAD_REQUEST, "invalid_request", message)
}

fn unauthorized(message: &str) -> Response {
    response(StatusCode::UNAUTHORIZED, "unauthorized", message)
}

fn forbidden(message: &str) -> Response {
    response(StatusCode::FORBIDDEN, "forbidden", message)
}

fn internal_error(message: &str) -> Response {
    response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

#[cfg(test)]
mod tests {
    use super::{normalize_ownership, normalize_trust_level};

    #[test]
    fn registry_classifications_are_canonicalized() {
        assert_eq!(normalize_ownership(" FIRST_PARTY "), Some("first_party"));
        assert_eq!(normalize_ownership("third_party"), Some("third_party"));
        assert_eq!(normalize_ownership("official"), None);

        assert_eq!(normalize_trust_level("CORE"), Some("core"));
        assert_eq!(normalize_trust_level("verified"), Some("verified"));
        assert_eq!(normalize_trust_level("trusted"), None);
    }
}
