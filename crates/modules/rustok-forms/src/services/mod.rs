//! Form submission intake and triage.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use sea_orm::TransactionTrait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use tracing::{instrument, warn};
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_email::TransactionalEmailSender;
use rustok_events::FormSubmissionEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    FormSubmissionResponse, FormSubmissionState, ListFormSubmissionsFilter, SubmitFormResponse,
};
use crate::entities::form_submission;
use crate::error::{
    FORM_SUBMISSION_NOT_FOUND, FORM_SUBMISSION_STATE_INVALID, FORM_SUBMIT_PAYLOAD_INVALID,
    FORM_SUBMIT_RATE_LIMITED, FormsError, FormsResult,
};

/// Silent trap field: non-empty values are stored as `spam` and answer like success.
pub const FORM_HONEYPOT_FIELD: &str = "website";

/// Maximum accepted payload size in bytes.
pub const MAX_FORM_PAYLOAD_BYTES: usize = 16 * 1024;
/// Maximum number of submitted fields.
pub const MAX_FORM_PAYLOAD_FIELDS: usize = 50;
/// Maximum field name length.
pub const MAX_FORM_FIELD_NAME_BYTES: usize = 64;
/// Maximum string field value length.
pub const MAX_FORM_FIELD_VALUE_BYTES: usize = 4096;

/// Rate limit window per (tenant, form, identity).
pub const FORM_SUBMIT_RATE_WINDOW_MINUTES: i64 = 10;
/// Maximum non-spam submissions per identity and window.
pub const FORM_SUBMIT_RATE_LIMIT: u64 = 5;

/// Email notification wiring for accepted submissions.
#[derive(Clone)]
pub struct FormsNotification {
    pub sender: Arc<dyn TransactionalEmailSender>,
    pub to: String,
    pub template_id: String,
}

#[derive(Clone)]
pub struct FormsService {
    db: sea_orm::DatabaseConnection,
    event_bus: TransactionalEventBus,
    notify: Option<FormsNotification>,
}

