use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::rbac_service::RbacService;
use rustok_blog::{PostService, UpdatePostInput as DomainUpdatePostInput};
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;

use crate::graphql::common::require_module_enabled;
use crate::graphql::schema::module_slug;

use super::types::*;

#[derive(Default)]
pub struct BlogMutation;

#[Object]
impl BlogMutation {
    async fn create_post(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreatePostInput,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::BLOG_POSTS_CREATE, Permission::BLOG_POSTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:create required",
            ));
        }

        let security = auth.security_context();
        let service = PostService::new(db.clone(), event_bus.clone());
        let post_id = service
            .create_post(tenant_id, security, input.into())
            .await?;

        Ok(post_id)
    }

    async fn update_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
        input: UpdatePostInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::BLOG_POSTS_UPDATE, Permission::BLOG_POSTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:update required",
            ));
        }

        let security = auth.security_context();
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
            metadata: None,
            version: None,
        };

        service
            .update_post(tenant_id, id, security, domain_input)
            .await?;

        Ok(true)
    }

    async fn delete_post(&self, ctx: &Context<'_>, id: Uuid, tenant_id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::BLOG_POSTS_DELETE, Permission::BLOG_POSTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:delete required",
            ));
        }

        let security = auth.security_context();
        let service = PostService::new(db.clone(), event_bus.clone());
        service.delete_post(tenant_id, id, security).await?;

        Ok(true)
    }

    async fn publish_post(&self, ctx: &Context<'_>, id: Uuid, tenant_id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::BLOG_POSTS_PUBLISH,
                Permission::BLOG_POSTS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:publish required",
            ));
        }

        let security = auth.security_context();
        let service = PostService::new(db.clone(), event_bus.clone());
        service.publish_post(tenant_id, id, security).await?;

        Ok(true)
    }

    async fn unpublish_post(&self, ctx: &Context<'_>, id: Uuid, tenant_id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[
                Permission::BLOG_POSTS_PUBLISH,
                Permission::BLOG_POSTS_MANAGE,
            ],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:publish required",
            ));
        }

        let security = auth.security_context();
        let service = PostService::new(db.clone(), event_bus.clone());
        service.unpublish_post(tenant_id, id, security).await?;

        Ok(true)
    }

    async fn archive_post(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
        reason: Option<String>,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::BLOG_POSTS_UPDATE, Permission::BLOG_POSTS_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: blog_posts:update required",
            ));
        }

        let security = auth.security_context();
        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .archive_post(tenant_id, id, security, reason)
            .await?;

        Ok(true)
    }
}
