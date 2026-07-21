use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{
    graphql::{GraphQLError, require_module_enabled},
    AuthContext, TenantContext, has_any_effective_permission,
};
use rustok_api::{Action, Permission, Resource};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{CreatePageInput, PageBodyInput, PageService, PageTranslationInput, UpdatePageInput};

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
        if input.publish.unwrap_or(false) {
            require_pages_permission(ctx, Permission::new(Resource::Pages, Action::Publish))?;
        }
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .create(
                tenant_id,
                page_security(&auth),
                CreatePageInput {
                    translations: input
                        .translations
                        .into_iter()
                        .map(|translation| PageTranslationInput {
                            locale: translation.locale,
                            title: translation.title,
                            slug: translation.slug,
                            meta_title: translation.meta_title,
                            meta_description: translation.meta_description,
                        })
                        .collect(),
                    template: input.template,
                    body: input.body.map(|body| PageBodyInput {
                        locale: body.locale,
                        content: body.content,
                        format: body.format,
                        content_json: body.content_json,
                    }),
                    channel_slugs: input.channel_slugs,
                    publish: input.publish.unwrap_or(false),
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .update(
                tenant_id,
                page_security(&auth),
                id,
                UpdatePageInput {
                    expected_version: input.expected_version,
                    translations: input.translations.map(|translations| {
                        translations
                            .into_iter()
                            .map(|translation| PageTranslationInput {
                                locale: translation.locale,
                                title: translation.title,
                                slug: translation.slug,
                                meta_title: translation.meta_title,
                                meta_description: translation.meta_description,
                            })
                            .collect()
                    }),
                    template: input.template,
                    channel_slugs: input.channel_slugs,
                    body: input.body.map(|body| PageBodyInput {
                        locale: body.locale,
                        content: body.content,
                        format: body.format,
                        content_json: body.content_json,
                    }),
                    status: None,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(page.into())
    }

    async fn publish_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: Option<i32>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth =
            require_pages_permission(ctx, Permission::new(Resource::Pages, Action::Publish))?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .publish_if_current(tenant_id, page_security(&auth), id, expected_version)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(page.into())
    }

    async fn unpublish_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: Option<i32>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth =
            require_pages_permission(ctx, Permission::new(Resource::Pages, Action::Publish))?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .unpublish_if_current(tenant_id, page_security(&auth), id, expected_version)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

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
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        let service = PageService::new(db.clone(), event_bus.clone());
        service
            .delete(tenant_id, page_security(&auth), id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(true)
    }
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
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
            "Page mutations must use the current tenant",
        ));
    }
    Ok(tenant.id)
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
    fn page_mutation_tenant_override_fails_closed() {
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
