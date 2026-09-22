use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::mention_reconciliation::{
    ForumMentionDrift, ForumMentionReconciliationReport, ForumMentionReconciliationService,
};

const MODULE_SLUG: &str = "forum";

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumMentionDrift {
    pub kind: String,
    pub revision_id: String,
    pub source_kind: String,
    pub source_id: Uuid,
    pub source_locale: String,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumMentionReconciliationReport {
    pub requested_limit: Option<i32>,
    pub effective_limit: i32,
    pub inspected_relation_revisions: i32,
    pub inspected_mention_revisions: i32,
    pub has_more_relation_revisions: bool,
    /// Decimal relation-revision keyset cursor. It is a string because owner revision IDs are i64
    /// while GraphQL Int is intentionally not used as the durable cursor representation.
    pub relation_cursor: Option<String>,
    pub drift_count: i32,
    /// True only when the current bounded relation-revision page has no detected mention drift.
    /// Whole-tenant clean requires exhausting the cursor chain with every page clean.
    pub clean: bool,
    pub drifts: Vec<GqlForumMentionDrift>,
}

#[derive(Default)]
pub struct ForumMentionReconciliationQuery;

#[Object]
impl ForumMentionReconciliationQuery {
    /// Read-only FORUM-33 mention projection drift report for the current tenant.
    ///
    /// `relation_after` is a positive decimal relation-revision ID returned by the previous page.
    /// The report uses Forum-owned relation/mention rows only; it does not re-resolve Profiles or
    /// inspect Notifications-owned delivery state and performs no repair.
    async fn forum_mention_reconciliation_report(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        relation_after: Option<String>,
    ) -> Result<GqlForumMentionReconciliationReport> {
        let (tenant_id, security, requested_limit, relation_after, db) =
            reconciliation_context(ctx, limit, relation_after).await?;
        let report = ForumMentionReconciliationService::new(db)
            .report_page(tenant_id, &security, requested_limit, relation_after)
            .await?;
        Ok(map_report(report))
    }
}

async fn reconciliation_context(
    ctx: &Context<'_>,
    limit: Option<i32>,
    relation_after: Option<String>,
) -> Result<(
    Uuid,
    SecurityContext,
    Option<u64>,
    Option<i64>,
    DatabaseConnection,
)> {
    require_module_enabled(ctx, MODULE_SLUG).await?;
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    require_operations_permissions(auth)?;
    let tenant = ctx.data::<TenantContext>()?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: tenant scope mismatch",
        ));
    }
    let requested_limit = normalize_limit(limit)?;
    let relation_after = normalize_relation_cursor(relation_after)?;
    let db = ctx.data::<DatabaseConnection>()?.clone();
    let security = SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
    Ok((tenant.id, security, requested_limit, relation_after, db))
}

fn require_operations_permissions(auth: &AuthContext) -> Result<()> {
    let categories_manage =
        has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_MANAGE]);
    let topics_manage =
        has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_MANAGE]);
    if categories_manage && topics_manage {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_categories:manage and forum_topics:manage required",
        ))
    }
}

fn normalize_limit(limit: Option<i32>) -> Result<Option<u64>> {
    match limit {
        None => Ok(None),
        Some(value) if value > 0 => Ok(Some(value as u64)),
        Some(_) => Err(async_graphql::Error::new(
            "Forum reconciliation limit must be positive",
        )),
    }
}

fn normalize_relation_cursor(cursor: Option<String>) -> Result<Option<i64>> {
    match cursor {
        None => Ok(None),
        Some(value) => value
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or_else(|| {
                async_graphql::Error::new(
                    "Forum mention reconciliation cursor must be a positive decimal revision ID",
                )
            }),
    }
}

fn map_report(report: ForumMentionReconciliationReport) -> GqlForumMentionReconciliationReport {
    GqlForumMentionReconciliationReport {
        requested_limit: report.requested_limit.map(saturating_i32),
        effective_limit: saturating_i32(report.effective_limit),
        inspected_relation_revisions: saturating_i32(report.inspected_relation_revisions),
        inspected_mention_revisions: saturating_i32(report.inspected_mention_revisions),
        has_more_relation_revisions: report.has_more_relation_revisions,
        relation_cursor: report.relation_cursor.map(|value| value.to_string()),
        drift_count: saturating_i32(report.drift_count() as u64),
        clean: report.is_clean(),
        drifts: report.drifts.into_iter().map(map_drift).collect(),
    }
}

fn map_drift(drift: ForumMentionDrift) -> GqlForumMentionDrift {
    GqlForumMentionDrift {
        kind: drift.kind.as_str().to_string(),
        revision_id: drift.revision_id.to_string(),
        source_kind: drift.source_kind,
        source_id: drift.source_id,
        source_locale: drift.source_locale,
        stored: drift.stored,
        expected: drift.expected,
    }
}

fn saturating_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Resource};

    fn auth(permissions: Vec<Permission>) -> AuthContext {
        let tenant_id = Uuid::new_v4();
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn mention_report_requires_both_effective_manage_permissions() {
        let both = auth(vec![
            Permission::new(Resource::ForumCategories, Action::Manage),
            Permission::new(Resource::ForumTopics, Action::Manage),
        ]);
        assert!(require_operations_permissions(&both).is_ok());

        let topics_only = auth(vec![Permission::new(Resource::ForumTopics, Action::Manage)]);
        assert!(require_operations_permissions(&topics_only).is_err());
    }

    #[test]
    fn relation_cursor_is_positive_decimal_i64() {
        assert_eq!(normalize_relation_cursor(None).unwrap(), None);
        assert_eq!(
            normalize_relation_cursor(Some("9223372036854775807".to_string())).unwrap(),
            Some(i64::MAX)
        );
        assert!(normalize_relation_cursor(Some("0".to_string())).is_err());
        assert!(normalize_relation_cursor(Some("not-a-cursor".to_string())).is_err());
    }
}
