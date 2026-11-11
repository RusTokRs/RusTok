use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

use crate::context::TenantContext;
use crate::context::TenantContextExt;

pub struct CurrentTenant(pub TenantContext);

impl<S> FromRequestParts<S> for CurrentTenant
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let tenant = parts
            .tenant_context()
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)?;

        Ok(Self(tenant))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::FromRequestParts;
    use axum::http::Request;
    use crate::context::TenantContextExtension;
    use uuid::Uuid;

    #[tokio::test]
    async fn current_tenant_extracts_when_present() {
        let tenant_id = Uuid::new_v4();
        let context = TenantContext {
            id: tenant_id,
            name: "Test Tenant".to_string(),
            slug: "test-tenant".to_string(),
            domain: Some("test.example.com".to_string()),
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };

        let mut request = Request::builder().uri("/").body(()).unwrap();
        request
            .extensions_mut()
            .insert(TenantContextExtension(context.clone()));

        let (mut parts, _) = request.into_parts();
        let extracted = CurrentTenant::from_request_parts(&mut parts, &()).await;
        assert!(extracted.is_ok());
        let current = extracted.unwrap();
        assert_eq!(current.0.id, tenant_id);
        assert_eq!(current.0.slug, "test-tenant");
    }

    #[tokio::test]
    async fn current_tenant_returns_not_found_when_missing() {
        let request = Request::builder().uri("/").body(()).unwrap();
        let (mut parts, _) = request.into_parts();
        let extracted = CurrentTenant::from_request_parts(&mut parts, &()).await;
        assert_eq!(extracted.err(), Some(StatusCode::NOT_FOUND));
    }
}
