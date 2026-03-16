use async_graphql::{Context, ErrorExtensions, InputObject, Result, SimpleObject};
use rustok_content::PLATFORM_FALLBACK_LOCALE;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::common::RequestContext;
use crate::context::TenantContext;
use crate::models::_entities::tenant_modules;

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
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(n.to_string())
}

pub fn decode_cursor(s: &str) -> Option<i64> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(s)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|value| value.parse().ok())
}

/// Guard that returns a GraphQL error when the given module is not enabled for
/// the current tenant.  Call this at the top of every resolver exposed by an
/// optional domain module.
///
/// ```rust,ignore
/// async fn posts(&self, ctx: &Context<'_>, ...) -> Result<...> {
///     require_module_enabled(ctx, module_slug::BLOG).await?;
///     // ...
/// }
/// ```
pub async fn require_module_enabled(ctx: &Context<'_>, slug: &str) -> Result<()> {
    let db = ctx.data::<DatabaseConnection>()?;
    let tenant = ctx.data::<TenantContext>()?;

    let enabled = tenant_modules::Entity::is_enabled(db, tenant.id, slug)
        .await
        .map_err(|e| {
            async_graphql::Error::new(format!("Module check failed: {e}"))
                .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
        })?;

    if !enabled {
        return Err(async_graphql::Error::new(format!(
            "Module '{slug}' is not enabled for this tenant"
        ))
        .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")));
    }

    Ok(())
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