impl FormsService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        event_bus: TransactionalEventBus,
        notify: Option<FormsNotification>,
    ) -> Self {
        Self {
            db,
            event_bus,
            notify,
        }
    }

    /// Stores one submission after abuse controls; honeypot captures answer like success.
    #[instrument(skip(self, fields))]
    pub async fn submit(
        &self,
        tenant_id: Uuid,
        form_id: &str,
        page_id: Option<Uuid>,
        locale: Option<&str>,
        fields: Map<String, Value>,
        identity: &str,
        user_agent: Option<String>,
    ) -> FormsResult<SubmitFormResponse> {
        validate_form_id(form_id)?;
        validate_payload(&fields)?;

        let honeypot_filled = fields
            .get(FORM_HONEYPOT_FIELD)
            .is_some_and(|value| match value {
                Value::Null => false,
                Value::String(text) => !text.trim().is_empty(),
                Value::Bool(flag) => *flag,
                _ => true,
            });
        let mut payload_fields = fields;
        payload_fields.remove(FORM_HONEYPOT_FIELD);
        let state = if honeypot_filled {
            FormSubmissionState::Spam
        } else {
            self.enforce_rate_limit(tenant_id, form_id, identity)
                .await?;
            FormSubmissionState::New
        };

        let ip_hash = hash_identity(identity);
        let now = Utc::now();
        let id = Uuid::new_v4();
        let locale = normalize_locale(locale);

        let txn = self.db.begin().await?;
        let row = form_submission::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            form_id: Set(form_id.to_string()),
            locale: Set(locale.clone()),
            page_id: Set(page_id),
            payload: Set(Value::Object(payload_fields)),
            state: Set(state.as_str().to_string()),
            ip_hash: Set(ip_hash),
            user_agent: Set(user_agent),
            created_at: Set(now.into()),
            handled_at: Set(None),
            handled_by: Set(None),
        }
        .insert(&txn)
        .await?;
        self.event_bus
            .publish_contract_in_tx(
                &txn,
                tenant_id,
                None,
                FormSubmissionEvent::SubmissionReceived {
                    submission_id: row.id,
                    tenant_id,
                    form_id: form_id.to_string(),
                    page_id,
                    locale,
                    state: state.as_str().to_string(),
                },
            )
            .await?;
        txn.commit().await?;

        if state != FormSubmissionState::Spam {
            self.notify_submission(id, form_id, page_id, &row.locale, &row.payload)
                .await;
        }
        Ok(SubmitFormResponse { id, accepted: true })
    }

    /// Lists submissions newest first.
    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListFormSubmissionsFilter,
    ) -> FormsResult<Vec<FormSubmissionResponse>> {
        enforce_forms_read(&security)?;
        let mut query =
            form_submission::Entity::find().filter(form_submission::Column::TenantId.eq(tenant_id));
        if let Some(form_id) = filter.form_id {
            query = query.filter(form_submission::Column::FormId.eq(form_id));
        }
        if let Some(state) = filter.state {
            query = query.filter(form_submission::Column::State.eq(state.as_str()));
        }
        let rows = query
            .order_by_desc(form_submission::Column::CreatedAt)
            .order_by_desc(form_submission::Column::Id)
            .offset(
                filter
                    .page
                    .saturating_sub(1)
                    .saturating_mul(filter.per_page),
            )
            .limit(filter.per_page.clamp(1, 100))
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(submission_response_from_model)
            .collect())
    }

    /// Moves one submission through the triage state machine.
    #[instrument(skip(self, security))]
    pub async fn set_state(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        id: Uuid,
        state: FormSubmissionState,
    ) -> FormsResult<FormSubmissionResponse> {
        let actor_id = enforce_forms_manage(&security)?;
        let row = form_submission::Entity::find_by_id(id)
            .filter(form_submission::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                FormsError::Rich(Box::new(
                    rustok_core::error::RichError::new(
                        rustok_core::error::ErrorKind::NotFound,
                        format!("Form submission `{id}` not found"),
                    )
                    .with_error_code(FORM_SUBMISSION_NOT_FOUND),
                ))
            })?;
        let current = FormSubmissionState::parse(&row.state).expect("state is storage-checked");
        if !matches!(
            (current, state),
            (FormSubmissionState::New, FormSubmissionState::Read)
                | (FormSubmissionState::New, FormSubmissionState::Handled)
                | (FormSubmissionState::New, FormSubmissionState::Spam)
                | (FormSubmissionState::Read, FormSubmissionState::Handled)
                | (FormSubmissionState::Read, FormSubmissionState::Spam)
                | (FormSubmissionState::Spam, FormSubmissionState::Read)
                | (FormSubmissionState::Handled, FormSubmissionState::Read)
        ) {
            return Err(FormsError::Rich(Box::new(
                rustok_core::error::RichError::new(
                    rustok_core::error::ErrorKind::Validation,
                    format!(
                        "Form submission state transition `{}` -> `{}` is not allowed",
                        current.as_str(),
                        state.as_str()
                    ),
                )
                .with_error_code(FORM_SUBMISSION_STATE_INVALID),
            )));
        }
        let mut active: form_submission::ActiveModel = row.into();
        active.state = Set(state.as_str().to_string());
        if state == FormSubmissionState::Handled {
            active.handled_at = Set(Some(Utc::now().into()));
            active.handled_by = Set(Some(actor_id));
        }
        let row = active.update(&self.db).await?;
        Ok(submission_response_from_model(row))
    }

    async fn enforce_rate_limit(
        &self,
        tenant_id: Uuid,
        form_id: &str,
        identity: &str,
    ) -> FormsResult<()> {
        let window_start: sea_orm::prelude::DateTimeWithTimeZone =
            (Utc::now() - ChronoDuration::minutes(FORM_SUBMIT_RATE_WINDOW_MINUTES)).into();
        let recent = form_submission::Entity::find()
            .filter(form_submission::Column::TenantId.eq(tenant_id))
            .filter(form_submission::Column::FormId.eq(form_id))
            .filter(form_submission::Column::IpHash.eq(hash_identity(identity)))
            .filter(form_submission::Column::State.ne(FormSubmissionState::Spam.as_str()))
            .filter(form_submission::Column::CreatedAt.gte(window_start))
            .count(&self.db)
            .await?;
        if recent >= FORM_SUBMIT_RATE_LIMIT {
            return Err(FormsError::Rich(Box::new(
                rustok_core::error::RichError::new(
                    rustok_core::error::ErrorKind::RateLimited,
                    "Too many form submissions, retry later",
                )
                .with_user_message("Too many submissions. Please retry later.")
                .with_error_code(FORM_SUBMIT_RATE_LIMITED),
            )));
        }
        Ok(())
    }

    async fn notify_submission(
        &self,
        id: Uuid,
        form_id: &str,
        page_id: Option<Uuid>,
        locale: &str,
        payload: &Value,
    ) {
        let Some(notify) = &self.notify else {
            return;
        };
        let vars = serde_json::json!({
            "submission_id": id.to_string(),
            "form_id": form_id,
            "page_id": page_id.map(|value| value.to_string()),
            "locale": locale,
            "payload": payload,
        });
        if let Err(error) = notify
            .sender
            .send_transactional(&notify.template_id, locale, &notify.to, &vars)
            .await
        {
            warn!(%error, submission_id = %id, "form submission email notification failed");
        }
    }
}

