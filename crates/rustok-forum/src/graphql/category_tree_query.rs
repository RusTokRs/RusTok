use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::Permission;
use rustok_api::{
    graphql::{require_module_enabled, resolve_graphql_locale, GraphQLError},
    has_any_effective_permission, AuthContext, TenantContext,
};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::{
    CategoryBreadcrumb, CategoryService, CategoryTreeNode, CategoryTreeQuery,
    MAX_FORUM_CATEGORY_TREE_NODES,
};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumCategoryTreeQuery;

#[Object]
impl ForumCategoryTreeQuery {
    async fn forum_category_tree(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        locale: Option<String>,
        fallback_locale: Option<String>,
    ) -> Result<GqlForumCategoryTree> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_category_list_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let requested_locale = resolve_graphql_locale(ctx, locale.as_deref());
        let fallback_locale = fallback_locale.unwrap_or_else(|| tenant.default_locale.clone());

        let started_at = Instant::now();
        let tree = CategoryService::new(db.clone())
            .tree(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                CategoryTreeQuery {
                    locale: Some(requested_locale),
                    fallback_locale: Some(fallback_locale),
                },
            )
            .await?;

        metrics::record_read_path_query(
            "graphql",
            "forum.category_tree",
            "service_tree",
            started_at.elapsed().as_secs_f64(),
            tree.total_nodes as u64,
        );
        metrics::record_read_path_budget(
            "graphql",
            "forum.category_tree",
            None,
            MAX_FORUM_CATEGORY_TREE_NODES,
            tree.total_nodes as usize,
        );

        Ok(tree.into())
    }
}

fn require_category_list_permission(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_CATEGORIES_LIST]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_categories:list required",
        ));
    }

    Ok(auth)
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
    }
}

#[derive(SimpleObject)]
pub struct GqlForumCategoryBreadcrumb {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
}

#[derive(SimpleObject)]
pub struct GqlForumCategoryTreeNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub depth: i32,
    pub position: i32,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub moderated: bool,
    pub topic_count: i32,
    pub reply_count: i32,
    pub is_subscribed: bool,
    pub has_children: bool,
    pub children_count: i32,
    pub breadcrumbs: Vec<GqlForumCategoryBreadcrumb>,
    pub children: Vec<GqlForumCategoryTreeNode>,
}

#[derive(SimpleObject)]
pub struct GqlForumCategoryTree {
    pub roots: Vec<GqlForumCategoryTreeNode>,
    pub total_nodes: i32,
    pub max_depth: i32,
}

impl From<CategoryBreadcrumb> for GqlForumCategoryBreadcrumb {
    fn from(value: CategoryBreadcrumb) -> Self {
        Self {
            id: value.id,
            name: value.name,
            slug: value.slug,
        }
    }
}

impl From<CategoryTreeNode> for GqlForumCategoryTreeNode {
    fn from(value: CategoryTreeNode) -> Self {
        Self {
            id: value.id,
            parent_id: value.parent_id,
            depth: i32::from(value.depth),
            position: value.position,
            requested_locale: value.requested_locale,
            effective_locale: value.effective_locale,
            available_locales: value.available_locales,
            name: value.name,
            slug: value.slug,
            description: value.description,
            icon: value.icon,
            color: value.color,
            moderated: value.moderated,
            topic_count: value.topic_count,
            reply_count: value.reply_count,
            is_subscribed: value.is_subscribed,
            has_children: value.has_children,
            children_count: value.children_count as i32,
            breadcrumbs: value.breadcrumbs.into_iter().map(Into::into).collect(),
            children: value.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<crate::CategoryTreeResponse> for GqlForumCategoryTree {
    fn from(value: crate::CategoryTreeResponse) -> Self {
        Self {
            roots: value.roots.into_iter().map(Into::into).collect(),
            total_nodes: value.total_nodes as i32,
            max_depth: i32::from(value.max_depth),
        }
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::{EmptyMutation, EmptySubscription, Schema};

    use crate::graphql::ForumQuery;

    #[test]
    fn schema_exposes_recursive_forum_category_tree() {
        let schema =
            Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription).finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("forumCategoryTree"));
        assert!(sdl.contains("type GqlForumCategoryTree"));
        assert!(sdl.contains("type GqlForumCategoryTreeNode"));
        assert!(sdl.contains("children: [GqlForumCategoryTreeNode!]!"));
        assert!(sdl.contains("breadcrumbs: [GqlForumCategoryBreadcrumb!]!"));
    }
}
