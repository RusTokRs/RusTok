use axum::{
    extract::FromRequestParts,
    http::{request::Parts, Extensions, StatusCode},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant context available after tenant middleware resolves the active tenant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantContext {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub settings: serde_json::Value,
    pub default_locale: String,
    pub is_active: bool,
}

impl TenantContext {
    pub fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.settings
            .get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TenantError {
    #[error("Tenant not found")]
    NotFound,
    #[error("Tenant is disabled")]
    Disabled,
    #[error("Missing tenant identifier")]
    MissingIdentifier,
    #[error("Invalid tenant identifier")]
    InvalidIdentifier,
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

impl TenantError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Disabled => StatusCode::FORBIDDEN,
            Self::MissingIdentifier | Self::InvalidIdentifier => StatusCode::BAD_REQUEST,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone)]
pub struct TenantContextExtension(pub TenantContext);

pub trait TenantContextExt {
    fn tenant_context(&self) -> Option<&TenantContext>;
}

impl TenantContextExt for Parts {
    fn tenant_context(&self) -> Option<&TenantContext> {
        self.extensions
            .get::<TenantContextExtension>()
            .map(|ext| &ext.0)
    }
}

impl TenantContextExt for Extensions {
    fn tenant_context(&self) -> Option<&TenantContext> {
        self.get::<TenantContextExtension>().map(|ext| &ext.0)
    }
}

impl<S> FromRequestParts<S> for TenantContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<TenantContextExtension>()
            .map(|ext| ext.0.clone())
            .ok_or((
                StatusCode::INTERNAL_SERVER_ERROR,
                "TenantContext not found. Did you forget TenantMiddleware?".to_string(),
            ))
    }
}

pub struct OptionalTenant(pub Option<TenantContext>);

impl<S> FromRequestParts<S> for OptionalTenant
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<TenantContextExtension>()
                .map(|ext| ext.0.clone()),
        ))
    }
}
