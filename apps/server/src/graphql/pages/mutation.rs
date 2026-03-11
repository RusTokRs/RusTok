use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_outbox::TransactionalEventBus;
use rustok_pages::{
    CreatePageInput, PageBodyInput, PageService, PageTranslationInput, UpdatePageInput,
};

use crate::context::AuthContext;
use crate::graphql::errors::GraphQLError;
use crate::services::auth::AuthService;
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
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
                    blocks: None,
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
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
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let has_perm = AuthService::has_any_permission(
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
}
