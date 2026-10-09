use async_graphql::{
    Context, ErrorExtensions, FieldError, InputObject, Object, Result, SimpleObject,
};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{FormSubmissionResponse, FormSubmissionState, ListFormSubmissionsFilter};
use crate::services::FormsService;

const MODULE_SLUG: &str = "forms";

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlFormSubmission {
    pub id: Uuid,
    pub form_id: String,
    pub locale: String,
    pub page_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub state: String,
    pub created_at: String,
    pub handled_at: Option<String>,
    pub handled_by: Option<Uuid>,
}

impl From<FormSubmissionResponse> for GqlFormSubmission {
    fn from(value: FormSubmissionResponse) -> Self {
        Self {
            id: value.id,
            form_id: value.form_id,
            locale: value.locale,
            page_id: value.page_id,
            payload: value.payload,
            state: value.state.as_str().to_string(),
            created_at: value.created_at,
            handled_at: value.handled_at,
            handled_by: value.handled_by,
        }
    }
}

#[derive(InputObject)]
pub struct ListGqlFormSubmissionsFilter {
    pub form_id: Option<String>,
    pub state: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct UpdateGqlFormSubmissionStateInput {
    /// Target state: `new`, `read`, `handled` or `spam`.
    pub state: String,
}

pub struct FormsQuery;

#[Object]
impl FormsQuery {
    /// Stored form submissions of the current tenant, newest first.
    async fn form_submissions(
        &self,
        ctx: &Context<'_>,
        filter: Option<ListGqlFormSubmissionsFilter>,
    ) -> Result<Vec<GqlFormSubmission>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_forms_permission(ctx, Permission::FORMS_READ)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let filter = filter.unwrap_or(ListGqlFormSubmissionsFilter {
            form_id: None,
            state: None,
            page: None,
            per_page: None,
        });
        let state = match filter.state.as_deref() {
            Some(raw) => Some(
                FormSubmissionState::parse(raw)
                    .ok_or_else(|| async_graphql::Error::new("unknown submission state"))?,
            ),
            None => None,
        };
        FormsService::new(db.clone(), event_bus.clone(), None)
            .list(
                tenant.id,
                security(&auth),
                ListFormSubmissionsFilter {
                    form_id: filter.form_id,
                    state,
                    page: filter.page.unwrap_or(1),
                    per_page: filter.per_page.unwrap_or(20),
                },
            )
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

pub struct FormsMutation;

#[Object]
impl FormsMutation {
    /// Moves one submission through the triage state machine.
    async fn update_form_submission_state(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateGqlFormSubmissionStateInput,
    ) -> Result<GqlFormSubmission> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let auth = require_forms_permission(ctx, Permission::FORMS_MANAGE)?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let state = FormSubmissionState::parse(input.state.as_str())
            .ok_or_else(|| async_graphql::Error::new("unknown submission state"))?;
        FormsService::new(db.clone(), event_bus.clone(), None)
            .set_state(tenant.id, security(&auth), id, state)
            .await
            .map(Into::into)
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

fn require_forms_permission(ctx: &Context<'_>, permission: Permission) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forms:* required",
        ));
    }

    Ok(auth)
}

fn security(auth: &AuthContext) -> SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}
