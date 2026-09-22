use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumCounterDrift, ForumCounterReconciliationReport, ForumCounterReconciliationService,
    services::{
        ForumSolutionDrift, ForumSolutionReconciliationReport, ForumSolutionReconciliationService,
    },
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
    pub topic_cursor: Option<Uuid>,
    pub category_cursor: Option<Uuid>,
    pub drift_count: i32,
    /// True only when the current bounded page contains no detected drift. Whole-tenant clean
    /// requires exhausting both cursor chains with every page clean.
    pub clean: bool,
    pub drifts: Vec<GqlForumCounterDrift>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSolutionDrift {
    pub kind: String,
    pub subject_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSolutionReconciliationReport {
    pub requested_limit: Option<i32>,
    pub effective_limit: i32,
    pub inspected_solutions: i32,
    pub inspected_solution_stats: i32,
    pub has_more_solutions: bool,
    pub has_more_solution_stats: bool,
    pub solution_cursor: Option<Uuid>,
    pub solution_stat_cursor: Option<Uuid>,
    pub drift_count: i32,
    /// True only when the current bounded accepted-solution/stat page contains no detected drift.
    /// Whole-tenant clean requires exhausting both cursor chains with every page clean.
    pub clean: bool,
    pub drifts: Vec<GqlForumSolutionDrift>,
}

#[derive(Default)]
pub struct ForumReconciliationQuery;

#[Object]
impl ForumReconciliationQuery {
    /// Read-only FORUM-33 owner counter drift report for the current tenant.
    ///
    /// `topic_after` and `category_after` are independent keyset cursors. Callers should echo each
    /// returned cursor into the corresponding `*_after` argument on the next page, including the
    /// cursor for a shape whose `has_more_*` flag is already false, so that shape is not rescanned.
    /// `clean` describes only the current bounded page; whole-tenant clean requires exhausting both
    /// cursor chains with every page clean.
    /// This is intentionally not a repair mutation. Repair remains closed until a write path can
    /// provide operator RBAC, dry-run, audit and durable idempotent job state.
    async fn forum_counter_reconciliation_report(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        topic_after: Option<Uuid>,
        category_after: Option<Uuid>,
    ) -> Result<GqlForumCounterReconciliationReport> {
        let (tenant_id, security, requested_limit, db) = reconciliation_context(ctx, limit).await?;
        let report = ForumCounterReconciliationService::new(db)
            .report_page(
                tenant_id,
                &security,
                requested_limit,
                topic_after,
                category_after,
            )
            .await?;
        Ok(map_report(report))
    }

    /// Read-only FORUM-33 accepted-solution and solution-author-stat drift report.
    ///
    /// A solution is authoritative only when the exact same-tenant/topic reply still exists and is
    /// `approved`. `forum_user_stats.solution_count` is reconciled as a projection of those approved
    /// solution rows. `solution_after` and `solution_stat_after` are independent UUID keyset cursors.
    /// `clean` is page-local and is not whole-tenant proof until both cursor chains are exhausted.
    async fn forum_solution_reconciliation_report(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        solution_after: Option<Uuid>,
        solution_stat_after: Option<Uuid>,
    ) -> Result<GqlForumSolutionReconciliationReport> {
        let (tenant_id, security, requested_limit, db) = reconciliation_context(ctx, limit).await?;
        let report = ForumSolutionReconciliationService::new(db)
            .report_page(
                tenant_id,
                &security,
                requested_limit,
                solution_after,
                solution_stat_after,
            )
            .await?;
        Ok(map_solution_report(report))
    }
}

async fn reconciliation_context(
    ctx: &Context<'_>,
    limit: Option<i32>,
) -> Result<(Uuid, SecurityContext, Option<u64>, DatabaseConnection)> {
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
    let db = ctx.data::<DatabaseConnection>()?.clone();
    let security = SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
    Ok((tenant.id, security, requested_limit, db))
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

fn map_report(report: ForumCounterReconciliationReport) -> GqlForumCounterReconciliationReport {
    let requested_limit = report.requested_limit.map(saturating_i32);
    GqlForumCounterReconciliationReport {
        requested_limit,
        effective_limit: saturating_i32(report.effective_limit),
        inspected_topics: saturating_i32(report.inspected_topics),
        inspected_categories: saturating_i32(report.inspected_categories),
        has_more_topics: report.has_more_topics,
        has_more_categories: report.has_more_categories,
        topic_cursor: report.topic_cursor,
        category_cursor: report.category_cursor,
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

fn map_solution_report(
    report: ForumSolutionReconciliationReport,
) -> GqlForumSolutionReconciliationReport {
    let requested_limit = report.requested_limit.map(saturating_i32);
    GqlForumSolutionReconciliationReport {
        requested_limit,
        effective_limit: saturating_i32(report.effective_limit),
        inspected_solutions: saturating_i32(report.inspected_solutions),
        inspected_solution_stats: saturating_i32(report.inspected_solution_stats),
        has_more_solutions: report.has_more_solutions,
        has_more_solution_stats: report.has_more_solution_stats,
        solution_cursor: report.solution_cursor,
        solution_stat_cursor: report.solution_stat_cursor,
        drift_count: saturating_i32(report.drift_count() as u64),
        clean: report.is_clean(),
        drifts: report.drifts.into_iter().map(map_solution_drift).collect(),
    }
}

fn map_solution_drift(drift: ForumSolutionDrift) -> GqlForumSolutionDrift {
    GqlForumSolutionDrift {
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

    #[test]
    fn mapped_report_preserves_independent_cursors() {
        let topic_cursor = Uuid::new_v4();
        let category_cursor = Uuid::new_v4();
        let mapped = map_report(ForumCounterReconciliationReport {
            requested_limit: Some(25),
            effective_limit: 25,
            inspected_topics: 25,
            inspected_categories: 4,
            has_more_topics: true,
            has_more_categories: false,
            topic_cursor: Some(topic_cursor),
            category_cursor: Some(category_cursor),
            drifts: Vec::new(),
        });
        assert_eq!(mapped.topic_cursor, Some(topic_cursor));
        assert_eq!(mapped.category_cursor, Some(category_cursor));
    }

    #[test]
    fn mapped_solution_report_preserves_independent_cursors() {
        let solution_cursor = Uuid::new_v4();
        let solution_stat_cursor = Uuid::new_v4();
        let mapped = map_solution_report(ForumSolutionReconciliationReport {
            requested_limit: Some(25),
            effective_limit: 25,
            inspected_solutions: 25,
            inspected_solution_stats: 8,
            has_more_solutions: true,
            has_more_solution_stats: false,
            solution_cursor: Some(solution_cursor),
            solution_stat_cursor: Some(solution_stat_cursor),
            drifts: Vec::new(),
        });
        assert_eq!(mapped.solution_cursor, Some(solution_cursor));
        assert_eq!(mapped.solution_stat_cursor, Some(solution_stat_cursor));
    }
}