fn submission_response_from_model(model: form_submission::Model) -> FormSubmissionResponse {
    FormSubmissionResponse {
        id: model.id,
        form_id: model.form_id,
        locale: model.locale,
        page_id: model.page_id,
        payload: model.payload,
        state: FormSubmissionState::parse(&model.state)
            .expect("submission state is constrained by the storage check"),
        created_at: model.created_at.to_string(),
        handled_at: model.handled_at.map(|value| value.to_string()),
        handled_by: model.handled_by,
    }
}

fn hash_identity(identity: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok-forms-ip-hash-v1:");
    hasher.update(identity.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_locale(locale: Option<&str>) -> String {
    locale
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
        .filter(|value| value.len() <= 16)
        .unwrap_or_else(|| "und".to_string())
}

fn validate_form_id(form_id: &str) -> FormsResult<()> {
    if form_id.is_empty()
        || form_id.len() > 64
        || !form_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(invalid_payload(
            "form_id must be 1-64 ascii alphanumerics, `-` or `_`",
        ));
    }
    Ok(())
}

fn validate_payload(fields: &Map<String, Value>) -> FormsResult<()> {
    if fields.len() > MAX_FORM_PAYLOAD_FIELDS {
        return Err(invalid_payload(format!(
            "payload has more than {MAX_FORM_PAYLOAD_FIELDS} fields"
        )));
    }
    for (name, value) in fields {
        if name.is_empty() || name.len() > MAX_FORM_FIELD_NAME_BYTES {
            return Err(invalid_payload(format!(
                "field names must be 1-{MAX_FORM_FIELD_NAME_BYTES} bytes"
            )));
        }
        match value {
            Value::String(text) if text.len() > MAX_FORM_FIELD_VALUE_BYTES => {
                return Err(invalid_payload(format!(
                    "field `{name}` exceeds {MAX_FORM_FIELD_VALUE_BYTES} bytes"
                )));
            }
            Value::Array(_) | Value::Object(_) => {
                return Err(invalid_payload(format!(
                    "field `{name}` must be a scalar value"
                )));
            }
            _ => {}
        }
    }
    let encoded = serde_json::to_string(&Value::Object(fields.clone()))
        .map_err(|error| invalid_payload(format!("payload is not serializable: {error}")))?;
    if encoded.len() > MAX_FORM_PAYLOAD_BYTES {
        return Err(invalid_payload(format!(
            "payload exceeds {MAX_FORM_PAYLOAD_BYTES} bytes"
        )));
    }
    Ok(())
}

fn invalid_payload(message: impl Into<String>) -> FormsError {
    FormsError::Rich(Box::new(
        rustok_core::error::RichError::new(
            rustok_core::error::ErrorKind::Validation,
            message.into(),
        )
        .with_error_code(FORM_SUBMIT_PAYLOAD_INVALID),
    ))
}

fn enforce_forms_read(security: &SecurityContext) -> FormsResult<()> {
    use rustok_api::{Action, Resource};
    if security.get_scope(Resource::Forms, Action::Read) == rustok_core::PermissionScope::None {
        return Err(FormsError::forbidden(
            "Permission denied: forms:read required",
        ));
    }
    Ok(())
}

fn enforce_forms_manage(security: &SecurityContext) -> FormsResult<Uuid> {
    use rustok_api::{Action, Resource};
    if security.get_scope(Resource::Forms, Action::Manage) == rustok_core::PermissionScope::None {
        return Err(FormsError::forbidden(
            "Permission denied: forms:manage required",
        ));
    }
    security
        .user_id
        .ok_or_else(|| FormsError::forbidden("Form triage requires an authenticated user"))
}

/// Builds the notification handle from host shared values and environment config.
///
/// `FORMS_NOTIFY_TO` selects the recipient; without it the inbox is the only
/// notification surface. `FORMS_NOTIFY_TEMPLATE` overrides the template id.
pub fn notification_from_shared(
    sender: Option<Arc<dyn TransactionalEmailSender>>,
) -> Option<FormsNotification> {
    let to = std::env::var("FORMS_NOTIFY_TO").ok()?;
    let sender = sender?;
    Some(FormsNotification {
        sender,
        to,
        template_id: std::env::var("FORMS_NOTIFY_TEMPLATE")
            .unwrap_or_else(|_| "form_submission".to_string()),
    })
}
