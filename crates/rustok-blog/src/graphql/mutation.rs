use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext, TenantContext,
};
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{PostService, UpdatePostInput as DomainUpdatePostInput};

use super::types::*;

const MODULE_SLUG: &str = "blog";

#[derive(Default)]
pub struct BlogMutation;

#[Object]
impl BlogMutation {
    async fn create_post(
        &self,
        ctx: &Context<'_>,
        input: CreatePostInput,
        tenant_id: Option<Uuid>,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_CREATE],
            "Permission denied: blog_posts:create required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        let post_id = service
            .create_post(tenant_id, auth.security_context(), input.into())
            .await?;

        Ok(post_id)
    }

    async fn update_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdatePostInput,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_UPDATE],
            "Permission denied: blog_posts:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        let domain_input = DomainUpdatePostInput {
            locale: input.locale,
            title: input.title,
            body: input.body,
            body_format: input.body_format,
            content_json: input.content_json,
            excerpt: input.excerpt,
            slug: input.slug,
            tags: input.tags,
            category_id: input.category_id,
            featured_image_url: input.featured_image_url,
            seo_title: input.seo_title,
            seo_description: input.seo_description,
            channel_slugs: input.channel_slugs,
            metadata: None,
            version: None,
        };

        service
            .update_post(tenant_id, id, auth.security_context(), domain_input)
            .await?;

        Ok(true)
    }

    async fn delete_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_DELETE],
            "Permission denied: blog_posts:delete required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .delete_post(tenant_id, id, auth.security_context())
            .await?;

        Ok(true)
    }

    async fn publish_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_PUBLISH],
            "Permission denied: blog_posts:publish required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .publish_post(tenant_id, id, auth.security_context())
            .await?;

        Ok(true)
    }

    async fn unpublish_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_PUBLISH],
            "Permission denied: blog_posts:publish required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .unpublish_post(tenant_id, id, auth.security_context())
            .await?;

        Ok(true)
    }

    async fn archive_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        reason: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_UPDATE],
            "Permission denied: blog_posts:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .archive_post(tenant_id, id, auth.security_context(), reason)
            .await?;

        Ok(true)
    }
}

fn require_blog_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }

    Ok(auth)
}
