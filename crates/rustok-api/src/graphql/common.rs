use std::sync::Arc;

use async_graphql::{Context, Error, ErrorExtensions, InputObject, Result, SimpleObject};
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
        let total = total.max(0);
        let offset = offset.max(0);
        let limit = limit.max(0);
        let page_end = offset.saturating_add(limit).min(total);
        let has_items = limit > 0 && offset < total;

        let start_cursor = has_items.then(|| encode_cursor(offset));
        let end_cursor = has_items.then(|| encode_cursor(page_end.saturating_sub(1)));

        Self {
            has_next_page: has_items && page_end < total,
            has_previous_page: total > 0 && offset > 0,
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
            return Err(pagination_input_error(
                "Provide only one of `first` or `last`",
            ));
        }

        validate_non_negative_pagination_value("offset", self.offset)?;
        validate_positive_pagination_value("limit", self.limit)?;
        if let Some(first) = self.first {
            validate_positive_pagination_value("first", first)?;
        }
        if let Some(last) = self.last {
            validate_positive_pagination_value("last", last)?;
        }

        let after = self
            .after
            .as_deref()
            .map(|cursor| decode_pagination_cursor(cursor, "after"))
            .transpose()?;
        let before = self
            .before
            .as_deref()
            .map(|cursor| decode_pagination_cursor(cursor, "before"))
            .transpose()?;

        const MAX_LIMIT: i64 = 100;
        let mut offset = self.offset;
        if let Some(after) = after {
            offset = after
                .checked_add(1)
                .ok_or_else(|| pagination_cursor_error("after"))?;
        }

        if let Some(before) = before {
            offset = offset.min(before);
        }

        let mut limit = self.limit.min(MAX_LIMIT);
        if let Some(first) = self.first {
            limit = first.min(MAX_LIMIT);
        }

        if let Some(last) = self.last {
            let last = last.min(MAX_LIMIT);
            if let Some(before) = before {
                offset = (before - last).max(0);
                limit = last;
            }
        }

        offset.checked_add(limit).ok_or_else(|| {
            pagination_input_error("Pagination offset and limit exceed supported range")
        })?;

        Ok((offset, limit))
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

fn decode_pagination_cursor(cursor: &str, argument: &'static str) -> Result<i64> {
    decode_cursor(cursor)
        .filter(|offset| *offset >= 0)
        .ok_or_else(|| pagination_cursor_error(argument))
}

fn pagination_cursor_error(argument: &'static str) -> Error {
    pagination_input_error(format!("Invalid `{argument}` pagination cursor"))
}

fn validate_non_negative_pagination_value(argument: &'static str, value: i64) -> Result<()> {
    if value < 0 {
        return Err(pagination_input_error(format!(
            "Pagination `{argument}` must be non-negative"
        )));
    }
    Ok(())
}

fn validate_positive_pagination_value(argument: &'static str, value: i64) -> Result<()> {
    if value <= 0 {
        return Err(pagination_input_error(format!(
            "Pagination `{argument}` must be greater than zero"
        )));
    }
    Ok(())
}

fn pagination_input_error(message: impl Into<String>) -> Error {
    Error::new(message.into()).extend_with(|_, ext| ext.set("code", "BAD_USER_INPUT"))
}

