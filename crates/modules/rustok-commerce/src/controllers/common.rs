use rustok_api::Permission;
use rustok_api::{AuthContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct PaginationParams {
    #[serde(
        default = "default_page",
        deserialize_with = "deserialize_number_or_string"
    )]
    pub page: u64,
    #[serde(
        default = "default_per_page",
        deserialize_with = "deserialize_number_or_string"
    )]
    pub per_page: u64,
}

fn deserialize_number_or_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct NumberOrStringVisitor;

    impl<'de> serde::de::Visitor<'de> for NumberOrStringVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a positive integer or a string representing a positive integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            if value >= 0 {
                Ok(value as u64)
            } else {
                Err(E::custom("expected positive integer"))
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<u64, E>
        where
            E: serde::de::Error,
        {
            value.parse::<u64>().map_err(E::custom)
        }
    }

    deserializer.deserialize_any(NumberOrStringVisitor)
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

impl PaginationParams {
    pub fn page(&self) -> u64 {
        self.page.max(1)
    }

    pub fn offset(&self) -> u64 {
        (self.page().saturating_sub(1)) * self.limit()
    }

    pub fn limit(&self) -> u64 {
        self.per_page.clamp(1, 100)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
    pub has_next: bool,
    pub has_prev: bool,
}

impl PaginationMeta {
    pub fn new(page: u64, per_page: u64, total: u64) -> Self {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let total_pages = total.div_ceil(per_page);
        Self {
            page,
            per_page,
            total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}

pub(super) fn ensure_permissions(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if auth.tenant_id.is_nil() {
        return Err(HttpError::forbidden(
            "commerce_invalid_tenant",
            "Authenticated context has an invalid nil tenant ID".to_string(),
        ));
    }
    if auth.user_id.is_nil() {
        return Err(HttpError::forbidden(
            "commerce_invalid_actor",
            "Authenticated context has an invalid nil user ID".to_string(),
        ));
    }
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "commerce_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_permissions;
    use rustok_api::AuthContext;
    use rustok_api::Permission;
    use uuid::Uuid;

    #[test]
    fn authenticated_permission_denial_is_forbidden_not_unauthorized() {
        let auth = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let error = ensure_permissions(
            &auth,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )
        .expect_err("missing permission on an authenticated principal must be forbidden");

        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(error.code, "commerce_permission_denied");
    }

    #[test]
    fn authenticated_context_rejects_nil_tenant_or_user() {
        let auth_nil_tenant = AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let err = ensure_permissions(
            &auth_nil_tenant,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )
        .expect_err("nil tenant must be rejected");
        assert_eq!(err.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(err.code, "commerce_invalid_tenant");

        let auth_nil_user = AuthContext {
            user_id: Uuid::nil(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![Permission::ORDERS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };

        let err2 = ensure_permissions(
            &auth_nil_user,
            &[Permission::ORDERS_READ],
            "Permission denied: orders:read required",
        )
        .expect_err("nil user must be rejected");
        assert_eq!(err2.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(err2.code, "commerce_invalid_actor");
    }
}
