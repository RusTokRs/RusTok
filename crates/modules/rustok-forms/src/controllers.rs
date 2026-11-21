use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, TenantContext, has_any_effective_permission,
};
use rustok_core::SecurityContext;
use rustok_email::TransactionalEmailSender;
use rustok_outbox::TransactionalEventBus;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{
    FormSubmissionResponse, ListFormSubmissionsFilter, SubmitFormResponse,
    UpdateFormSubmissionStateInput,
};
use crate::error::{FORM_SUBMIT_PAYLOAD_INVALID, FormsError, FormsResult};
use crate::services::{FormsNotification, FormsService, notification_from_shared};

#[derive(Clone)]
pub struct FormsHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    notify: Option<FormsNotification>,
}

impl FormsHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> FormsResult<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                FormsError::internal(
                    "forms HTTP routes require TransactionalEventBus in HostRuntimeContext",
                )
            })?;
        let sender = runtime.shared_get::<Arc<dyn TransactionalEmailSender>>();
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            notify: notification_from_shared(sender),
        })
    }

    fn service(&self) -> FormsService {
        FormsService::new(self.db.clone(), self.event_bus.clone(), self.notify.clone())
    }
}

fn security(auth: &AuthContext) -> SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

/// Header carrying the client IP after the server's request-trust evaluation.
/// The server middleware strips any client-supplied value and re-inserts the
/// effective IP, so this is the only forwarding header a module may trust.
const TRUSTED_CLIENT_IP_HEADER: &str = "x-rustok-trusted-client-ip";

fn request_identity(headers: &HeaderMap) -> String {
    // The submit rate limit is keyed by this identity, so it must not come
    // from client-controlled forwarding headers: rotating `X-Forwarded-For`
    // per request would mint a fresh rate-limit bucket every time and allow
    // unbounded submission spam. Only the server-injected trusted IP counts;
    // without it all anonymous callers share the single "unknown" bucket
    // (fail-closed) instead of trusting spoofable headers.
    headers
        .get(TRUSTED_CLIENT_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "unknown".to_string())
}

fn request_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(512).collect())
}

fn split_reserved_fields(
    mut fields: Map<String, Value>,
) -> FormsResult<(Option<Uuid>, Option<String>, Map<String, Value>)> {
    let page_id = match fields.remove("page_id") {
        Some(Value::String(value)) => {
            let parsed = Uuid::parse_str(value.trim())
                .map_err(|_| invalid_request("page_id must be a UUID"))?;
            if parsed.is_nil() {
                return Err(invalid_request("page_id must not be nil"));
            }
            Some(parsed)
        }
        Some(Value::Null) | None => None,
        Some(_) => return Err(invalid_request("page_id must be a UUID string")),
    };
    let locale = match fields.remove("locale") {
        Some(Value::String(value)) => Some(value),
        Some(Value::Null) | None => None,
        Some(_) => return Err(invalid_request("locale must be a string")),
    };
    Ok((page_id, locale, fields))
}

fn invalid_request(message: impl Into<String>) -> FormsError {
    FormsError::Rich(Box::new(
        rustok_core::error::RichError::new(
            rustok_core::error::ErrorKind::Validation,
            message.into(),
        )
        .with_error_code(FORM_SUBMIT_PAYLOAD_INVALID),
    ))
}

/// JSON intake: one flat object of scalar fields (reserved keys: `page_id`, `locale`).
#[utoipa::path(
    post,
    path = "/api/forms/{form_id}/submit",
    tag = "forms",
    params(("form_id" = String, Path, description = "Form identifier from the page document")),
    request_body(content = String, content_type = "application/json", description = "Flat object of scalar form fields; reserved keys: page_id, locale, website (honeypot)"),
    responses(
        (status = 202, description = "Submission accepted (captures look identical)", body = SubmitFormResponse),
        (status = 400, description = "Invalid payload"),
        (status = 429, description = "Rate limited", body = String),
        (status = 500, description = "Storage failure")
    )
)]
pub async fn submit_form_json(
    State(runtime): State<FormsHttpRuntime>,
    tenant: TenantContext,
    Path(form_id): Path<String>,
    headers: HeaderMap,
    Json(fields): Json<Map<String, Value>>,
) -> HttpResult<impl IntoResponse> {
    submit_form_fields(runtime, tenant, form_id, headers, fields).await
}

/// Form-encoded intake for native HTML form posts.
#[utoipa::path(
    post,
    path = "/api/forms/{form_id}/submit/form",
    tag = "forms",
    params(("form_id" = String, Path, description = "Form identifier from the page document")),
    request_body(content = String, content_type = "application/x-www-form-urlencoded", description = "Form-encoded scalar form fields; reserved keys: page_id, locale, website (honeypot)"),
    responses(
        (status = 202, description = "Submission accepted (captures look identical)", body = SubmitFormResponse),
        (status = 400, description = "Invalid payload"),
        (status = 429, description = "Rate limited", body = String),
        (status = 500, description = "Storage failure")
    )
)]
pub async fn submit_form_urlencoded(
    State(runtime): State<FormsHttpRuntime>,
    tenant: TenantContext,
    Path(form_id): Path<String>,
    headers: HeaderMap,
    axum::extract::Form(fields): axum::extract::Form<Map<String, Value>>,
) -> HttpResult<impl IntoResponse> {
    submit_form_fields(runtime, tenant, form_id, headers, fields).await
}

