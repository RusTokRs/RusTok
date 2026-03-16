use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_outbox::TransactionalEventBus;
use rustok_pages::{
    BlockService, BlockTranslationInput, BlockType, CreateBlockInput, CreatePageInput,
    PageBodyInput, PageService, PageTranslationInput, UpdateBlockInput, UpdatePageInput,
};

use crate::context::AuthContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::errors::GraphQLError;
use crate::graphql::schema::module_slug;
use crate::services::rbac_service::RbacService;
use rustok_core::Permission;

use super::types::*;

#[derive(Default)]
pub struct PagesMutation;

#[Object]
impl PagesMutation {
    async fn create_page(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        input: CreateGqlPageInput,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PAGES_CREATE, Permission::PAGES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: pages:create required",
            ));
        }

        let security = auth.security_context();
        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .create(
                tenant_id,
                security,
                CreatePageInput {
                    translations: input
                        .translations
                        .into_iter()
                        .map(|t| PageTranslationInput {
                            locale: t.locale,
                            title: t.title,
                            slug: t.slug,
                            meta_title: t.meta_title,
                            meta_description: t.meta_description,
                        })
                        .collect(),
                    template: input.template,
                    body: input.body.map(|b| PageBodyInput {
                        locale: b.locale,
                        content: b.content,
                        format: b.format,
                        content_json: b.content_json,
                    }),
                    blocks: input
                        .blocks
                        .map(|blocks| {
                            blocks
                                .into_iter()
                                .map(map_create_block_input)
                                .collect::<Result<Vec<_>>>()
                        })
                        .transpose()?,
                    publish: input.publish.unwrap_or(false),
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn update_page(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateGqlPageInput,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PAGES_UPDATE, Permission::PAGES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: pages:update required",
            ));
        }

        let security = auth.security_context();
        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .update(
                tenant_id,
                security,
                id,
                UpdatePageInput {
                    translations: input.translations.map(|translations| {
                        translations
                            .into_iter()
                            .map(|t| PageTranslationInput {
                                locale: t.locale,
                                title: t.title,
                                slug: t.slug,
                                meta_title: t.meta_title,
                                meta_description: t.meta_description,
                            })
                            .collect()
                    }),
                    template: input.template,
                    body: input.body.map(|b| PageBodyInput {
                        locale: b.locale,
                        content: b.content,
                        format: b.format,
                        content_json: b.content_json,
                    }),
                    status: None,
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn publish_page(&self, ctx: &Context<'_>, tenant_id: Uuid, id: Uuid) -> Result<GqlPage> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PAGES_UPDATE, Permission::PAGES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: pages:update required",
            ));
        }

        let security = auth.security_context();
        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .publish(tenant_id, security, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn unpublish_page(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PAGES_UPDATE, Permission::PAGES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: pages:update required",
            ));
        }

        let security = auth.security_context();
        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .unpublish(tenant_id, security, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn delete_page(&self, ctx: &Context<'_>, tenant_id: Uuid, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = RbacService::has_any_permission(
            db,
            &tenant_id,
            &auth.user_id,
            &[Permission::PAGES_DELETE, Permission::PAGES_MANAGE],
        )
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

        if !has_perm {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: pages:delete required",
            ));
        }

        let security = auth.security_context();
        let service = PageService::new(db.clone(), event_bus.clone());
        service
            .delete(tenant_id, security, id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn add_block(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        page_id: Uuid,
        input: CreateGqlBlockInput,
    ) -> Result<GqlBlock> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        ensure_pages_permission(db, &tenant_id, &auth.user_id, Permission::PAGES_UPDATE).await?;

        let service = BlockService::new(db.clone(), event_bus.clone());
        let block = service
            .create(
                tenant_id,
                auth.security_context(),
                page_id,
                map_create_block_input(input)?,
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(block.into())
    }

    async fn update_block(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        block_id: Uuid,
        input: UpdateGqlBlockInput,
    ) -> Result<GqlBlock> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        ensure_pages_permission(db, &tenant_id, &auth.user_id, Permission::PAGES_UPDATE).await?;

        let service = BlockService::new(db.clone(), event_bus.clone());
        let block = service
            .update(
                tenant_id,
                auth.security_context(),
                block_id,
                UpdateBlockInput {
                    position: input.position,
                    data: input.data,
                    translations: input.translations.map(map_block_translations),
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(block.into())
    }

    async fn delete_block(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        block_id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        ensure_pages_permission(db, &tenant_id, &auth.user_id, Permission::PAGES_DELETE).await?;

        let service = BlockService::new(db.clone(), event_bus.clone());
        service
            .delete(tenant_id, auth.security_context(), block_id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn reorder_blocks(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        page_id: Uuid,
        input: ReorderBlocksInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        ensure_pages_permission(db, &tenant_id, &auth.user_id, Permission::PAGES_UPDATE).await?;

        let service = BlockService::new(db.clone(), event_bus.clone());
        service
            .reorder(tenant_id, auth.security_context(), page_id, input.block_ids)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }
}

async fn ensure_pages_permission(
    db: &DatabaseConnection,
    tenant_id: &Uuid,
    user_id: &Uuid,
    permission: Permission,
) -> Result<()> {
    let has_perm = RbacService::has_any_permission(
        db,
        tenant_id,
        user_id,
        &[permission, Permission::PAGES_MANAGE],
    )
    .await
    .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

    if !has_perm {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: pages:* required",
        ));
    }

    Ok(())
}

fn map_create_block_input(input: CreateGqlBlockInput) -> Result<CreateBlockInput> {
    Ok(CreateBlockInput {
        block_type: parse_block_type(&input.block_type)?,
        position: input.position,
        data: input.data,
        translations: input.translations.map(map_block_translations),
    })
}

fn map_block_translations(
    translations: Vec<GqlBlockTranslationInput>,
) -> Vec<BlockTranslationInput> {
    translations
        .into_iter()
        .map(|t| BlockTranslationInput {
            locale: t.locale,
            data: t.data,
        })
        .collect()
}

fn parse_block_type(value: &str) -> Result<BlockType> {
    serde_json::from_value::<BlockType>(serde_json::Value::String(value.to_string()))
        .map_err(|_| async_graphql::Error::new(format!("Invalid block_type: {value}")))
}
