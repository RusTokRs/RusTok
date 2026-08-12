use leptos::prelude::*;
#[cfg(feature = "ssr")]
use rustok_ui_core::normalize_ui_text;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList};

#[cfg(feature = "ssr")]
use crate::model::{CurrentTenant, CustomerListItem, CustomerProfileRecord, CustomerRecord};

#[cfg(feature = "ssr")]
const CUSTOMER_ADMIN_OWNER: &str = "rustok_customer.admin_transport";
#[cfg(feature = "ssr")]
const CUSTOMER_ADMIN_BOUNDARY: &str = "customer_admin_native_transport";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<CustomerAdminBootstrap, ApiError> {
    customer_bootstrap_native().await.map_err(Into::into)
}

pub async fn fetch_customers(
    search: String,
    page: u64,
    per_page: u64,
) -> Result<CustomerList, ApiError> {
    customer_list_native(search, page, per_page)
        .await
        .map_err(Into::into)
}

pub async fn fetch_customer_detail(customer_id: String) -> Result<CustomerDetail, ApiError> {
    customer_detail_native(customer_id)
        .await
        .map_err(Into::into)
}

pub async fn create_customer(payload: CustomerDraft) -> Result<CustomerDetail, ApiError> {
    customer_create_native(payload).await.map_err(Into::into)
}

pub async fn update_customer(
    customer_id: String,
    payload: CustomerDraft,
) -> Result<CustomerDetail, ApiError> {
    customer_update_native(customer_id, payload)
        .await
        .map_err(Into::into)
}

#[cfg(feature = "ssr")]
fn customer_admin_correlation_id(owner_operation: &'static str) -> String {
    format!("customer-admin:{owner_operation}:{}", uuid::Uuid::new_v4())
}

#[cfg(feature = "ssr")]
fn customer_context_error<E>(
    _error: E,
    owner_operation: &'static str,
    context_kind: &'static str,
    correlation_id: &str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    tracing::error!(
        error_type = std::any::type_name::<E>(),
        owner = CUSTOMER_ADMIN_OWNER,
        owner_operation,
        context_kind,
        correlation_id,
        code,
        boundary = CUSTOMER_ADMIN_BOUNDARY,
        "customer admin request context extraction failed"
    );
    ServerFnError::new(public_message)
}

#[cfg(feature = "ssr")]
fn auth_context_error<E>(
    error: E,
    owner_operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    customer_context_error(
        error,
        owner_operation,
        "auth",
        correlation_id,
        "customer.admin_auth_context_unavailable",
        "Customer authentication context is temporarily unavailable",
    )
}

#[cfg(feature = "ssr")]
fn tenant_context_error<E>(
    error: E,
    owner_operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    customer_context_error(
        error,
        owner_operation,
        "tenant",
        correlation_id,
        "customer.admin_tenant_context_unavailable",
        "Customer tenant context is temporarily unavailable",
    )
}

#[cfg(feature = "ssr")]
async fn optional_request_context(
    owner_operation: &'static str,
    correlation_id: &str,
) -> Option<rustok_api::RequestContext> {
    match leptos_axum::extract::<rustok_api::RequestContext>().await {
        Ok(context) => Some(context),
        Err(error) => {
            tracing::warn!(
                error_type = std::any::type_name_of_val(&error),
                owner = CUSTOMER_ADMIN_OWNER,
                owner_operation,
                context_kind = "request",
                correlation_id,
                code = "customer.admin_optional_request_context_unavailable",
                boundary = CUSTOMER_ADMIN_BOUNDARY,
                "customer admin optional request context extraction failed"
            );
            None
        }
    }
}

