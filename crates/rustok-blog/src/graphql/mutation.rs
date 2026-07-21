use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::Permission;
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext, TenantContext,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CommentService, ModerateCommentInput, PostService, UpdatePostInput as DomainUpdatePostInput,
};

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
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PostService::new(db.clone(), event_bus.clone());
        let post_id = service
            .create_post(
                tenant_id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
                input.into(),
            )
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
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

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
            .update_post(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
                domain_input,
            )
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
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .delete_post(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
            )
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
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .publish_post(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
            )
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
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .unpublish_post(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
            )
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
            &[Permission::BLOG_POSTS_PUBLISH],
            "Permission denied: blog_posts:publish required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PostService::new(db.clone(), event_bus.clone());
        service
            .archive_post(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
                reason,
            )
            .await?;

        Ok(true)
    }

    async fn moderate_comment(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        status: GqlModerateCommentStatus,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_blog_permission(
            ctx,
            &[Permission::BLOG_POSTS_MANAGE],
            "Permission denied: blog_posts:manage required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;
        let locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tenant.default_locale.clone());

        CommentService::new(db.clone(), event_bus.clone())
            .moderate_comment(
                tenant_id,
                id,
                rustok_core::security_context_from_access_token(
                    auth.user_id,
                    &auth.grant_type,
                    &auth.permissions,
                ),
                ModerateCommentInput {
                    status: status.into(),
                    locale: Some(locale),
                },
                Some(tenant.default_locale.as_str()),
            )
            .await?;

        Ok(true)
    }
}

fn mutation_tenant_id(
    tenant: &TenantContext,
    auth: &AuthContext,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ));
    }
    if requested.is_some_and(|tenant_id| tenant_id != tenant.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Blog mutations must use the current tenant",
        ));
    }
    Ok(tenant.id)
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

#[cfg(test)]
mod tests {
    use super::mutation_tenant_id;
    use rustok_api::{AuthContext, TenantContext};
    use uuid::Uuid;

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth(tenant_id: Uuid) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: Vec::new(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn blog_mutation_tenant_override_fails_closed() {
        let current = Uuid::new_v4();
        assert_eq!(
            mutation_tenant_id(&tenant(current), &auth(current), None).unwrap(),
            current
        );
        assert!(
            mutation_tenant_id(&tenant(current), &auth(current), Some(Uuid::new_v4())).is_err()
        );
        assert!(mutation_tenant_id(&tenant(current), &auth(Uuid::new_v4()), None).is_err());
    }
}
