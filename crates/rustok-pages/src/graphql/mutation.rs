use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError},
    has_any_effective_permission, AuthContext,
};
use rustok_core::Permission;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    BlockService, BlockTranslationInput, BlockType, CreateBlockInput, CreatePageInput,
    PageBodyInput, PageService, PageTranslationInput, UpdateBlockInput, UpdatePageInput,
};

use super::types::*;

const MODULE_SLUG: &str = "pages";

#[derive(Default)]
pub struct PagesMutation;

#[Object]
impl PagesMutation {
    async fn create_page(
        &self,
        ctx: &Context<'_>,
        input: CreateGqlPageInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_CREATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .create(
                tenant_id,
                auth.security_context(),
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
                    channel_slugs: input.channel_slugs,
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
        id: Uuid,
        input: UpdateGqlPageInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .update(
                tenant_id,
                auth.security_context(),
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
                    channel_slugs: input.channel_slugs,
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

    async fn publish_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .publish(tenant_id, auth.security_context(), id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn unpublish_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .unpublish(tenant_id, auth.security_context(), id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.into())
    }

    async fn delete_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_DELETE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = PageService::new(db.clone(), event_bus.clone());
        service
            .delete(tenant_id, auth.security_context(), id)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }

    async fn add_block(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        input: CreateGqlBlockInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlBlock> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

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
        block_id: Uuid,
        input: UpdateGqlBlockInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlBlock> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

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
        block_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_DELETE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

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
        page_id: Uuid,
        input: ReorderBlocksInput,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<rustok_api::TenantContext>()?;
        let tenant_id = tenant_id.unwrap_or(tenant.id);

        let service = BlockService::new(db.clone(), event_bus.clone());
        service
            .reorder(tenant_id, auth.security_context(), page_id, input.block_ids)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(true)
    }
}

fn require_pages_permission(ctx: &Context<'_>, permission: Permission) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: pages:* required",
        ));
    }

    Ok(auth)
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