#[cfg(feature = "ssr")]
fn customer_owner_error(
    error: rustok_customer::CustomerError,
    owner_operation: &'static str,
    correlation_id: &str,
    tenant_id: uuid::Uuid,
    actor_id: uuid::Uuid,
    customer_id: Option<uuid::Uuid>,
    request_context: Option<&rustok_api::RequestContext>,
) -> ServerFnError {
    use rustok_customer::CustomerError;

    let (public_message, public_code, technical, error_kind) = match &error {
        CustomerError::Validation(_) => (
            "Customer request is invalid",
            "customer.admin_request_invalid",
            false,
            "validation",
        ),
        CustomerError::CustomerNotFound(_) => (
            "Customer was not found",
            "customer.admin_not_found",
            false,
            "customer_not_found",
        ),
        CustomerError::CustomerByUserNotFound(_) => (
            "Customer was not found",
            "customer.admin_not_found",
            false,
            "customer_by_user_not_found",
        ),
        CustomerError::DuplicateEmail(_) => (
            "Customer email already exists",
            "customer.admin_duplicate_email",
            false,
            "duplicate_email",
        ),
        CustomerError::DuplicateUserLink(_) => (
            "Customer is already linked to a user",
            "customer.admin_duplicate_user_link",
            false,
            "duplicate_user_link",
        ),
        CustomerError::Profile(_) => (
            "Customer profile is temporarily unavailable",
            "customer.admin_profile_unavailable",
            true,
            "profile",
        ),
        CustomerError::Database(_) => (
            "Customer data is temporarily unavailable",
            "customer.admin_storage_unavailable",
            true,
            "database",
        ),
    };

    if technical {
        tracing::error!(
            error_kind,
            owner = "rustok_customer",
            consumer = CUSTOMER_ADMIN_OWNER,
            owner_operation,
            correlation_id,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            customer_id = ?customer_id,
            request_tenant_id = ?request_context.map(|context| context.tenant_id),
            request_user_id = ?request_context.and_then(|context| context.user_id),
            channel_id = ?request_context.and_then(|context| context.channel_id),
            channel_slug = ?request_context.and_then(|context| context.channel_slug.as_deref()),
            locale = ?request_context.map(|context| context.locale.as_str()),
            public_code,
            boundary = CUSTOMER_ADMIN_BOUNDARY,
            "customer admin owner operation failed"
        );
    } else {
        tracing::warn!(
            error_kind,
            owner = "rustok_customer",
            consumer = CUSTOMER_ADMIN_OWNER,
            owner_operation,
            correlation_id,
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            customer_id = ?customer_id,
            request_tenant_id = ?request_context.map(|context| context.tenant_id),
            request_user_id = ?request_context.and_then(|context| context.user_id),
            channel_id = ?request_context.and_then(|context| context.channel_id),
            channel_slug = ?request_context.and_then(|context| context.channel_slug.as_deref()),
            locale = ?request_context.map(|context| context.locale.as_str()),
            public_code,
            boundary = CUSTOMER_ADMIN_BOUNDARY,
            "customer admin owner operation was rejected"
        );
    }

    ServerFnError::new(public_message)
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_api::Permission],
    required: &[rustok_api::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(permissions, required) {
        return Err(ServerFnError::new(format!("Permission denied: {message}")));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(value: &str, field_name: &str) -> Result<Option<uuid::Uuid>, ServerFnError> {
    let Some(value) = normalize_ui_text(value) else {
        return Ok(None);
    };

    Ok(Some(parse_uuid(value.as_str(), field_name)?))
}

#[cfg(feature = "ssr")]
fn customer_service(
    runtime_ctx: &rustok_api::HostRuntimeContext,
) -> rustok_customer::CustomerService {
    rustok_customer::CustomerService::new(runtime_ctx.db_clone())
}

#[cfg(feature = "ssr")]
fn profile_service(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    auth: &rustok_api::AuthContext,
) -> rustok_profiles::ProfilePresentationService {
    let audience = if auth.is_service_principal() {
        rustok_profiles::ProfileAccessAudience::TrustedService { actor_id: None }
    } else {
        rustok_profiles::ProfileAccessAudience::Authenticated {
            actor_id: auth.user_id,
        }
    };
    rustok_profiles::ProfilePresentationService::for_audience(runtime_ctx.db_clone(), audience)
}

#[cfg(feature = "ssr")]
fn map_current_tenant(tenant: &rustok_api::TenantContext) -> CurrentTenant {
    CurrentTenant {
        id: tenant.id.to_string(),
        slug: tenant.slug.clone(),
        name: tenant.name.clone(),
    }
}

#[cfg(feature = "ssr")]
fn display_name(first_name: Option<&str>, last_name: Option<&str>, email: &str) -> String {
    let parts = [first_name, last_name]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        email.to_string()
    } else {
        parts.join(" ")
    }
}

#[cfg(feature = "ssr")]
fn map_customer_list_item(value: rustok_customer::CustomerResponse) -> CustomerListItem {
    CustomerListItem {
        id: value.id.to_string(),
        email: value.email.clone(),
        full_name: display_name(
            value.first_name.as_deref(),
            value.last_name.as_deref(),
            value.email.as_str(),
        ),
        phone: value.phone,
        locale: value.locale,
        user_id: value.user_id.map(|item| item.to_string()),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_customer_record(value: rustok_customer::CustomerResponse) -> CustomerRecord {
    let email = value.email.clone();
    CustomerRecord {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        user_id: value.user_id.map(|item| item.to_string()),
        full_name: display_name(
            value.first_name.as_deref(),
            value.last_name.as_deref(),
            email.as_str(),
        ),
        email,
        first_name: value.first_name,
        last_name: value.last_name,
        phone: value.phone,
        locale: value.locale,
        metadata_pretty: serde_json::to_string_pretty(&value.metadata)
            .unwrap_or_else(|_| "{}".to_string()),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_profile(value: rustok_profiles::ProfileSummary) -> CustomerProfileRecord {
    CustomerProfileRecord {
        handle: value.handle,
        display_name: value.display_name,
        preferred_locale: value.preferred_locale,
        visibility: value.visibility.to_string(),
        tags: value.tags,
    }
}

#[cfg(feature = "ssr")]
async fn load_customer_detail(
    customer_service: &rustok_customer::CustomerService,
    profile_service: &rustok_profiles::ProfilePresentationService,
    tenant: &rustok_api::TenantContext,
    actor_id: uuid::Uuid,
    customer_id: uuid::Uuid,
    requested_locale: Option<&str>,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
    owner_operation: &'static str,
) -> Result<CustomerDetail, ServerFnError> {
    let detail = customer_service
        .get_customer_with_profile(
            profile_service,
            tenant.id,
            customer_id,
            requested_locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|error| {
            customer_owner_error(
                error,
                owner_operation,
                correlation_id,
                tenant.id,
                actor_id,
                Some(customer_id),
                request_context,
            )
        })?;

    Ok(CustomerDetail {
        customer: map_customer_record(detail.customer),
        profile: detail.profile.map(map_profile),
    })
}

#[server(prefix = "/api/fn", endpoint = "customer/bootstrap")]
async fn customer_bootstrap_native() -> Result<CustomerAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext};

        let owner_operation = "bootstrap";
        let correlation_id = customer_admin_correlation_id(owner_operation);
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| auth_context_error(error, owner_operation, &correlation_id))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| tenant_context_error(error, owner_operation, &correlation_id))?;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_LIST, Permission::CUSTOMERS_READ],
            "customers:list or customers:read required",
        )?;

        Ok(CustomerAdminBootstrap {
            current_tenant: map_current_tenant(&tenant),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "customer/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "customer/list")]
async fn customer_list_native(
    search: String,
    page: u64,
    per_page: u64,
) -> Result<CustomerList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_customer::ListCustomersInput;

        let owner_operation = "list_customers";
        let correlation_id = customer_admin_correlation_id(owner_operation);
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| auth_context_error(error, owner_operation, &correlation_id))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| tenant_context_error(error, owner_operation, &correlation_id))?;
        let request_context = optional_request_context(owner_operation, &correlation_id).await;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_LIST],
            "customers:list required",
        )?;

        let search = {
            let trimmed = search.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let service = customer_service(&runtime_ctx);
        let (items, total) = service
            .list_customers(
                tenant.id,
                ListCustomersInput {
                    search,
                    page,
                    per_page,
                },
            )
            .await
            .map_err(|error| {
                customer_owner_error(
                    error,
                    owner_operation,
                    &correlation_id,
                    tenant.id,
                    auth.user_id,
                    None,
                    request_context.as_ref(),
                )
            })?;

        Ok(CustomerList {
            items: items.into_iter().map(map_customer_list_item).collect(),
            total,
            page,
            per_page,
            has_next: page.saturating_mul(per_page) < total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (search, page, per_page);
        Err(ServerFnError::new(
            "customer/list requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "customer/detail")]
async fn customer_detail_native(customer_id: String) -> Result<CustomerDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};

        let owner_operation = "get_customer_detail";
        let correlation_id = customer_admin_correlation_id(owner_operation);
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| auth_context_error(error, owner_operation, &correlation_id))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| tenant_context_error(error, owner_operation, &correlation_id))?;
        let request_context = optional_request_context(owner_operation, &correlation_id).await;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_READ],
            "customers:read required",
        )?;

        let customer_id = parse_uuid(&customer_id, "customer_id")?;
        let customer_service = customer_service(&runtime_ctx);
        let profile_service = profile_service(&runtime_ctx, &auth);

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            auth.user_id,
            customer_id,
            Some(tenant.default_locale.as_str()),
            request_context.as_ref(),
            &correlation_id,
            owner_operation,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = customer_id;
        Err(ServerFnError::new(
            "customer/detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "customer/create")]
async fn customer_create_native(payload: CustomerDraft) -> Result<CustomerDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_customer::CreateCustomerInput;

        let owner_operation = "create_customer";
        let correlation_id = customer_admin_correlation_id(owner_operation);
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| auth_context_error(error, owner_operation, &correlation_id))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| tenant_context_error(error, owner_operation, &correlation_id))?;
        let request_context = optional_request_context(owner_operation, &correlation_id).await;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_CREATE],
            "customers:create required",
        )?;

        let locale = optional_text(payload.locale);
        let requested_locale = locale
            .as_deref()
            .unwrap_or(tenant.default_locale.as_str())
            .to_string();
        let customer_service = customer_service(&runtime_ctx);
        let profile_service = profile_service(&runtime_ctx, &auth);
        let created = customer_service
            .create_customer(
                tenant.id,
                CreateCustomerInput {
                    user_id: parse_optional_uuid(payload.user_id.as_str(), "user_id")?,
                    email: payload.email,
                    first_name: optional_text(payload.first_name),
                    last_name: optional_text(payload.last_name),
                    phone: optional_text(payload.phone),
                    locale,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(|error| {
                customer_owner_error(
                    error,
                    owner_operation,
                    &correlation_id,
                    tenant.id,
                    auth.user_id,
                    None,
                    request_context.as_ref(),
                )
            })?;

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            auth.user_id,
            created.id,
            Some(requested_locale.as_str()),
            request_context.as_ref(),
            &correlation_id,
            owner_operation,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = payload;
        Err(ServerFnError::new(
            "customer/create requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "customer/update")]
async fn customer_update_native(
    customer_id: String,
    payload: CustomerDraft,
) -> Result<CustomerDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
        use rustok_customer::UpdateCustomerInput;

        let owner_operation = "update_customer";
        let correlation_id = customer_admin_correlation_id(owner_operation);
        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|error| auth_context_error(error, owner_operation, &correlation_id))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|error| tenant_context_error(error, owner_operation, &correlation_id))?;
        let request_context = optional_request_context(owner_operation, &correlation_id).await;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_UPDATE],
            "customers:update required",
        )?;

        let customer_id = parse_uuid(&customer_id, "customer_id")?;
        let locale = {
            let trimmed = payload.locale.trim();
            if trimmed.is_empty() {
                tenant.default_locale.clone()
            } else {
                trimmed.to_string()
            }
        };
        let customer_service = customer_service(&runtime_ctx);
        let profile_service = profile_service(&runtime_ctx, &auth);
        customer_service
            .update_customer(
                tenant.id,
                customer_id,
                UpdateCustomerInput {
                    email: Some(payload.email),
                    first_name: Some(payload.first_name),
                    last_name: Some(payload.last_name),
                    phone: Some(payload.phone),
                    locale: Some(locale.clone()),
                    metadata: None,
                },
            )
            .await
            .map_err(|error| {
                customer_owner_error(
                    error,
                    owner_operation,
                    &correlation_id,
                    tenant.id,
                    auth.user_id,
                    Some(customer_id),
                    request_context.as_ref(),
                )
            })?;

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            auth.user_id,
            customer_id,
            Some(locale.as_str()),
            request_context.as_ref(),
            &correlation_id,
            owner_operation,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (customer_id, payload);
        Err(ServerFnError::new(
            "customer/update requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn optional_text(value: String) -> Option<String> {
    normalize_ui_text(value.as_str())
}
