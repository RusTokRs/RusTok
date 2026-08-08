use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumCounterDrift, ForumCounterReconciliationReport, ForumCounterReconciliationService,
};

const MODULE_SLUG: &str = "forum";

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumCounterDrift {
    pub kind: String,
    pub subject_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumCounterReconciliationReport {
    pub requested_limit: Option<i32>,
    pub effective_limit: i32,
    pub inspected_topics: i32,
    pub inspected_categories: i32,
    pub has_more_topics: bool,
    pub has_more_categories: bool,
    pub drift_count: i32,
    pub clean: bool,
    pub drifts: Vec<GqlForumCounterDrift>,
}

#[derive(Default)]
pub struct ForumReconciliationQuery;

#[Object]
impl ForumReconciliationQuery {
    /// Read-only FORUM-33 owner counter drift report for the current tenant.
    ///
    /// This is intentionally not a repair mutation. Repair remains closed until a write path can
    /// provide operator RBAC, dry-run, audit and durable idempotent job state.
    async fn forum_counter_reconciliation_report(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<GqlForumCounterReconciliationReport> {
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
        let db = ctx.data::<DatabaseConnection>()?;
        let report = ForumCounterReconciliationService::new(db.clone())
            .report(tenant.id, requested_limit)
            .await?;
        Ok(map_report(report))
    }
}

fn require_operations_permissions(auth: &AuthContext) -> Result<()> {
    let categories_manage = has_any_effective_permission(
        &auth.permissions,
        &[Permission::FORUM_CATEGORIES_MANAGE],
    );
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
            "Forum counter reconciliation limit must be positive",
        )),
    }
}

fn map_report(report: ForumCounterReconciliationReport) -> GqlForumCounterReconciliationReport {
    let requested_limit = report.requested_limit.map(saturating_i32);
    GqlForumCounterReconciliationReport {
        requested_limit,
        effective_limit: saturating_i32(report.effective_limit),
        inspected_topics: saturating_i32(report.inspected_topics),
        inspected_categories: saturating_i32(report.inspected_categories),
        has_more_topics: report.has_more_topics,
        has_more_categories: report.has_more_categories,
        drift_count: saturating_i32(report.drift_count() as u64),
        clean: report.is_clean(),
        drifts: report.drifts.into_iter().map(map_drift).collect(),
    }
}

fn map_drift(drift: ForumCounterDrift) -> GqlForumCounterDrift {
    GqlForumCounterDrift {
        kind: drift.kind.as_str().to_string(),
        subject_id: drift.subject_id,
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
    fn operator_report_requires_both_effective_manage_permissions() {
        let both = auth(vec![
            Permission::new(Resource::ForumCategories, Action::Manage),
            Permission::new(Resource::ForumTopics, Action::Manage),
        ]);
        assert!(require_operations_permissions(&both).is_ok());

        let topics_only = auth(vec![Permission::new(Resource::ForumTopics, Action::Manage)]);
        assert!(require_operations_permissions(&topics_only).is_err());

        let read_only = auth(vec![
            Permission::FORUM_CATEGORIES_READ,
            Permission::FORUM_TOPICS_READ,
        ]);
        assert!(require_operations_permissions(&read_only).is_err());
    }

    #[test]
    fn report_limit_rejects_non_positive_values() {
        assert_eq!(normalize_limit(None).unwrap(), None);
        assert_eq!(normalize_limit(Some(25)).unwrap(), Some(25));
        assert!(normalize_limit(Some(0)).is_err());
        assert!(normalize_limit(Some(-1)).is_err());
    }
}