async fn submit_form_fields(
    runtime: FormsHttpRuntime,
    tenant: TenantContext,
    form_id: String,
    headers: HeaderMap,
    fields: Map<String, Value>,
) -> HttpResult<impl IntoResponse> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant",
            "tenant_id must not be nil",
        ));
    }
    let (page_id, locale, fields) = split_reserved_fields(fields).map_err(HttpError::from)?;
    let identity = request_identity(&headers);
    let user_agent = request_user_agent(&headers);
    let result = runtime
        .service()
        .submit(
            tenant.id,
            &form_id,
            page_id,
            locale.as_deref(),
            fields,
            &identity,
            user_agent,
        )
        .await;
    match result {
        Ok(response) => Ok((StatusCode::ACCEPTED, Json(response)).into_response()),
        Err(error) => {
            let rich: rustok_core::error::RichError = error.into();
            if rich.error_code.as_deref() == Some("FORM_SUBMIT_RATE_LIMITED") {
                Ok((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({ "error": "FORM_SUBMIT_RATE_LIMITED" })),
                )
                    .into_response())
            } else {
                Err(HttpError::from(FormsError::Rich(Box::new(rich))))
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/forms/submissions",
    tag = "forms",
    params(ListFormSubmissionsFilter),
    responses(
        (status = 200, description = "Submissions newest first", body = [FormSubmissionResponse]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_form_submissions(
    State(runtime): State<FormsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    axum::extract::Query(filter): axum::extract::Query<ListFormSubmissionsFilter>,
) -> HttpResult<Json<Vec<FormSubmissionResponse>>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant",
            "tenant_id must not be nil",
        ));
    }
    ensure_forms_permission(&auth, Permission::FORMS_READ)?;
    runtime
        .service()
        .list(tenant.id, security(&auth), filter)
        .await
        .map(Json)
        .map_err(HttpError::from)
}

#[utoipa::path(
    patch,
    path = "/api/admin/forms/submissions/{id}",
    tag = "forms",
    params(("id" = Uuid, Path, description = "Submission ID")),
    request_body = UpdateFormSubmissionStateInput,
    responses(
        (status = 200, description = "Submission state updated", body = FormSubmissionResponse),
        (status = 404, description = "Submission not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_form_submission_state(
    State(runtime): State<FormsHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateFormSubmissionStateInput>,
) -> HttpResult<Json<FormSubmissionResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant",
            "tenant_id must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_id",
            "submission id must not be nil",
        ));
    }
    ensure_forms_permission(&auth, Permission::FORMS_MANAGE)?;
    runtime
        .service()
        .set_state(tenant.id, security(&auth), id, input.state)
        .await
        .map(Json)
        .map_err(HttpError::from)
}

fn ensure_forms_permission(auth: &AuthContext, permission: Permission) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(HttpError::forbidden(
            "forms_permission_denied",
            "Permission denied: forms:* required",
        ));
    }
    Ok(())
}

/// JSON intake route; `{form_id}` is the Fly document form identifier.
pub const SUBMIT_JSON_ROUTE: &str = "/api/forms/{form_id}/submit";

/// URL-encoded intake route; this is the native `action` a rendered provider form is bound to.
///
/// Mirrored as `FORMS_SUBMIT_ENDPOINT_TEMPLATE` in
/// `crates/modules/rustok-page-builder/src/interaction_providers.rs`;
/// `scripts/verify/verify-fly-interaction-capabilities.mjs` fails when the two drift apart.
pub const SUBMIT_URLENCODED_ROUTE: &str = "/api/forms/{form_id}/submit/form";

pub fn axum_router(runtime: &HostRuntimeContext) -> Result<axum::Router, FormsError> {
    let state = FormsHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route(SUBMIT_JSON_ROUTE, axum::routing::post(submit_form_json))
        .route(
            SUBMIT_URLENCODED_ROUTE,
            axum::routing::post(submit_form_urlencoded),
        )
        .route(
            "/api/admin/forms/submissions",
            axum::routing::get(list_form_submissions),
        )
        .route(
            "/api/admin/forms/submissions/{id}",
            axum::routing::patch(update_form_submission_state),
        )
        .with_state(state))
}

#[cfg(test)]
mod tests {
    use super::{TRUSTED_CLIENT_IP_HEADER, request_identity};
    use axum::http::HeaderMap;

    #[test]
    fn request_identity_uses_only_the_server_trusted_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(TRUSTED_CLIENT_IP_HEADER, "203.0.113.7".parse().unwrap());
        headers.insert("x-forwarded-for", "198.51.100.9".parse().unwrap());
        assert_eq!(request_identity(&headers), "203.0.113.7");
    }

    #[test]
    fn request_identity_ignores_spoofable_forwarding_headers() {
        // Rotating client-supplied forwarding headers must not mint fresh
        // rate-limit buckets: without the server-trusted IP every anonymous
        // caller shares the fail-closed "unknown" identity.
        for value in ["198.51.100.9", "192.0.2.44, 198.51.100.9"] {
            let mut headers = HeaderMap::new();
            headers.insert("x-forwarded-for", value.parse().unwrap());
            headers.insert("x-real-ip", "192.0.2.44".parse().unwrap());
            assert_eq!(request_identity(&headers), "unknown");
        }
        assert_eq!(request_identity(&HeaderMap::new()), "unknown");
    }
}
