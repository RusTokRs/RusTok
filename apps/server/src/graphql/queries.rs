use async_graphql::{Context, FieldError, Object, Result};
use chrono::{Duration, Utc};
use sea_orm::{
    sea_query::Expr, ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use std::collections::HashSet;

use crate::context::{AuthContext, TenantContext};
use crate::graphql::common::{encode_cursor, PageInfo, PaginationInput};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::{
    ActivityItem, ActivityUser, DashboardStats, ModuleRegistryItem, Tenant, TenantModule, User,
    UserConnection, UserEdge, UsersFilter,
};
use crate::models::_entities::tenant_modules::Column as TenantModulesColumn;
use crate::models::_entities::tenant_modules::Entity as TenantModulesEntity;
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::users;
use crate::services::auth::AuthService;
use rustok_content::entities::node::{Column as NodesColumn, Entity as NodesEntity};
use rustok_core::ModuleRegistry;
use rustok_outbox::entity::{Column as SysEventsColumn, Entity as SysEventsEntity};

fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 {
            0.0
        } else {
            100.0
        }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

fn parse_order_total(payload: &serde_json::Value) -> Option<i64> {
    payload
        .get("event")
        .and_then(|event| event.get("data"))
        .and_then(|data| data.get("total"))
        .and_then(serde_json::Value::as_i64)
}

#[derive(Default)]
pub struct RootQuery;

#[Object]
impl RootQuery {
    async fn health(&self) -> &str {
        "GraphQL is working!"
    }

    async fn api_version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    async fn current_tenant(&self, ctx: &Context<'_>) -> Result<Tenant> {
        let tenant = ctx.data::<TenantContext>()?;
        Ok(Tenant {
            id: tenant.id,
            name: tenant.name.clone(),
            slug: tenant.slug.clone(),
        })
    }

    async fn enabled_modules(&self, ctx: &Context<'_>) -> Result<Vec<String>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let modules = TenantModulesEntity::find_enabled(&app_ctx.db, tenant.id)
            .await
            .map_err(|err| err.to_string())?;

        Ok(modules)
    }

    async fn module_registry(&self, ctx: &Context<'_>) -> Result<Vec<ModuleRegistryItem>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;
        let enabled_modules = TenantModulesEntity::find_enabled(&app_ctx.db, tenant.id)
            .await
            .map_err(|err| err.to_string())?;
        let enabled_set: HashSet<String> = enabled_modules.into_iter().collect();

        Ok(registry
            .list()
            .into_iter()
            .map(|module| ModuleRegistryItem {
                module_slug: module.slug().to_string(),
                name: module.name().to_string(),
                description: module.description().to_string(),
                version: module.version().to_string(),
                kind: if registry.is_core(module.slug()) { "core".to_string() } else { "optional".to_string() },
                enabled: registry.is_core(module.slug()) || enabled_set.contains(module.slug()),
                dependencies: module
                    .dependencies()
                    .iter()
                    .map(|dependency| dependency.to_string())
                    .collect(),
            })
            .collect())
    }

    async fn tenant_modules(&self, ctx: &Context<'_>) -> Result<Vec<TenantModule>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let modules = TenantModulesEntity::find()
            .filter(TenantModulesColumn::TenantId.eq(tenant.id))
            .all(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        Ok(modules
            .into_iter()
            .map(|module| TenantModule {
                module_slug: module.module_slug,
                enabled: module.enabled,
                settings: module.settings.to_string(),
            })
            .collect())
    }

    async fn me(&self, ctx: &Context<'_>) -> Result<Option<User>> {
        let auth = match ctx.data_opt::<AuthContext>() {
            Some(auth) => auth,
            None => return Ok(None),
        };
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let user = users::Entity::find()
            .filter(UsersColumn::Id.eq(auth.user_id))
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| err.to_string())?;

        Ok(user.as_ref().map(User::from))
    }

    async fn user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<Option<User>> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let can_read_users = AuthService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_READ,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_read_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:read required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(user.as_ref().map(User::from))
    }

    async fn users(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] pagination: PaginationInput,
        filter: Option<UsersFilter>,
        search: Option<String>,
    ) -> Result<UserConnection> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;

        let can_list_users = AuthService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_LIST,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_list_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:list required",
            ));
        }

        let (offset, limit) = pagination.normalize()?;
        let mut query = users::Entity::find().filter(UsersColumn::TenantId.eq(tenant.id));

        if let Some(filter) = filter {
            if let Some(role) = filter.role {
                let role: rustok_core::UserRole = role.into();
                query = query.filter(UsersColumn::Role.eq(role.to_string()));
            }

            if let Some(status) = filter.status {
                let status: rustok_core::UserStatus = status.into();
                query = query.filter(UsersColumn::Status.eq(status.to_string()));
            }
        }

        if let Some(search) = search {
            let search = search.trim();
            if !search.is_empty() {
                let condition = Condition::any()
                    .add(UsersColumn::Email.contains(search))
                    .add(UsersColumn::Name.contains(search));
                query = query.filter(condition);
            }
        }
        let total = query
            .clone()
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;
        let users = query
            .offset(offset as u64)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let edges = users
            .iter()
            .enumerate()
            .map(|(index, user)| UserEdge {
                node: User::from(user),
                cursor: encode_cursor(offset + index as i64),
            })
            .collect();

        Ok(UserConnection {
            edges,
            page_info: PageInfo::new(total, offset, limit),
        })
    }

    async fn dashboard_stats(&self, ctx: &Context<'_>) -> Result<DashboardStats> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let total_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let total_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let current_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .filter(UsersColumn::CreatedAt.gte(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let previous_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .filter(UsersColumn::CreatedAt.gte(previous_period_start))
            .filter(UsersColumn::CreatedAt.lt(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let current_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .filter(NodesColumn::CreatedAt.gte(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let previous_posts = NodesEntity::find()
            .filter(NodesColumn::TenantId.eq(tenant.id))
            .filter(NodesColumn::Kind.eq("post"))
            .filter(NodesColumn::CreatedAt.gte(previous_period_start))
            .filter(NodesColumn::CreatedAt.lt(current_period_start))
            .count(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            as i64;

        let tenant_id_str = tenant.id.to_string();

        let order_events = SysEventsEntity::find()
            .filter(SysEventsColumn::EventType.eq("order.placed"))
            .filter(
                Condition::any()
                    .add(Expr::cust_with_values(
                        "payload->>'tenant_id' = $1",
                        [tenant_id_str.clone()],
                    ))
                    .add(Expr::cust_with_values(
                        "payload->'event'->>'tenant_id' = $1",
                        [tenant_id_str],
                    )),
            )
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let mut total_orders = 0i64;
        let mut total_revenue = 0i64;
        let mut current_orders = 0i64;
        let mut previous_orders = 0i64;
        let mut current_revenue = 0i64;
        let mut previous_revenue = 0i64;

        for event in order_events {
            let order_total = parse_order_total(&event.payload).unwrap_or(0);
            total_orders += 1;
            total_revenue += order_total;

            let created_at = event.created_at;
            if created_at >= current_period_start {
                current_orders += 1;
                current_revenue += order_total;
            } else if created_at >= previous_period_start {
                previous_orders += 1;
                previous_revenue += order_total;
            }
        }

        Ok(DashboardStats {
            total_users,
            total_posts,
            total_orders,
            total_revenue,
            users_change: calculate_percent_change(current_users, previous_users),
            posts_change: calculate_percent_change(current_posts, previous_posts),
            orders_change: calculate_percent_change(current_orders, previous_orders),
            revenue_change: calculate_percent_change(current_revenue, previous_revenue),
        })
    }

    async fn recent_activity(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] limit: i64,
    ) -> Result<Vec<ActivityItem>> {
        let app_ctx = ctx.data::<loco_rs::prelude::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let limit = limit.clamp(1, 50);

        let recent_users = users::Entity::find()
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .order_by_desc(UsersColumn::CreatedAt)
            .limit(limit as u64)
            .all(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let activities = recent_users
            .into_iter()
            .map(|user| ActivityItem {
                id: user.id.to_string(),
                r#type: "user.created".to_string(),
                description: format!("New user {} joined", user.email),
                timestamp: user.created_at.to_rfc3339(),
                user: Some(ActivityUser {
                    id: user.id.to_string(),
                    name: user.name,
                }),
            })
            .collect();

        Ok(activities)
    }
}
