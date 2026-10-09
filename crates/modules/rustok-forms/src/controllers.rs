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
    FormSubmissionResponse, FormSubmissionState, ListFormSubmissionsFilter, SubmitFormResponse,
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

fn request_identity(headers: &HeaderMap) -> String {
    for name in ["x-forwarded-for", "x-real-ip"] {
        if let Some(value) = headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.split(',').next().unwrap_or(value).trim().to_string();
        }
    }
    "unknown".to_string()
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
        Some(Value::String(value)) => Some(
            Uuid::parse_str(value.trim()).map_err(|_| invalid_request("page_id must be a UUID"))?,
        ),
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
        Ok(response) => Ok((StatusCode::ACCEPTED, Json(response))),
        Err(error) => {
            let rich: rustok_core::error::RichError = error.into();
            if rich.error_code.as_deref() == Some("FORM_SUBMIT_RATE_LIMITED") {
                Ok((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({ "error": "FORM_SUBMIT_RATE_LIMITED" })),
                ))
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

pub fn axum_router(runtime: &HostRuntimeContext) -> Result<axum::Router, FormsError> {
    let state = FormsHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .route(
            "/api/forms/{form_id}/submit",
            axum::routing::post(submit_form_json),
        )
        .route(
            "/api/forms/{form_id}/submit/form",
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
