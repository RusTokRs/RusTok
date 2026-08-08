use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::services::subscription::reconciliation::{
    ForumSubscriptionCursor, ForumSubscriptionDrift, ForumSubscriptionReconciliationReport,
    ForumSubscriptionReconciliationService,
};

const MODULE_SLUG: &str = "forum";

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSubscriptionCursor {
    pub target_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSubscriptionDrift {
    pub kind: String,
    pub target_kind: String,
    pub target_id: Uuid,
    pub user_id: Uuid,
    pub stored: i64,
    pub expected: i64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct GqlForumSubscriptionReconciliationReport {
    pub requested_limit: Option<i32>,
    pub effective_limit: i32,
    pub inspected_topic_subscriptions: i32,
    pub inspected_category_subscriptions: i32,
    pub has_more_topic_subscriptions: bool,
    pub has_more_category_subscriptions: bool,
    pub topic_cursor: Option<GqlForumSubscriptionCursor>,
    pub category_cursor: Option<GqlForumSubscriptionCursor>,
    pub drift_count: i32,
    /// True only when the current bounded topic/category subscription page has no detected drift.
    /// Whole-tenant clean requires exhausting both composite cursor chains with every page clean.
    pub clean: bool,
    pub drifts: Vec<GqlForumSubscriptionDrift>,
}

#[derive(Default)]
pub struct ForumSubscriptionReconciliationQuery;

#[Object]
impl ForumSubscriptionReconciliationQuery {
    /// Read-only FORUM-33 subscription-owner drift report for the current tenant.
    ///
    /// Topic and category rows use independent composite `(target_id, user_id)` keyset cursors.
    /// Callers must echo both components of a returned cursor together on the next page. The report
    /// does not infer missing subscriptions from participation policy and does not perform repair.
    #[allow(clippy::too_many_arguments)]
    async fn forum_subscription_reconciliation_report(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        topic_after: Option<Uuid>,
        topic_user_after: Option<Uuid>,
        category_after: Option<Uuid>,
        category_user_after: Option<Uuid>,
    ) -> Result<GqlForumSubscriptionReconciliationReport> {
        let (tenant_id, security, requested_limit, db) =
            reconciliation_context(ctx, limit).await?;
        let report = ForumSubscriptionReconciliationService::new(db)
            .report_page(
                tenant_id,
                &security,
                requested_limit,
                topic_after,
                topic_user_after,
                category_after,
                category_user_after,
            )
            .await?;
        Ok(map_report(report))
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
    let security =
        SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions);
    Ok((tenant.id, security, requested_limit, db))
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
            "Forum reconciliation limit must be positive",
        )),
    }
}

fn map_report(
    report: ForumSubscriptionReconciliationReport,
) -> GqlForumSubscriptionReconciliationReport {
    GqlForumSubscriptionReconciliationReport {
        requested_limit: report.requested_limit.map(saturating_i32),
        effective_limit: saturating_i32(report.effective_limit),
        inspected_topic_subscriptions: saturating_i32(report.inspected_topic_subscriptions),
        inspected_category_subscriptions: saturating_i32(report.inspected_category_subscriptions),
        has_more_topic_subscriptions: report.has_more_topic_subscriptions,
        has_more_category_subscriptions: report.has_more_category_subscriptions,
        topic_cursor: report.topic_cursor.map(map_cursor),
        category_cursor: report.category_cursor.map(map_cursor),
        drift_count: saturating_i32(report.drift_count() as u64),
        clean: report.is_clean(),
        drifts: report.drifts.into_iter().map(map_drift).collect(),
    }
}

fn map_cursor(cursor: ForumSubscriptionCursor) -> GqlForumSubscriptionCursor {
    GqlForumSubscriptionCursor {
        target_id: cursor.target_id,
        user_id: cursor.user_id,
    }
}

fn map_drift(drift: ForumSubscriptionDrift) -> GqlForumSubscriptionDrift {
    GqlForumSubscriptionDrift {
        kind: drift.kind.as_str().to_string(),
        target_kind: drift.target_kind.as_str().to_string(),
        target_id: drift.target_id,
        user_id: drift.user_id,
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
    fn subscription_report_requires_both_effective_manage_permissions() {
        let both = auth(vec![
            Permission::new(Resource::ForumCategories, Action::Manage),
            Permission::new(Resource::ForumTopics, Action::Manage),
        ]);
        assert!(require_operations_permissions(&both).is_ok());

        let topics_only = auth(vec![Permission::new(Resource::ForumTopics, Action::Manage)]);
        assert!(require_operations_permissions(&topics_only).is_err());
    }

    #[test]
    fn mapped_subscription_report_preserves_composite_cursors() {
        let topic_cursor = ForumSubscriptionCursor {
            target_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
        };
        let category_cursor = ForumSubscriptionCursor {
            target_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
        };
        let mapped = map_report(ForumSubscriptionReconciliationReport {
            requested_limit: Some(25),
            effective_limit: 25,
            inspected_topic_subscriptions: 25,
            inspected_category_subscriptions: 4,
            has_more_topic_subscriptions: true,
            has_more_category_subscriptions: false,
            topic_cursor: Some(topic_cursor),
            category_cursor: Some(category_cursor),
            drifts: Vec::new(),
        });
        assert_eq!(mapped.topic_cursor.unwrap().target_id, topic_cursor.target_id);
        assert_eq!(
            mapped.category_cursor.unwrap().user_id,
            category_cursor.user_id
        );
    }
}
