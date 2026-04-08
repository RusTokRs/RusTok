use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList};

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
fn ensure_permission(
    permissions: &[rustok_core::Permission],
    required: &[rustok_core::Permission],
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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_uuid(trimmed, field_name)?))
    }
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
    profile_service: &rustok_profiles::ProfileService,
    tenant: &rustok_api::TenantContext,
    customer_id: uuid::Uuid,
    requested_locale: Option<&str>,
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
        .map_err(ServerFnError::new)?;

    Ok(CustomerDetail {
        customer: map_customer_record(detail.customer),
        profile: detail.profile.map(map_profile),
    })
}

#[server(prefix = "/api/fn", endpoint = "customer/bootstrap")]
async fn customer_bootstrap_native() -> Result<CustomerAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

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
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_customer::{CustomerService, ListCustomersInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

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
        let service = CustomerService::new(app_ctx.db.clone());
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
            .map_err(ServerFnError::new)?;

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
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_customer::CustomerService;
        use rustok_profiles::ProfileService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_READ],
            "customers:read required",
        )?;

        let customer_id = parse_uuid(&customer_id, "customer_id")?;
        let customer_service = CustomerService::new(app_ctx.db.clone());
        let profile_service = ProfileService::new(app_ctx.db.clone());

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            customer_id,
            Some(tenant.default_locale.as_str()),
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
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_customer::{CreateCustomerInput, CustomerService};
        use rustok_profiles::ProfileService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_CREATE],
            "customers:create required",
        )?;

        let requested_locale = {
            let trimmed = payload.locale.trim();
            if trimmed.is_empty() {
                Some(tenant.default_locale.as_str())
            } else {
                Some(trimmed)
            }
        };
        let customer_service = CustomerService::new(app_ctx.db.clone());
        let profile_service = ProfileService::new(app_ctx.db.clone());
        let created = customer_service
            .create_customer(
                tenant.id,
                CreateCustomerInput {
                    user_id: parse_optional_uuid(payload.user_id.as_str(), "user_id")?,
                    email: payload.email,
                    first_name: optional_text(payload.first_name),
                    last_name: optional_text(payload.last_name),
                    phone: optional_text(payload.phone),
                    locale: optional_text(payload.locale),
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            created.id,
            requested_locale,
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
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_customer::{CustomerService, UpdateCustomerInput};
        use rustok_profiles::ProfileService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::CUSTOMERS_UPDATE],
            "customers:update required",
        )?;

        let customer_id = parse_uuid(&customer_id, "customer_id")?;
        let requested_locale = {
            let trimmed = payload.locale.trim();
            if trimmed.is_empty() {
                Some(tenant.default_locale.as_str())
            } else {
                Some(trimmed)
            }
        };
        let customer_service = CustomerService::new(app_ctx.db.clone());
        let profile_service = ProfileService::new(app_ctx.db.clone());
        customer_service
            .update_customer(
                tenant.id,
                customer_id,
                UpdateCustomerInput {
                    email: Some(payload.email),
                    first_name: Some(payload.first_name),
                    last_name: Some(payload.last_name),
                    phone: Some(payload.phone),
                    locale: Some(payload.locale),
                    metadata: None,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        load_customer_detail(
            &customer_service,
            &profile_service,
            &tenant,
            customer_id,
            requested_locale,
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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
