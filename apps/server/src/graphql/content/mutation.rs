use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::errors::GraphQLError;
use crate::graphql::schema::module_slug;
use crate::services::rbac_service::RbacService;
use rustok_content::NodeService;
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;

use super::types::*;

#[derive(Default)]
pub struct ContentMutation;

#[Object]
impl ContentMutation {
    async fn create_node(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        author_id: Option<Uuid>,
        input: CreateNodeInput,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_CREATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:create required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let domain_input = rustok_content::dto::CreateNodeInput {
            parent_id: input.parent_id,
            author_id: input.author_id.or(author_id),
            kind: input.kind,
            category_id: input.category_id,
            status: input.status.map(Into::into),
            position: input.position,
            depth: input.depth,
            reply_count: input.reply_count,
            metadata: input
                .metadata
                .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
            translations: input
                .translations
                .into_iter()
                .map(|t| rustok_content::dto::NodeTranslationInput {
                    locale: t.locale,
                    title: t.title,
                    slug: t.slug,
                    excerpt: t.excerpt,
                })
                .collect(),
            bodies: input
                .bodies
                .into_iter()
                .map(|b| rustok_content::dto::BodyInput {
                    locale: b.locale,
                    body: b.body,
                    format: b.format,
                })
                .collect(),
        };

        let node: rustok_content::dto::NodeResponse = service
            .create_node(tenant_id, security, domain_input)
            .await?;

        Ok(node.into())
    }

    async fn update_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
        author_id: Option<Uuid>,
        input: UpdateNodeInput,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_UPDATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:update required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let resolved_author_id = input.author_id.or(author_id);
        let domain_input = rustok_content::dto::UpdateNodeInput {
            parent_id: input.parent_id.map(Some),
            author_id: resolved_author_id.map(Some),
            category_id: input.category_id.map(Some),
            status: input.status.map(Into::into),
            position: input.position,
            depth: input.depth,
            reply_count: input.reply_count,
            metadata: None,
            translations: input.translations.map(|ts| {
                ts.into_iter()
                    .map(|t| rustok_content::dto::NodeTranslationInput {
                        locale: t.locale,
                        title: t.title,
                        slug: t.slug,
                        excerpt: t.excerpt,
                    })
                    .collect()
            }),
            bodies: input.bodies.map(|bs| {
                bs.into_iter()
                    .map(|b| rustok_content::dto::BodyInput {
                        locale: b.locale,
                        body: b.body,
                        format: b.format,
                    })
                    .collect()
            }),
            expected_version: input.expected_version,
        };

        let node: rustok_content::dto::NodeResponse = service
            .update_node(tenant_id, id, security, domain_input)
            .await?;

        Ok(node.into())
    }

    async fn delete_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
        _author_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_DELETE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:delete required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        service.delete_node(tenant_id, id, security).await?;

        Ok(true)
    }

    async fn publish_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_UPDATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:update required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let node = service.publish_node(tenant_id, id, security).await?;

        Ok(node.into())
    }

    async fn unpublish_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_UPDATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:update required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let node = service.unpublish_node(tenant_id, id, security).await?;

        Ok(node.into())
    }

    async fn archive_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_UPDATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:update required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let node = service.archive_node(tenant_id, id, security).await?;

        Ok(node.into())
    }

    async fn restore_node(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Uuid,
    ) -> Result<GqlNode> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::NODES_UPDATE, Permission::NODES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: nodes:update required",
            ));
        }

        let security = auth.security_context();
        let service = NodeService::new(db.clone(), event_bus.clone());
        let node = service.restore_node(tenant_id, id, security).await?;

        Ok(node.into())
    }
}
