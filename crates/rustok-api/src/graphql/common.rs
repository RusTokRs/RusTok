use async_graphql::{Context, ErrorExtensions, InputObject, Result, SimpleObject};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::PLATFORM_FALLBACK_LOCALE;
use crate::context::TenantContext;
use crate::request::RequestContext;

#[derive(SimpleObject, Debug, Clone)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
    pub total_count: i64,
}

impl PageInfo {
    pub fn new(total: i64, offset: i64, limit: i64) -> Self {
        let start_cursor = if total > 0 {
            Some(encode_cursor(offset))
        } else {
            None
        };
        let end_cursor = if total > 0 {
            Some(encode_cursor((offset + limit).min(total) - 1))
        } else {
            None
        };

        Self {
            has_next_page: offset + limit < total,
            has_previous_page: offset > 0,
            start_cursor,
            end_cursor,
            total_count: total,
        }
    }
}

#[derive(InputObject, Debug, Clone, Default)]
pub struct PaginationInput {
    #[graphql(default = 0)]
    pub offset: i64,
    #[graphql(default = 20)]
    pub limit: i64,
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub after: Option<String>,
    pub before: Option<String>,
}

impl PaginationInput {
    pub fn requested_limit(&self) -> u64 {
        self.first.or(self.last).unwrap_or(self.limit).max(0) as u64
    }

    pub fn normalize(&self) -> Result<(i64, i64)> {
        if self.first.is_some() && self.last.is_some() {
            return Err("Provide only one of `first` or `last`".into());
        }

        const MAX_LIMIT: i64 = 100;
        let mut offset = self.offset.max(0);
        if let Some(ref cursor) = self.after {
            offset = decode_cursor(cursor).unwrap_or(-1) + 1;
        }

        if let Some(ref cursor) = self.before {
            let before = decode_cursor(cursor).unwrap_or(0);
            offset = offset.min(before.max(0));
        }

        let mut limit = self.limit.clamp(1, MAX_LIMIT);
        if let Some(first) = self.first {
            limit = first.clamp(1, MAX_LIMIT);
        }

        if let Some(last) = self.last {
            let last = last.clamp(1, MAX_LIMIT);
            if let Some(ref cursor) = self.before {
                let before = decode_cursor(cursor).unwrap_or(0).max(0);
                offset = (before - last).max(0);
                limit = last;
            }
        }

        Ok((offset.max(0), limit))
    }
}

pub fn encode_cursor(n: i64) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(n.to_string())
}

pub fn decode_cursor(s: &str) -> Option<i64> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(s)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|value| value.parse().ok())
}

pub async fn require_module_enabled(ctx: &Context<'_>, slug: &str) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let backend = db.get_database_backend();
    let query = match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT 1 FROM tenant_modules WHERE tenant_id = ?1 AND module_slug = ?2 AND enabled = 1 LIMIT 1"
        }
        _ => {
            "SELECT 1 FROM tenant_modules WHERE tenant_id = $1 AND module_slug = $2 AND enabled = true LIMIT 1"
        }
    };

    use sea_orm::{ConnectionTrait, Statement};
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            query,
            vec![tenant.id.into(), slug.into()],
        ))
        .await
        .map_err(|e| {
            async_graphql::Error::new(format!("Module check failed: {e}"))
                .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
        })?;

    let enabled = row.is_some();

    if !enabled {
        return Err(async_graphql::Error::new(format!(
            "Module '{slug}' is not enabled for this tenant"
        ))
        .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")));
    }

    Ok(())
}

/// Resolve an optional GraphQL tenant argument without allowing the client to
/// escape the tenant established by the HTTP/WS tenant resolver.
///
/// Permission snapshots, module enablement and auth claims are all bound to the
/// request tenant. Accepting a different resolver argument would reuse that
/// authority against another tenant's rows.
pub fn resolve_graphql_tenant_id(ctx: &Context<'_>, requested: Option<Uuid>) -> Result<Uuid> {
    let tenant = ctx.data::<TenantContext>()?;
    match requested {
        Some(requested) if requested != tenant.id => Err(async_graphql::Error::new(
            "tenantId does not match the authenticated request tenant",
        )
        .extend_with(|_, ext| ext.set("code", "TENANT_MISMATCH"))),
        Some(requested) => Ok(requested),
        None => Ok(tenant.id),
    }
}

pub fn resolve_graphql_locale(ctx: &Context<'_>, requested: Option<&str>) -> String {
    requested
        .map(str::trim)
        .filter(|locale| !locale.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            ctx.data_opt::<RequestContext>()
                .map(|request| request.locale.clone())
        })
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

#[cfg(test)]
mod tests {
    use super::resolve_graphql_tenant_id;
    use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema};
    use uuid::Uuid;

    struct Query;

    #[Object]
    impl Query {
        async fn resolved(
            &self,
            ctx: &Context<'_>,
            requested: Option<Uuid>,
        ) -> async_graphql::Result<Uuid> {
            resolve_graphql_tenant_id(ctx, requested)
        }
    }

    fn tenant(id: Uuid) -> crate::TenantContext {
        crate::TenantContext {
            id,
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    #[tokio::test]
    async fn rejects_cross_tenant_override() {
        let tenant_id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .data(tenant(tenant_id))
            .finish();
        let response = schema
            .execute(format!("{{ resolved(requested: \"{other}\") }}"))
            .await;

        assert!(!response.errors.is_empty());
    }
}
