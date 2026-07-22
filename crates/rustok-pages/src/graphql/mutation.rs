use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext,
    graphql::{GraphQLError, require_module_enabled, resolve_graphql_locale},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuItemInput,
    MenuItemTranslationInput, MenuLocation, MenuService, MenuTranslationInput,
    PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
    PAGE_PUBLISH_IDEMPOTENCY_CONFLICT, PAGE_PUBLISH_OPERATION_INTEGRITY, PageBodyInput,
    PageBodyRevisionInput, PageService, PageTranslationInput, PagesError, PatchPageMetadataInput,
    PublishPageInput, ReviewedPagePublishRuntimeInput, SavePageDocumentInput,
};

use super::types::*;

const MODULE_SLUG: &str = "pages";
const PAGE_METADATA_VERSION_CONFLICT: &str = "PAGE_METADATA_VERSION_CONFLICT";
const PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND: &str =
    "PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND";

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
            return Err(create_publish_bypass_error());
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
                        .map(page_translation_input)
                        .collect(),
                    template: input.template,
                    body: input.body.map(page_body_input),
                    channel_slugs: input.channel_slugs,
                    publish: false,
                },
            )
            .await
            .map_err(map_pages_error)?;

        Ok(page.into())
    }

    async fn create_menu(
        &self,
        ctx: &Context<'_>,
        input: CreateGqlMenuInput,
        locale: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlMenu> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_CREATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;
        let effective_locale = resolve_graphql_locale(ctx, locale.as_deref());

        MenuService::new(db.clone(), event_bus.clone())
            .create(
                tenant_id,
                page_security(&auth),
                &effective_locale,
                CreateMenuInput {
                    translations: input
                        .translations
                        .into_iter()
                        .map(menu_translation_input)
                        .collect(),
                    location: menu_location_input(input.location),
                    items: input.items.into_iter().map(menu_item_input).collect(),
                },
            )
            .await
            .map(Into::into)
            .map_err(map_pages_error)
    }

    async fn patch_page_metadata(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: PatchGqlPageMetadataInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        PageService::new(db.clone(), event_bus.clone())
            .patch_metadata(
                tenant_id,
                page_security(&auth),
                id,
                PatchPageMetadataInput {
                    expected_version: input.expected_version,
                    translations: input
                        .translations
                        .map(|items| items.into_iter().map(page_translation_input).collect()),
                    template: input.template,
                    channel_slugs: input.channel_slugs,
                },
            )
            .await
            .map(Into::into)
            .map_err(map_pages_error)
    }

    async fn save_page_document(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: SaveGqlPageDocumentInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPage> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        PageService::new(db.clone(), event_bus.clone())
            .save_document(
                tenant_id,
                page_security(&auth),
                id,
                SavePageDocumentInput {
                    expected_revision: input.expected_revision,
                    body: page_body_input(input.body),
                },
            )
            .await
            .map(Into::into)
            .map_err(map_pages_error)
    }

    async fn publish_page(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: PublishGqlPageInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPublishPageResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth =
            require_pages_permission(ctx, Permission::new(Resource::Pages, Action::Publish))?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, tenant_id)?;

        PageService::new(db.clone(), event_bus.clone())
            .publish_reviewed(
                tenant_id,
                page_security(&auth),
                id,
                publish_page_input(input),
            )
            .await
            .map(Into::into)
            .map_err(map_pages_error)
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

        PageService::new(db.clone(), event_bus.clone())
            .unpublish_if_current(tenant_id, page_security(&auth), id, expected_version)
            .await
            .map(Into::into)
            .map_err(map_pages_error)
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

        PageService::new(db.clone(), event_bus.clone())
            .delete(tenant_id, page_security(&auth), id)
            .await
            .map_err(map_pages_error)?;
        Ok(true)
    }
}

fn page_translation_input(input: GqlPageTranslationInput) -> PageTranslationInput {
    PageTranslationInput {
        locale: input.locale,
        title: input.title,
        slug: input.slug,
        meta_title: input.meta_title,
        meta_description: input.meta_description,
    }
}

