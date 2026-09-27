use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::{AuthContext, TenantContext};

#[derive(SimpleObject, Clone, Debug)]
pub struct StarterExecutionReportPayload {
    pub tenant_id: Uuid,
    pub blueprint_id: String,
    pub pages_created: i32,
    pub blog_categories_created: i32,
    pub blog_posts_created: i32,
    pub forum_categories_created: i32,
    pub forum_topics_created: i32,
    pub forum_replies_created: i32,
    pub menus_created: i32,
    pub skipped_existing: i32,
    pub duration_ms: i64,
}

impl From<rustok_starter::StarterExecutionReport> for StarterExecutionReportPayload {
    fn from(report: rustok_starter::StarterExecutionReport) -> Self {
        Self {
            tenant_id: report.tenant_id,
            blueprint_id: report.blueprint_id,
            pages_created: report.pages_created as i32,
            blog_categories_created: report.blog_categories_created as i32,
            blog_posts_created: report.blog_posts_created as i32,
            forum_categories_created: report.forum_categories_created as i32,
            forum_topics_created: report.forum_topics_created as i32,
            forum_replies_created: report.forum_replies_created as i32,
            menus_created: report.menus_created as i32,
            skipped_existing: report.skipped_existing as i32,
            duration_ms: report.duration_ms as i64,
        }
    }
}

#[derive(Default)]
pub struct StarterMutation;

#[Object]
impl StarterMutation {
    /// Imports a starter blueprint (pages, blog, forum, navigation) into the active tenant.
    /// Requires `MODULES_MANAGE` or `TENANTS_MANAGE` permission.
    async fn import_starter(
        &self,
        ctx: &Context<'_>,
        name: Option<String>,
    ) -> Result<StarterExecutionReportPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE)
            && !has_effective_permission(&auth.permissions, &Permission::TENANTS_MANAGE)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "modules:manage or tenants:manage permission required to import starter data",
            ));
        }

        let tenant = ctx.data::<TenantContext>()?;
        let db = ctx.data::<DatabaseConnection>()?.clone();

        let starter_name = name.unwrap_or_else(|| "default".to_string());
        let blueprint = if starter_name == "default" {
            rustok_starter::default_starter()
        } else {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "Unknown starter blueprint `{starter_name}`"
            )));
        };

        let event_bus = if let Ok(bus) = ctx.data::<TransactionalEventBus>() {
            bus.clone()
        } else {
            rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
                rustok_outbox::OutboxTransport::new(db.clone()),
            ))
        };

        let engine = rustok_starter::StarterEngine::new(db, event_bus);
        let security = rustok_core::SecurityContext::system();

        let report = engine
            .import_blueprint(tenant.id, &security, &blueprint)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(report.into())
    }
}