fn module_check_error(source: sea_orm::DbErr) -> Error {
    let mut error = Error::new("Module availability check failed")
        .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"));
    error.source = Some(Arc::new(source));
    error
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
        .map_err(module_check_error)?;

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
    use super::{
        PageInfo, PaginationInput, decode_cursor, encode_cursor, module_check_error,
        resolve_graphql_tenant_id,
    };
    use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Pos, Schema, Value};
    use sea_orm::DbErr;
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

    fn error_code(error: &async_graphql::Error) -> Option<&Value> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
    }

    #[test]
    fn page_info_avoids_overflow_and_out_of_range_cursors() {
        let beyond_end = PageInfo::new(10, i64::MAX, 100);
        assert_eq!(beyond_end.total_count, 10);
        assert_eq!(beyond_end.start_cursor, None);
        assert_eq!(beyond_end.end_cursor, None);
        assert!(!beyond_end.has_next_page);
        assert!(beyond_end.has_previous_page);

        let near_limit = PageInfo::new(i64::MAX, i64::MAX - 10, 100);
        assert_eq!(
            near_limit.start_cursor.as_deref().and_then(decode_cursor),
            Some(i64::MAX - 10)
        );
        assert_eq!(
            near_limit.end_cursor.as_deref().and_then(decode_cursor),
            Some(i64::MAX - 1)
        );
        assert!(!near_limit.has_next_page);
        assert!(near_limit.has_previous_page);

        let invalid_total = PageInfo::new(-1, 0, 20);
        assert_eq!(invalid_total.total_count, 0);
        assert_eq!(invalid_total.start_cursor, None);
        assert_eq!(invalid_total.end_cursor, None);
    }

    #[test]
    fn pagination_accepts_valid_forward_and_backward_cursors() {
        let forward = PaginationInput {
            first: Some(25),
            after: Some(encode_cursor(4)),
            ..Default::default()
        };
        assert_eq!(forward.normalize().expect("valid forward cursor"), (5, 25));

        let backward = PaginationInput {
            last: Some(5),
            before: Some(encode_cursor(20)),
            ..Default::default()
        };
        assert_eq!(
            backward.normalize().expect("valid backward cursor"),
            (15, 5)
        );
    }

    #[test]
    fn pagination_caps_large_page_sizes() {
        for input in [
            PaginationInput {
                limit: 500,
                ..Default::default()
            },
            PaginationInput {
                first: Some(500),
                ..Default::default()
            },
            PaginationInput {
                last: Some(500),
                before: Some(encode_cursor(500)),
                ..Default::default()
            },
        ] {
            assert_eq!(
                input.normalize().expect("large limit should be capped").1,
                100
            );
        }
    }

    #[test]
    fn pagination_rejects_invalid_numeric_arguments_as_bad_user_input() {
        let cases = [
            (
                "Pagination `offset` must be non-negative",
                PaginationInput {
                    offset: -1,
                    ..Default::default()
                },
            ),
            (
                "Pagination `limit` must be greater than zero",
                PaginationInput {
                    limit: 0,
                    ..Default::default()
                },
            ),
            (
                "Pagination `first` must be greater than zero",
                PaginationInput {
                    first: Some(0),
                    ..Default::default()
                },
            ),
            (
                "Pagination `last` must be greater than zero",
                PaginationInput {
                    last: Some(-1),
                    ..Default::default()
                },
            ),
        ];

        for (expected_message, input) in cases {
            let error = input
                .normalize()
                .expect_err("invalid numeric pagination input must fail");
            assert_eq!(error.message, expected_message);
            assert_eq!(error_code(&error), Some(&Value::from("BAD_USER_INPUT")));
        }
    }

    #[test]
    fn pagination_rejects_malformed_cursors_as_bad_user_input() {
        for (argument, input) in [
            (
                "after",
                PaginationInput {
                    after: Some("not-base64".to_string()),
                    ..Default::default()
                },
            ),
            (
                "before",
                PaginationInput {
                    before: Some(encode_cursor(-1)),
                    ..Default::default()
                },
            ),
        ] {
            let error = input.normalize().expect_err("invalid cursor must fail");
            assert_eq!(
                error.message,
                format!("Invalid `{argument}` pagination cursor")
            );
            assert_eq!(error_code(&error), Some(&Value::from("BAD_USER_INPUT")));
        }
    }

    #[test]
    fn pagination_rejects_after_cursor_offset_overflow() {
        let error = PaginationInput {
            after: Some(encode_cursor(i64::MAX)),
            ..Default::default()
        }
        .normalize()
        .expect_err("overflowing cursor must fail");

        assert_eq!(error.message, "Invalid `after` pagination cursor");
        assert_eq!(error_code(&error), Some(&Value::from("BAD_USER_INPUT")));
    }

    #[test]
    fn pagination_rejects_offset_limit_overflow() {
        let error = PaginationInput {
            offset: i64::MAX,
            limit: 100,
            ..Default::default()
        }
        .normalize()
        .expect_err("overflowing page range must fail");

        assert_eq!(
            error.message,
            "Pagination offset and limit exceed supported range"
        );
        assert_eq!(error_code(&error), Some(&Value::from("BAD_USER_INPUT")));
    }

    #[test]
    fn pagination_rejects_conflicting_direction_limits_with_code() {
        let error = PaginationInput {
            first: Some(10),
            last: Some(10),
            ..Default::default()
        }
        .normalize()
        .expect_err("first and last together must fail");

        assert_eq!(error.message, "Provide only one of `first` or `last`");
        assert_eq!(error_code(&error), Some(&Value::from("BAD_USER_INPUT")));
    }

    #[test]
    fn module_check_errors_keep_source_but_redact_public_message() {
        let secret = "password=private host=internal";
        let graphql_error = module_check_error(DbErr::Custom(secret.to_string()));

        assert_eq!(graphql_error.message, "Module availability check failed");
        assert!(!graphql_error.message.contains(secret));

        let server_error = graphql_error.into_server_error(Pos::default());
        assert!(server_error.source::<DbErr>().is_some());
        assert_eq!(
            server_error
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.get("code")),
            Some(&Value::from("INTERNAL_SERVER_ERROR"))
        );
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