fn page_body_input(input: GqlPageBodyInput) -> PageBodyInput {
    PageBodyInput {
        locale: input.locale,
        content: input.content,
        format: input.format,
        content_json: input.content_json,
    }
}

fn publish_page_input(input: PublishGqlPageInput) -> PublishPageInput {
    PublishPageInput {
        expected_version: input.expected_version,
        expected_body_revisions: input
            .expected_body_revisions
            .into_iter()
            .map(|revision| PageBodyRevisionInput {
                locale: revision.locale,
                revision: revision.revision,
            })
            .collect(),
        idempotency_key: input.idempotency_key,
        runtime: ReviewedPagePublishRuntimeInput {
            format: input.runtime.format,
            scenario_id: input.runtime.scenario_id,
            context: input.runtime.context,
            review_hash: input.runtime.review_hash,
        },
    }
}

fn menu_translation_input(input: GqlMenuTranslationInput) -> MenuTranslationInput {
    MenuTranslationInput {
        locale: input.locale,
        name: input.name,
    }
}

fn menu_item_translation_input(input: GqlMenuItemTranslationInput) -> MenuItemTranslationInput {
    MenuItemTranslationInput {
        locale: input.locale,
        title: input.title,
    }
}

fn menu_item_input(input: GqlMenuItemInput) -> MenuItemInput {
    MenuItemInput {
        translations: input
            .translations
            .into_iter()
            .map(menu_item_translation_input)
            .collect(),
        url: input.url,
        page_id: input.page_id,
        icon: input.icon,
        position: input.position,
        children: input
            .children
            .map(|children| children.into_iter().map(menu_item_input).collect()),
    }
}

fn menu_location_input(input: GqlMenuLocation) -> MenuLocation {
    match input {
        GqlMenuLocation::Header => MenuLocation::Header,
        GqlMenuLocation::Footer => MenuLocation::Footer,
        GqlMenuLocation::Sidebar => MenuLocation::Sidebar,
        GqlMenuLocation::Mobile => MenuLocation::Mobile,
    }
}

fn create_publish_bypass_error() -> async_graphql::Error {
    async_graphql::Error::new(
        "Page creation cannot publish a Page Builder document; use publishPage with a reviewed runtime",
    )
    .extend_with(|_, extensions| {
        extensions.set("code", PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND);
    })
}

fn map_pages_error(error: PagesError) -> async_graphql::Error {
    let code = match &error {
        PagesError::VersionConflict { .. } => PAGE_METADATA_VERSION_CONFLICT,
        PagesError::PageNotFound(_) => "PAGE_NOT_FOUND",
        PagesError::MenuNotFound(_) => "MENU_NOT_FOUND",
        PagesError::DuplicateSlug { .. } => "DUPLICATE_SLUG",
        PagesError::Forbidden(_) => "PAGES_PERMISSION_DENIED",
        PagesError::FeatureDisabled { .. } => "FEATURE_DISABLED",
        PagesError::CannotDeletePublished => CANNOT_DELETE_PUBLISHED_ERROR_CODE,
        PagesError::PublishRuntimeReviewInvalid(_) => PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID,
        PagesError::PublishSanitize(_) => PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
        PagesError::PublishRuntimeMaterializationMismatch(_) => {
            PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH
        }
        PagesError::PublishIdempotencyConflict(_) => PAGE_PUBLISH_IDEMPOTENCY_CONFLICT,
        PagesError::PublishOperationIntegrity(_) => PAGE_PUBLISH_OPERATION_INTEGRITY,
        PagesError::Rich(rich) => rich
            .error_code
            .as_deref()
            .unwrap_or("PAGES_OPERATION_FAILED"),
        _ => "PAGES_OPERATION_FAILED",
    };
    async_graphql::Error::new(error.to_string()).extend_with(|_, extensions| {
        extensions.set("code", code);
    })
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
            "Pages mutations must use the current tenant",
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
