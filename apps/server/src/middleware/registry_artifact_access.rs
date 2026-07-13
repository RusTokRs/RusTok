use axum::{
    body::Body,
    extract::State,
    http::{
        header::{self, HeaderValue},
        Method, Request, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::{has_effective_permission, AuthContextExtension, Permission};
use sea_orm::EntityTrait;
use subtle::ConstantTimeEq;

use crate::models::{registry_module_owner, registry_publish_request};
use crate::services::registry_principal::RegistryPrincipalRef;
use crate::services::server_runtime_context::ServerRuntimeContext;

const ARTIFACT_CONTENT_TYPE: &str = "application/octet-stream";
const ARTIFACT_DISPOSITION: &str = "attachment";

/// Enforce registry artifact access before the legacy controller executes.
///
/// A user may download an unpublished artifact only when they created/publish
/// the request, own its module slug, or hold request-effective
/// `modules:manage`. Remote runners retain their separate host-token path.
/// Downloads are streamed through this boundary instead of redirecting to
/// storage, so even legacy objects with unsafe metadata are delivered only as
/// an opaque attachment.
pub async fn enforce(
    State(ctx): State<ServerRuntimeContext>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path();
    let Some((request_id, operation)) = artifact_route(path) else {
        return next.run(request).await;
    };

    if method == Method::PUT && operation == ArtifactOperation::Upload {
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(ARTIFACT_CONTENT_TYPE),
        );
        return next.run(request).await;
    }

    if method != Method::GET || operation != ArtifactOperation::Download {
        return next.run(request).await;
    }

    let publish_request = match registry_publish_request::Entity::find_by_id(request_id)
        .one(ctx.db())
        .await
    {
        Ok(Some(request)) => request,
        Ok(None) => return not_found("Registry publish request was not found"),
        Err(error) => {
            tracing::error!(%error, "Failed to load registry publish request for artifact authorization");
            return internal_error("Failed to authorize registry artifact download");
        }
    };

    if !runner_token_is_valid(&ctx, &request) {
        let auth = match request.extensions().get::<AuthContextExtension>() {
            Some(extension) => &extension.0,
            None => return unauthorized("Registry artifact download requires authentication"),
        };
        if auth.client_id.is_some() && auth.session_id.is_nil() {
            return forbidden("Registry artifact download requires a user session");
        }

        let owner = match registry_module_owner::Entity::find_by_id(publish_request.slug.clone())
            .one(ctx.db())
            .await
        {
            Ok(owner) => owner,
            Err(error) => {
                tracing::error!(%error, "Failed to load registry owner for artifact authorization");
                return internal_error("Failed to authorize registry artifact download");
            }
        };

        let principal = RegistryPrincipalRef::user(auth.user_id);
        let owns_request = principal_matches(&publish_request.requested_by, &principal)
            || publish_request
                .publisher_principal
                .as_ref()
                .is_some_and(|value| principal_matches(value, &principal));
        let owns_slug = owner
            .as_ref()
            .is_some_and(|owner| principal_matches(&owner.owner_principal, &principal));
        let can_manage = has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE);
        let ownerless_requester = owner.is_none() && owns_request;

        if !(can_manage || owns_slug || ownerless_requester) {
            return forbidden(
                "Registry artifact download is restricted to the module owner, request publisher, or modules:manage",
            );
        }
    }

    let Some(storage_key) = publish_request.artifact_storage_key.as_deref() else {
        return not_found("Registry publish artifact was not uploaded");
    };
    let Some(storage) = ctx.shared_get::<rustok_storage::StorageService>() else {
        return internal_error("Registry artifact storage is unavailable");
    };
    let bytes = match storage.read(storage_key).await {
        Ok(bytes) => bytes,
        Err(rustok_storage::StorageError::NotFound(_)) => {
            return not_found("Registry publish artifact was not found")
        }
        Err(error) => {
            tracing::error!(%error, request_id = %publish_request.id, "Failed to read registry artifact");
            return internal_error("Failed to read registry publish artifact");
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, ARTIFACT_CONTENT_TYPE)
        .header(header::CONTENT_DISPOSITION, ARTIFACT_DISPOSITION)
        .header(header::CACHE_CONTROL, "private, no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from(bytes))
        .unwrap_or_else(|error| {
            tracing::error!(%error, "Failed to build registry artifact response");
            internal_error("Failed to build registry artifact response")
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactOperation {
    Upload,
    Download,
}

fn artifact_route(path: &str) -> Option<(&str, ArtifactOperation)> {
    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() == 5
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "publish"
        && segments[4] == "artifact"
    {
        return Some((segments[3], ArtifactOperation::Upload));
    }
    if segments.len() == 6
        && segments[0] == "v2"
        && segments[1] == "catalog"
        && segments[2] == "publish"
        && segments[4] == "artifact"
        && segments[5] == "download"
    {
        return Some((segments[3], ArtifactOperation::Download));
    }
    None
}

fn principal_matches(value: &serde_json::Value, principal: &RegistryPrincipalRef) -> bool {
    let persisted = RegistryPrincipalRef::from_json_value(value);
    if persisted.is_user() && principal.is_user() {
        persisted.user_id() == principal.user_id()
    } else {
        persisted.subject == principal.subject
            || persisted.persisted_label() == principal.persisted_label()
    }
}

fn runner_token_is_valid(ctx: &ServerRuntimeContext, request: &Request<Body>) -> bool {
    let settings = &ctx.settings().registry.remote_executor;
    if !settings.enabled {
        return false;
    }
    let Some(expected) = settings.shared_token.as_deref() else {
        return false;
    };
    let Some(provided) = request
        .headers()
        .get("x-rustok-runner-token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
}

fn response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

fn unauthorized(message: &str) -> Response {
    response(StatusCode::UNAUTHORIZED, "unauthorized", message)
}

fn forbidden(message: &str) -> Response {
    response(StatusCode::FORBIDDEN, "forbidden", message)
}

fn not_found(message: &str) -> Response {
    response(StatusCode::NOT_FOUND, "not_found", message)
}

fn internal_error(message: &str) -> Response {
    response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

#[cfg(test)]
mod tests {
    use super::{artifact_route, ArtifactOperation};

    #[test]
    fn matches_only_registry_artifact_routes() {
        assert_eq!(
            artifact_route("/v2/catalog/publish/rpr_1/artifact"),
            Some(("rpr_1", ArtifactOperation::Upload))
        );
        assert_eq!(
            artifact_route("/v2/catalog/publish/rpr_1/artifact/download"),
            Some(("rpr_1", ArtifactOperation::Download))
        );
        assert_eq!(artifact_route("/v2/catalog/publish/rpr_1"), None);
        assert_eq!(artifact_route("/v1/catalog/rpr_1/artifact"), None);
    }
}
