use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

use crate::auth::hash_password;
use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use crate::graphql::common::require_module_enabled;
use crate::graphql::errors::GraphQLError;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use crate::graphql::schema::module_slug;
use crate::graphql::types::{
    BuildJob, CreateUserInput, DeleteUserPayload, TenantModule, UpdateUserInput, User,
};
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use crate::graphql::types::{
    ContentOrchestrationPayload, DemotePostToTopicInput as GqlDemotePostToTopicInput,
    MergeTopicsInput as GqlMergeTopicsInput, PromoteTopicToPostInput as GqlPromoteTopicToPostInput,
    SplitTopicInput as GqlSplitTopicInput,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::release::{Column as ReleaseColumn, Entity as ReleaseEntity, ReleaseStatus};
use crate::models::users;
use crate::modules::{ManifestDiff, ManifestError, ManifestManager, ModulesManifest};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::build_event_hub::{
    build_event_hub_from_context, BuildEventHubPublisher, CompositeBuildEventPublisher,
};
use crate::services::build_service::BuildService;
use crate::services::build_service::EventBusBuildEventPublisher;
#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
use crate::services::content_orchestration::content_orchestration_from_context;
use crate::services::event_bus::event_bus_from_context;
use crate::services::flex_attached_values::{
    FlexAttachedValuesService, PreparedAttachedValuesWrite,
};
use crate::services::module_lifecycle::{
    ModuleLifecycleService, ToggleModuleError, UpdateModuleSettingsError,
};
use crate::services::platform_composition::{
    PlatformCompositionBuildError, PlatformCompositionBuildService, PlatformCompositionError,
    PlatformCompositionService,
};
use crate::services::rbac_service::RbacService;
use rustok_core::{ModuleRegistry, Permission};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct RootMutation;

const TOGGLE_ERR_UNKNOWN_MODULE: &str = "Unknown module";

fn toggle_err_core_module_cannot_be_disabled(module_slug: &str) -> String {
    format!("Core module cannot be disabled: {module_slug}")
}

fn toggle_err_missing_dependencies(missing: &str) -> String {
    format!("Missing module dependencies: {missing}")
}

fn toggle_err_has_dependents(dependents: &str) -> String {
    format!("Module is required by: {dependents}")
}

fn toggle_err_hook_failed(reason: &str) -> String {
    format!("Module lifecycle hook failed: {reason}")
}

fn map_custom_field_error(error: rustok_core::field_schema::FlexError) -> FieldError {
    match error {
        rustok_core::field_schema::FlexError::ValidationFailed(errors) => {
            let messages: Vec<String> = errors
                .iter()
                .map(|e| format!("{}: {}", e.field_key, e.message))
                .collect();
            FieldError::new(format!(
                "Custom field validation failed: {}",
                messages.join("; ")
            ))
            .extend_with(|_, ext| {
                ext.set("code", "CUSTOM_FIELD_VALIDATION_FAILED");
                if let Ok(v) = serde_json::to_value(&errors) {
                    if let Ok(gql_value) = async_graphql::Value::from_json(v) {
                        ext.set("fields", gql_value);
                    }
                }
            })
        }
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

fn effective_request_locale(ctx: &Context<'_>, tenant: &TenantContext) -> String {
    ctx.data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone())
}

async fn prepare_user_custom_fields_write(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    locale: &str,
    entity_id: Option<Uuid>,
    existing_metadata: Option<&serde_json::Value>,
    custom_fields: Option<serde_json::Value>,
) -> Result<PreparedAttachedValuesWrite> {
    let prepared = match (entity_id, existing_metadata) {
        (Some(entity_id), Some(existing_metadata)) => {
            FlexAttachedValuesService::prepare_update(
                db,
                tenant_id,
                "user",
                entity_id,
                locale,
                existing_metadata,
                custom_fields,
            )
            .await
        }
        _ => {
            FlexAttachedValuesService::prepare_create(db, tenant_id, "user", locale, custom_fields)
                .await
        }
    };

    prepared.map_err(map_custom_field_error)
}

#[cfg(test)]
async fn validate_custom_fields(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    custom_fields: Option<serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    Ok(prepare_user_custom_fields_write(
        db,
        tenant_id,
        rustok_core::PLATFORM_FALLBACK_LOCALE,
        None,
        None,
        custom_fields,
    )
    .await?
    .metadata)
}

fn map_create_user_error(err: AuthLifecycleError) -> FieldError {
    match err {
        AuthLifecycleError::EmailAlreadyExists => {
            FieldError::new("User with this email already exists")
        }
        AuthLifecycleError::Internal(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        _ => <FieldError as GraphQLError>::internal_error("Failed to create user"),
    }
}

fn map_manifest_error(err: ManifestError) -> FieldError {
    match err {
        ManifestError::UnknownModule(_)
        | ManifestError::ModuleAlreadyInstalled(_)
        | ManifestError::ModuleNotInstalled(_)
        | ManifestError::RequiredModule(_)
        | ManifestError::HasDependents { .. }
        | ManifestError::MissingDependencies { .. }
        | ManifestError::UnknownDefaultEnabled(_)
        | ManifestError::VersionUnchanged(_, _)
        | ManifestError::InvalidVersion
        | ManifestError::InvalidBuildSurface(_)
        | ManifestError::MissingInRegistry(_)
        | ManifestError::RequiredMismatch(_)
        | ManifestError::DependencyMismatch(_)
        | ManifestError::MissingModulePackageManifest { .. }
        | ManifestError::ModulePackageSlugMismatch { .. }
        | ManifestError::InvalidModuleVersion { .. }
        | ManifestError::InvalidModuleDependency { .. }
        | ManifestError::InvalidModuleConflict { .. }
        | ManifestError::InvalidDependencyVersionReq { .. }
        | ManifestError::MissingDependencyVersion { .. }
        | ManifestError::IncompatibleDependencyVersion { .. }
        | ManifestError::ConflictingModule { .. }
        | ManifestError::IncompatibleRustokVersion { .. }
        | ManifestError::InvalidModuleOwnership { .. }
        | ManifestError::InvalidModuleTrustLevel { .. }
        | ManifestError::InvalidModuleUiClassification { .. }
        | ManifestError::InvalidModuleAdminSurface { .. }
        | ManifestError::ConflictingModuleAdminSurface { .. }
        | ManifestError::InvalidModuleSettingKey { .. }
        | ManifestError::InvalidModuleSettingSchema { .. }
        | ManifestError::InvalidModuleSettingValue { .. }
        | ManifestError::InvalidModuleMarketplaceMetadata { .. }
        | ManifestError::InvalidModuleUiWiring { .. } => FieldError::new(err.to_string()),
        ManifestError::Read { .. }
        | ManifestError::Parse { .. }
        | ManifestError::Write { .. }
        | ManifestError::ModulePackageRead { .. }
        | ManifestError::ModulePackageParse { .. } => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
    }
}

fn parse_build_id(build_id: &str) -> Result<Uuid> {
    Uuid::parse_str(build_id).map_err(|_| FieldError::new("Invalid build ID"))
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
fn map_content_error(err: rustok_content::ContentError) -> FieldError {
    match err {
        rustok_content::ContentError::Validation(message)
        | rustok_content::ContentError::Forbidden(message) => FieldError::new(message),
        rustok_content::ContentError::NodeNotFound(_)
        | rustok_content::ContentError::CategoryNotFound(_)
        | rustok_content::ContentError::TranslationNotFound { .. }
        | rustok_content::ContentError::DuplicateSlug { .. }
        | rustok_content::ContentError::ConcurrentModification { .. } => {
            FieldError::new(err.to_string())
        }
        rustok_content::ContentError::Database(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        rustok_content::ContentError::Core(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        rustok_content::ContentError::Rich(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
    }
}

async fn ensure_modules_manage_permission(
    ctx: &Context<'_>,
) -> Result<(AuthContext, TenantContext)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let tenant = ctx.data::<TenantContext>()?.clone();
    let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

    let can_manage_modules = RbacService::has_permission(
        &app_ctx.db,
        &tenant.id,
        &auth.user_id,
        &Permission::MODULES_MANAGE,
    )
    .await
    .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    if !can_manage_modules {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: modules:manage required",
        ));
    }

    Ok((auth, tenant))
}

#[allow(clippy::too_many_arguments)]
async fn persist_manifest_and_request_build(
    app_ctx: &loco_rs::app::AppContext,
    tenant_id: Uuid,
    registry: &ModuleRegistry,
    expected_revision: Option<i64>,
    manifest: ModulesManifest,
    manifest_diff: ManifestDiff,
    requested_by: &str,
    reason: String,
) -> Result<BuildJob> {
    let event_publisher = Arc::new(CompositeBuildEventPublisher::new(vec![
        Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
            app_ctx,
        ))),
        Arc::new(EventBusBuildEventPublisher::new(
            event_bus_from_context(app_ctx),
            tenant_id,
        )),
    ]));

    let result = PlatformCompositionBuildService::update_manifest_and_request_build(
        &app_ctx.db,
        event_publisher,
        registry,
        expected_revision,
        manifest,
        manifest_diff,
        requested_by.to_string(),
        reason,
    )
    .await
    .map_err(map_platform_composition_build_error)?;

    Ok(BuildJob::from_model(&result.build))
}

fn map_platform_composition_build_error(error: PlatformCompositionBuildError) -> FieldError {
    match error {
        PlatformCompositionBuildError::Composition(error) => map_platform_composition_error(error),
        PlatformCompositionBuildError::Build(error) => {
            <FieldError as GraphQLError>::internal_error(&error)
        }
    }
}

fn map_toggle_module_error(error: ToggleModuleError) -> FieldError {
    match error {
        ToggleModuleError::UnknownModule => {
            <FieldError as GraphQLError>::bad_user_input(TOGGLE_ERR_UNKNOWN_MODULE)
        }
        ToggleModuleError::CoreModuleCannotBeDisabled(module_slug) => {
            <FieldError as GraphQLError>::bad_user_input(toggle_err_core_module_cannot_be_disabled(
                &module_slug,
            ))
        }
        ToggleModuleError::MissingDependencies(missing) => {
            <FieldError as GraphQLError>::bad_user_input(toggle_err_missing_dependencies(&missing))
        }
        ToggleModuleError::HasDependents(dependents) => {
            <FieldError as GraphQLError>::bad_user_input(toggle_err_has_dependents(&dependents))
        }
        ToggleModuleError::Database(err) => {
            <FieldError as GraphQLError>::internal_error(&err.to_string())
        }
        ToggleModuleError::PreHookFailed(err) => {
            FieldError::new(toggle_err_hook_failed(&err)).extend_with(|_, ext| {
                ext.set("code", "MODULE_HOOK_FAILED");
                ext.set("retryable_issue", false);
                ext.set("operation_issue", "pre_hook_failed");
            })
        }
        ToggleModuleError::PostHookFailed(err) => {
            FieldError::new(toggle_err_hook_failed(&err)).extend_with(|_, ext| {
                ext.set("code", "MODULE_HOOK_FAILED");
                ext.set("retryable_issue", true);
                ext.set("operation_issue", "post_hook_failed");
            })
        }
        ToggleModuleError::Policy(err) => <FieldError as GraphQLError>::internal_error(&err),
    }
}

fn map_platform_composition_error(error: PlatformCompositionError) -> FieldError {
    match error {
        PlatformCompositionError::RevisionConflict { expected, current } => {
            FieldError::new(format!(
                "Platform composition revision conflict: expected {expected}, current {current}"
            ))
        }
        PlatformCompositionError::Manifest(error) => map_manifest_error(error),
        other => <FieldError as GraphQLError>::internal_error(&other.to_string()),
    }
}

#[Object]
impl RootMutation {
    async fn create_user(&self, ctx: &Context<'_>, input: CreateUserInput) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_create_users = RbacService::has_any_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &[
                rustok_core::Permission::USERS_CREATE,
                rustok_core::Permission::USERS_MANAGE,
            ],
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_create_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:create required",
            ));
        }

        let requested_role = input
            .role
            .map(Into::into)
            .unwrap_or(rustok_core::UserRole::Customer);

        let locale = effective_request_locale(ctx, tenant);
        let prepared_custom_fields = prepare_user_custom_fields_write(
            &app_ctx.db,
            tenant.id,
            locale.as_str(),
            None,
            None,
            input.custom_fields,
        )
        .await?;

        let status = input.status.map(Into::into);
        let mut user = AuthLifecycleService::create_user(
            app_ctx,
            tenant.id,
            &input.email,
            &input.password,
            input.name,
            requested_role,
            status,
        )
        .await
        .map_err(map_create_user_error)?;

        if let Some(metadata) = prepared_custom_fields.metadata {
            let mut active: users::ActiveModel = user.into();
            active.metadata = Set(metadata);
            user = active
                .update(&app_ctx.db)
                .await
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        }

        if let (Some(locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &app_ctx.db,
                tenant.id,
                "user",
                user.id,
                locale,
                values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }

        Ok(User::from(&user))
    }

    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    async fn promote_topic_to_post(
        &self,
        ctx: &Context<'_>,
        input: GqlPromoteTopicToPostInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        require_module_enabled(ctx, module_slug::BLOG).await?;
        require_module_enabled(ctx, module_slug::FORUM).await?;
        require_module_enabled(ctx, "comments").await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let service = content_orchestration_from_context(app_ctx);

        let result = service
            .promote_topic_to_post(
                tenant.id,
                auth.security_context(),
                rustok_content::PromoteTopicToPostInput {
                    topic_id: input.topic_id,
                    locale: input.locale,
                    blog_category_id: input.blog_category_id,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;

        Ok(ContentOrchestrationPayload {
            source_id: result.source_id,
            target_id: result.target_id,
            moved_comments: result.moved_comments,
        })
    }

    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    async fn demote_post_to_topic(
        &self,
        ctx: &Context<'_>,
        input: GqlDemotePostToTopicInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        require_module_enabled(ctx, module_slug::BLOG).await?;
        require_module_enabled(ctx, module_slug::FORUM).await?;
        require_module_enabled(ctx, "comments").await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let service = content_orchestration_from_context(app_ctx);

        let result = service
            .demote_post_to_topic(
                tenant.id,
                auth.security_context(),
                rustok_content::DemotePostToTopicInput {
                    post_id: input.post_id,
                    locale: input.locale,
                    forum_category_id: input.forum_category_id,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;

        Ok(ContentOrchestrationPayload {
            source_id: result.source_id,
            target_id: result.target_id,
            moved_comments: result.moved_comments,
        })
    }

    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    async fn split_topic(
        &self,
        ctx: &Context<'_>,
        input: GqlSplitTopicInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        require_module_enabled(ctx, module_slug::FORUM).await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let service = content_orchestration_from_context(app_ctx);

        let result = service
            .split_topic(
                tenant.id,
                auth.security_context(),
                rustok_content::SplitTopicInput {
                    topic_id: input.topic_id,
                    locale: input.locale,
                    reply_ids: input.reply_ids,
                    new_title: input.new_title,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;

        Ok(ContentOrchestrationPayload {
            source_id: result.source_id,
            target_id: result.target_id,
            moved_comments: result.moved_comments,
        })
    }

    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    async fn merge_topics(
        &self,
        ctx: &Context<'_>,
        input: GqlMergeTopicsInput,
    ) -> Result<ContentOrchestrationPayload> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        require_module_enabled(ctx, module_slug::FORUM).await?;

        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let service = content_orchestration_from_context(app_ctx);

        let result = service
            .merge_topics(
                tenant.id,
                auth.security_context(),
                rustok_content::MergeTopicsInput {
                    target_topic_id: input.target_topic_id,
                    source_topic_ids: input.source_topic_ids,
                    reason: input.reason,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_content_error)?;

        Ok(ContentOrchestrationPayload {
            source_id: result.source_id,
            target_id: result.target_id,
            moved_comments: result.moved_comments,
        })
    }

    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
        input: UpdateUserInput,
    ) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_update_users = RbacService::has_any_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &[
                rustok_core::Permission::USERS_UPDATE,
                rustok_core::Permission::USERS_MANAGE,
            ],
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_update_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:update required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        if let Some(email) = &input.email {
            let existing = users::Entity::find_by_email(&app_ctx.db, tenant.id, email)
                .await
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

            if existing
                .as_ref()
                .is_some_and(|existing| existing.id != user.id)
            {
                return Err(FieldError::new("User with this email already exists"));
            }
        }

        let user_id = user.id;
        let existing_metadata = user.metadata.clone();
        let mut model: users::ActiveModel = user.into();

        if let Some(email) = input.email {
            model.email = Set(email.to_lowercase());
        }

        if let Some(name) = input.name {
            model.name = Set(Some(name));
        }

        let requested_role = input.role.map(rustok_core::UserRole::from);

        if let Some(status) = input.status {
            let status: rustok_core::UserStatus = status.into();
            model.status = Set(status);
        }

        if let Some(password) = input.password {
            let password_hash = hash_password(&password)
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
            model.password_hash = Set(password_hash);
        }

        let locale = effective_request_locale(ctx, tenant);
        let prepared_custom_fields = prepare_user_custom_fields_write(
            &app_ctx.db,
            tenant.id,
            locale.as_str(),
            Some(user_id),
            Some(&existing_metadata),
            input.custom_fields,
        )
        .await?;

        if let Some(metadata) = prepared_custom_fields.metadata {
            model.metadata = Set(metadata);
        }

        let tx = app_ctx
            .db
            .begin()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let user = model
            .update(&tx)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if let Some(role) = requested_role {
            RbacService::replace_user_role(&tx, &user.id, &tenant.id, role)
                .await
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        }

        if let (Some(locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &tx, tenant.id, "user", user_id, locale, values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }

        tx.commit()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(User::from(&user))
    }

    async fn disable_user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<User> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_manage_users = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_MANAGE,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_manage_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:manage required",
            ));
        }

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        let mut model: users::ActiveModel = user.into();
        model.status = Set(rustok_core::UserStatus::Inactive);

        let user = model
            .update(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(User::from(&user))
    }

    async fn delete_user(&self, ctx: &Context<'_>, id: uuid::Uuid) -> Result<DeleteUserPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;

        let can_manage_users = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &rustok_core::Permission::USERS_MANAGE,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_manage_users {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: users:manage required",
            ));
        }

        let txn = app_ctx
            .db
            .begin()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&txn)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        FlexAttachedValuesService::delete_localized_values(&txn, tenant.id, "user", id)
            .await
            .map_err(map_custom_field_error)?;

        let model: users::ActiveModel = user.into();
        model
            .delete(&txn)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        txn.commit()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(DeleteUserPayload { success: true })
    }

    async fn install_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        version: String,
        expected_revision: Option<i64>,
    ) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let snapshot = PlatformCompositionService::active_snapshot(&app_ctx.db)
            .await
            .map_err(map_platform_composition_error)?;
        let mut manifest = snapshot.manifest.clone();
        let manifest_diff =
            ManifestManager::install_builtin_module(&mut manifest, &slug, Some(version))
                .map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            Some(expected_revision.unwrap_or(snapshot.revision)),
            manifest,
            manifest_diff,
            &auth.user_id.to_string(),
            format!("install module {slug}"),
        )
        .await
    }

    async fn uninstall_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        expected_revision: Option<i64>,
    ) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let snapshot = PlatformCompositionService::active_snapshot(&app_ctx.db)
            .await
            .map_err(map_platform_composition_error)?;
        let mut manifest = snapshot.manifest.clone();
        let manifest_diff =
            ManifestManager::uninstall_module(&mut manifest, &slug).map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            Some(expected_revision.unwrap_or(snapshot.revision)),
            manifest,
            manifest_diff,
            &auth.user_id.to_string(),
            format!("uninstall module {slug}"),
        )
        .await
    }

    async fn upgrade_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        version: String,
        expected_revision: Option<i64>,
    ) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let snapshot = PlatformCompositionService::active_snapshot(&app_ctx.db)
            .await
            .map_err(map_platform_composition_error)?;
        let mut manifest = snapshot.manifest.clone();
        let manifest_diff = ManifestManager::upgrade_module(&mut manifest, &slug, version)
            .map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            Some(expected_revision.unwrap_or(snapshot.revision)),
            manifest,
            manifest_diff,
            &auth.user_id.to_string(),
            format!("upgrade module {slug}"),
        )
        .await
    }

    async fn rollback_build(&self, ctx: &Context<'_>, build_id: String) -> Result<BuildJob> {
        let (_, tenant) = ensure_modules_manage_permission(ctx).await?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let service = BuildService::with_event_publisher(
            app_ctx.db.clone(),
            Arc::new(CompositeBuildEventPublisher::new(vec![
                Arc::new(BuildEventHubPublisher::new(build_event_hub_from_context(
                    app_ctx,
                ))),
                Arc::new(EventBusBuildEventPublisher::new(
                    event_bus_from_context(app_ctx),
                    tenant.id,
                )),
            ])),
        );

        if service
            .active_build()
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .is_some()
        {
            return Err(FieldError::new(
                "Cannot rollback while another build is still queued or running",
            ));
        }

        let build = service
            .get_build(parse_build_id(&build_id)?)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("Build not found"))?;

        let release_id = build
            .release_id
            .clone()
            .ok_or_else(|| FieldError::new("Build does not have a release to rollback"))?;

        let active_release = ReleaseEntity::find()
            .filter(ReleaseColumn::Status.eq(ReleaseStatus::Active))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("No active release available for rollback"))?;

        if active_release.id != release_id {
            return Err(FieldError::new(
                "Only the build that backs the current active release can be rolled back",
            ));
        }

        if active_release.previous_release_id.is_none() {
            return Err(FieldError::new(
                "No previous release available for rollback",
            ));
        }

        let restored_release = service
            .rollback(&release_id)
            .await
            .map_err(|err| FieldError::new(err.to_string()))?;

        let restored_build = service
            .get_build(restored_release.build_id)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::internal_error(
                    "restored release is missing its build record",
                )
            })?;

        Ok(BuildJob::from_model(&restored_build))
    }

    async fn toggle_module(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        enabled: bool,
    ) -> Result<TenantModule> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;

        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let tenant = ctx.data::<TenantContext>()?;

        let can_manage_modules = RbacService::has_permission(
            &app_ctx.db,
            &tenant.id,
            &auth.user_id,
            &Permission::MODULES_MANAGE,
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_manage_modules {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            ));
        }

        let registry = ctx.data::<ModuleRegistry>()?;

        let module = ModuleLifecycleService::toggle_module_with_actor(
            &app_ctx.db,
            registry,
            tenant.id,
            &module_slug,
            enabled,
            Some(auth.user_id.to_string()),
        )
        .await
        .map_err(map_toggle_module_error)?;

        Ok(TenantModule {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
        })
    }

    async fn update_module_settings(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        settings: String,
    ) -> Result<TenantModule> {
        let (_, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let settings_json: serde_json::Value = serde_json::from_str(&settings)
            .map_err(|err| FieldError::new(format!("Invalid JSON in settings: {err}")))?;

        let module = ModuleLifecycleService::update_module_settings(
            &app_ctx.db,
            registry,
            tenant.id,
            &module_slug,
            settings_json,
        )
        .await
        .map_err(|err| match err {
            UpdateModuleSettingsError::UnknownModule => FieldError::new("Unknown module"),
            UpdateModuleSettingsError::ModuleNotEnabled(module_slug) => FieldError::new(format!(
                "Module is not enabled for this tenant: {}",
                module_slug
            )),
            UpdateModuleSettingsError::InvalidSettings => {
                FieldError::new("Module settings must be a JSON object")
            }
            UpdateModuleSettingsError::Validation(message) => FieldError::new(message),
            UpdateModuleSettingsError::Manifest(err) => map_manifest_error(err),
            UpdateModuleSettingsError::Policy(err) => {
                <FieldError as GraphQLError>::internal_error(&err)
            }
            UpdateModuleSettingsError::Database(err) => {
                <FieldError as GraphQLError>::internal_error(&err.to_string())
            }
        })?;

        Ok(TenantModule {
            module_slug: module.module_slug,
            enabled: module.enabled,
            settings: module.settings.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_create_user_error, map_manifest_error, map_platform_composition_build_error,
        map_platform_composition_error, map_toggle_module_error, prepare_user_custom_fields_write,
        validate_custom_fields, AuthLifecycleError, ManifestError, PlatformCompositionBuildError,
        PlatformCompositionError, ToggleModuleError,
    };
    use async_graphql::ErrorExtensions;
    use crate::models::user_field_definitions::ActiveModel as UserFieldDefinitionActiveModel;
    use migration::Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        entity::prelude::DateTimeWithTimeZone, ActiveModelTrait, DatabaseConnection, Set,
    };
    use uuid::Uuid;

    fn field_definition_model(
        tenant_id: Uuid,
        field_key: &str,
        field_type: &str,
        is_localized: bool,
        is_required: bool,
        default_value: Option<serde_json::Value>,
    ) -> UserFieldDefinitionActiveModel {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        UserFieldDefinitionActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            field_key: Set(field_key.to_string()),
            field_type: Set(field_type.to_string()),
            label: Set(serde_json::json!({"en": field_key})),
            description: Set(None),
            is_localized: Set(is_localized),
            is_required: Set(is_required),
            default_value: Set(default_value),
            validation: Set(None),
            position: Set(0),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
    }

    async fn db_with_definitions(
        definitions: Vec<UserFieldDefinitionActiveModel>,
    ) -> DatabaseConnection {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        for definition in definitions {
            definition
                .insert(&db)
                .await
                .expect("failed to insert user field definition");
        }
        db
    }

    #[test]
    fn create_user_maps_email_exists() {
        let err = map_create_user_error(AuthLifecycleError::EmailAlreadyExists);
        assert!(err.message.contains("already exists"));
    }

    #[test]
    fn create_user_maps_internal_error() {
        let err = map_create_user_error(AuthLifecycleError::Internal(
            crate::error::Error::InternalServerError,
        ));
        assert!(!err.message.is_empty());
    }

    #[test]
    fn toggle_error_maps_database_and_policy_to_internal_errors() {
        let db_err = map_toggle_module_error(ToggleModuleError::Database(sea_orm::DbErr::Custom(
            "db down".to_string(),
        )));
        assert!(!db_err.message.is_empty());

        let policy_err = map_toggle_module_error(ToggleModuleError::Policy("policy".to_string()));
        assert!(!policy_err.message.is_empty());
    }

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn extension_string(error: &async_graphql::Error, key: &str) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(key))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    fn extension_bool(error: &async_graphql::Error, key: &str) -> Option<bool> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get(key))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_bool())
    }

    struct ToggleCase {
        error: ToggleModuleError,
        expected_message: String,
        expected_code: Option<&'static str>,
        case_name: &'static str,
    }

    fn toggle_error_contract_cases() -> Vec<ToggleCase> {
        vec![
            ToggleCase {
                error: ToggleModuleError::UnknownModule,
                expected_message: TOGGLE_ERR_UNKNOWN_MODULE.to_string(),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "unknown-module",
            },
            ToggleCase {
                error: ToggleModuleError::CoreModuleCannotBeDisabled("core".into()),
                expected_message: toggle_err_core_module_cannot_be_disabled("core"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "core-disable",
            },
            ToggleCase {
                error: ToggleModuleError::MissingDependencies("pricing".into()),
                expected_message: toggle_err_missing_dependencies("pricing"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "missing-dependencies",
            },
            ToggleCase {
                error: ToggleModuleError::HasDependents("checkout".into()),
                expected_message: toggle_err_has_dependents("checkout"),
                expected_code: Some("BAD_USER_INPUT"),
                case_name: "has-dependents",
            },
            ToggleCase {
                error: ToggleModuleError::PreHookFailed("boom".into()),
                expected_message: toggle_err_hook_failed("boom"),
                expected_code: Some("MODULE_HOOK_FAILED"),
                case_name: "pre-hook-failed",
            },
            ToggleCase {
                error: ToggleModuleError::PostHookFailed("downstream timeout".into()),
                expected_message: toggle_err_hook_failed("downstream timeout"),
                expected_code: Some("MODULE_HOOK_FAILED"),
                case_name: "post-hook-failed",
            },
            ToggleCase {
                error: ToggleModuleError::Database(sea_orm::DbErr::Custom("db down".to_string())),
                expected_message: "Internal server error".to_string(),
                expected_code: Some("INTERNAL_ERROR"),
                case_name: "database",
            },
            ToggleCase {
                error: ToggleModuleError::Policy("policy".to_string()),
                expected_message: "Internal server error".to_string(),
                expected_code: Some("INTERNAL_ERROR"),
                case_name: "policy",
            },
        ]
    }

    fn toggle_user_input_error_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("BAD_USER_INPUT"))
            .collect()
    }

    fn toggle_internal_error_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("INTERNAL_ERROR"))
            .collect()
    }

    fn toggle_hook_failed_cases() -> Vec<ToggleCase> {
        toggle_error_contract_cases()
            .into_iter()
            .filter(|case| case.expected_code == Some("MODULE_HOOK_FAILED"))
            .collect()
    }

    #[test]
    fn toggle_error_taxonomy_partitions_are_disjoint_and_complete() {
        let all = toggle_error_contract_cases();
        let user = toggle_user_input_error_cases();
        let internal = toggle_internal_error_cases();

        assert_eq!(
            all.len(),
            user.len() + internal.len(),
            "toggle taxonomy partition drifted: user + internal cases must cover all cases exactly"
        );

        for user_case in &user {
            assert!(
                internal.iter().all(|case| case.case_name != user_case.case_name),
                "toggle taxonomy partition overlap detected for case: {}",
                user_case.case_name
            );
        }

        assert!(
            all.iter().all(|case| {
                case.expected_code == Some("BAD_USER_INPUT")
                    || case.expected_code == Some("MODULE_HOOK_FAILED")
                    || case.expected_code == Some("INTERNAL_ERROR")
            }),
            "toggle taxonomy contains unsupported error code category"
        );

        let mut seen_case_names = std::collections::BTreeSet::new();
        for case in &all {
            assert!(
                seen_case_names.insert(case.case_name),
                "toggle taxonomy contains duplicated case_name: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_error_mapping_sets_expected_error_codes() {
        for case in toggle_error_contract_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                case.expected_code,
                "toggle error code drifted for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_user_input_taxonomy_maps_only_to_bad_user_input_code() {
        for case in toggle_user_input_error_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("BAD_USER_INPUT"),
                "toggle user-input taxonomy must map to BAD_USER_INPUT code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_hook_failed_taxonomy_maps_only_to_module_hook_failed_code() {
        for case in toggle_hook_failed_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("MODULE_HOOK_FAILED"),
                "toggle hook-failed taxonomy must map to MODULE_HOOK_FAILED code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_internal_error_taxonomy_maps_only_to_internal_error_code() {
        for case in toggle_internal_error_cases() {
            let gql = map_toggle_module_error(case.error).extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some("INTERNAL_ERROR"),
                "toggle internal taxonomy must map to INTERNAL_ERROR code for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_internal_error_taxonomy_uses_generic_internal_message() {
        for case in toggle_internal_error_cases() {
            let mapped = map_toggle_module_error(case.error);
            assert_eq!(
                mapped.message, "Internal server error",
                "toggle internal taxonomy must not leak implementation details for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_error_mapping_matrix_preserves_message_and_code_contract() {
        for case in toggle_error_contract_cases() {
            let mapped = map_toggle_module_error(case.error);
            assert_eq!(
                mapped.message, case.expected_message,
                "toggle message drifted for case: {}",
                case.case_name
            );
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                case.expected_code,
                "toggle error code drifted for case: {}",
                case.case_name
            );
            assert!(
                !mapped.message.contains("rolled back"),
                "toggle error contract must not reference partial rollback semantics for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn toggle_hook_failed_pre_hook_sets_non_retryable_issue_extensions() {
        let mapped = map_toggle_module_error(ToggleModuleError::PreHookFailed("boom".to_string()));
        let gql = mapped.extend();

        assert_eq!(error_code(&gql).as_deref(), Some("MODULE_HOOK_FAILED"));
        assert_eq!(extension_bool(&gql, "retryable_issue"), Some(false));
        assert_eq!(
            extension_string(&gql, "operation_issue").as_deref(),
            Some("pre_hook_failed")
        );
    }

    #[test]
    fn toggle_hook_failed_post_hook_sets_retryable_issue_extensions() {
        let mapped = map_toggle_module_error(ToggleModuleError::PostHookFailed("downstream timeout".to_string()));
        let gql = mapped.extend();

        assert_eq!(error_code(&gql).as_deref(), Some("MODULE_HOOK_FAILED"));
        assert_eq!(extension_bool(&gql, "retryable_issue"), Some(true));
        assert_eq!(
            extension_string(&gql, "operation_issue").as_deref(),
            Some("post_hook_failed")
        );
    }

    #[test]
    fn toggle_error_taxonomy_matrix_stays_stable() {
        for case in toggle_user_input_error_cases() {
            let field_error = map_toggle_module_error(case.error);
            assert_eq!(
                field_error.message, case.expected_message,
                "toggle error taxonomy drifted for case: {}",
                case.case_name
            );
            assert!(
                !field_error.message.contains("rolled back"),
                "toggle error taxonomy unexpectedly references partial rollback for case: {}",
                case.case_name
            );
        }
    }

    #[test]
    fn manifest_error_maps_validation_errors_to_user_messages() {
        let err = map_manifest_error(ManifestError::RequiredModule("pages".to_string()));
        assert!(err.message.contains("required"));
    }

    #[test]
    fn platform_composition_error_maps_revision_conflict_with_expected_and_current() {
        let err = map_platform_composition_error(PlatformCompositionError::RevisionConflict {
            expected: 3,
            current: 5,
        });
        assert_eq!(
            err.message,
            "Platform composition revision conflict: expected 3, current 5"
        );
        let gql = err.extend();
        assert_eq!(
            error_code(&gql).as_deref(),
            Some("BAD_USER_INPUT"),
            "revision conflict must stay in user-facing conflict taxonomy"
        );
    }

    #[test]
    fn platform_composition_error_matrix_preserves_taxonomy_for_internal_and_user_paths() {
        struct Case {
            name: &'static str,
            error: PlatformCompositionError,
            expected_code: &'static str,
            message_fragment: &'static str,
        }

        let cases = vec![
            Case {
                name: "revision conflict",
                error: PlatformCompositionError::RevisionConflict {
                    expected: 7,
                    current: 9,
                },
                expected_code: "BAD_USER_INPUT",
                message_fragment: "revision conflict",
            },
            Case {
                name: "serialize failure",
                error: PlatformCompositionError::Serialize("serde exploded".to_string()),
                expected_code: "INTERNAL_SERVER_ERROR",
                message_fragment: "serde exploded",
            },
            Case {
                name: "deserialize failure",
                error: PlatformCompositionError::Deserialize("bad snapshot".to_string()),
                expected_code: "INTERNAL_SERVER_ERROR",
                message_fragment: "bad snapshot",
            },
            Case {
                name: "database failure",
                error: PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                    "db is unavailable".to_string(),
                )),
                expected_code: "INTERNAL_SERVER_ERROR",
                message_fragment: "db is unavailable",
            },
            Case {
                name: "manifest validation direct mapping",
                error: PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                    "pages".to_string(),
                )),
                expected_code: "BAD_USER_INPUT",
                message_fragment: "required",
            },
        ];

        for case in cases {
            let mapped = map_platform_composition_error(case.error);
            assert!(
                mapped.message.to_lowercase().contains(case.message_fragment),
                "message contract drifted for case `{}`",
                case.name
            );
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some(case.expected_code),
                "error code contract drifted for case `{}`",
                case.name
            );
            assert!(
                !mapped.message.to_lowercase().contains("rolled back"),
                "error message must not reintroduce partial rollback wording for case `{}`",
                case.name
            );
        }
    }

    #[test]
    fn platform_composition_build_error_matrix_preserves_message_and_code_contract() {
        struct Case {
            name: &'static str,
            error: PlatformCompositionBuildError,
            expected_code: &'static str,
            expected_message_fragment: &'static str,
            exact_message: Option<&'static str>,
        }

        let cases = vec![
            Case {
                name: "build enqueue failure",
                error: PlatformCompositionBuildError::Build(sea_orm::DbErr::Custom(
                    "enqueue failed".to_string(),
                )),
                expected_code: "INTERNAL_SERVER_ERROR",
                expected_message_fragment: "enqueue failed",
                exact_message: None,
            },
            Case {
                name: "manifest validation failure",
                error: PlatformCompositionBuildError::Composition(PlatformCompositionError::Manifest(
                    ManifestError::RequiredModule("pages".to_string()),
                )),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "required",
                exact_message: None,
            },
            Case {
                name: "serialize failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Serialize("serde exploded".to_string()),
                ),
                expected_code: "INTERNAL_SERVER_ERROR",
                expected_message_fragment: "serde exploded",
                exact_message: None,
            },
            Case {
                name: "deserialize failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::Deserialize("bad snapshot".to_string()),
                ),
                expected_code: "INTERNAL_SERVER_ERROR",
                expected_message_fragment: "bad snapshot",
                exact_message: None,
            },
            Case {
                name: "database failure via composition wrapper",
                error: PlatformCompositionBuildError::Composition(PlatformCompositionError::Database(
                    sea_orm::DbErr::Custom("db is unavailable".to_string()),
                )),
                expected_code: "INTERNAL_SERVER_ERROR",
                expected_message_fragment: "db is unavailable",
                exact_message: None,
            },
            Case {
                name: "build path revision conflict",
                error: PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::RevisionConflict {
                        expected: 11,
                        current: 13,
                    },
                ),
                expected_code: "BAD_USER_INPUT",
                expected_message_fragment: "revision conflict",
                exact_message: Some("Platform composition revision conflict: expected 11, current 13"),
            },
        ];

        for case in cases {
            let mapped = map_platform_composition_build_error(case.error);
            assert!(
                mapped.message.contains(case.expected_message_fragment),
                "message fragment drifted for case `{}`",
                case.name
            );
            if let Some(exact) = case.exact_message {
                assert_eq!(
                    mapped.message, exact,
                    "exact message drifted for case `{}`",
                    case.name
                );
            }
            let gql = mapped.extend();
            assert_eq!(
                error_code(&gql).as_deref(),
                Some(case.expected_code),
                "error code drifted for case `{}`",
                case.name
            );
        }
    }

    #[test]
    fn platform_composition_build_error_mapping_never_mentions_partial_rollback() {
        let errors = vec![
            PlatformCompositionBuildError::Build(sea_orm::DbErr::Custom("enqueue failed".to_string())),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Manifest(
                ManifestError::RequiredModule("pages".to_string()),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Serialize(
                "serde exploded".to_string(),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Deserialize(
                "bad snapshot".to_string(),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::Database(
                sea_orm::DbErr::Custom("db is unavailable".to_string()),
            )),
            PlatformCompositionBuildError::Composition(PlatformCompositionError::RevisionConflict {
                expected: 1,
                current: 2,
            }),
        ];

        for error in errors {
            let mapped = map_platform_composition_build_error(error);
            assert!(
                !mapped.message.to_lowercase().contains("rolled back"),
                "platform composition build error contract must not mention partial rollback semantics"
            );
        }
    }

    #[test]
    fn platform_composition_build_wrapper_preserves_composition_mapping_contract() {
        let composition_errors = vec![
            PlatformCompositionError::RevisionConflict {
                expected: 5,
                current: 8,
            },
            PlatformCompositionError::Manifest(ManifestError::RequiredModule(
                "pages".to_string(),
            )),
            PlatformCompositionError::Serialize("serde exploded".to_string()),
            PlatformCompositionError::Deserialize("bad snapshot".to_string()),
            PlatformCompositionError::Database(sea_orm::DbErr::Custom(
                "db is unavailable".to_string(),
            )),
        ];

        for composition_error in composition_errors {
            let direct = map_platform_composition_error(composition_error.clone());
            let wrapped = map_platform_composition_build_error(
                PlatformCompositionBuildError::Composition(composition_error),
            );

            assert_eq!(
                wrapped.message, direct.message,
                "build-wrapper mapping drifted from direct composition mapping"
            );

            let direct_gql = direct.extend();
            let wrapped_gql = wrapped.extend();
            assert_eq!(
                error_code(&wrapped_gql),
                error_code(&direct_gql),
                "build-wrapper error code drifted from direct composition mapping"
            );
        }
    }

    #[tokio::test]
    async fn validate_custom_fields_applies_defaults() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            Some(serde_json::json!("sales")),
        )])
        .await;

        let result = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect("defaults should be applied");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_strips_unknown_keys() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            None,
        )])
        .await;

        let result = validate_custom_fields(
            &db,
            tenant_id,
            Some(serde_json::json!({"department": "sales", "unknown": "drop"})),
        )
        .await
        .expect("unknown keys should be stripped");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_returns_input_when_schema_empty() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(Vec::<UserFieldDefinitionActiveModel>::new()).await;
        let payload = Some(serde_json::json!({"nickname": "neo"}));

        let result = validate_custom_fields(&db, tenant_id, payload.clone())
            .await
            .expect("without schema payload should pass through");

        assert_eq!(result, payload);
    }

    #[tokio::test]
    async fn validate_custom_fields_error_contains_field_details() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id, "phone", "text", false, true, None,
        )])
        .await;

        let err = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect_err("missing required field must fail");

        let fields = err
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("fields"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        assert!(!fields.is_empty());
        let first_field = &fields[0];
        let key = first_field
            .get("field_key")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let code = first_field
            .get("error_code")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(key, "phone");
        assert_eq!(code, "required");
    }
    #[tokio::test]
    async fn validate_custom_fields_applies_defaults_when_input_is_none() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id,
            "department",
            "text",
            false,
            false,
            Some(serde_json::json!("sales")),
        )])
        .await;

        let result = validate_custom_fields(&db, tenant_id, None)
            .await
            .expect("defaults should be applied for empty input");

        assert_eq!(result, Some(serde_json::json!({"department": "sales"})));
    }

    #[tokio::test]
    async fn validate_custom_fields_returns_graphql_error_for_required_field() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![field_definition_model(
            tenant_id, "phone", "text", false, true, None,
        )])
        .await;

        let err = validate_custom_fields(&db, tenant_id, Some(serde_json::json!({})))
            .await
            .expect_err("missing required field must fail");

        assert!(err.message.contains("Custom field validation failed"));
        let code = err
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default();
        assert_eq!(code, "CUSTOM_FIELD_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn prepare_user_custom_fields_write_splits_localized_values_from_metadata() {
        let tenant_id = Uuid::new_v4();
        let db = db_with_definitions(vec![
            field_definition_model(tenant_id, "nickname", "text", false, false, None),
            field_definition_model(tenant_id, "bio", "text", true, false, None),
        ])
        .await;

        let prepared = prepare_user_custom_fields_write(
            &db,
            tenant_id,
            "ru",
            None,
            None,
            Some(serde_json::json!({"nickname": "neo", "bio": "Привет"})),
        )
        .await
        .expect("custom fields should split successfully");

        assert_eq!(
            prepared.metadata,
            Some(serde_json::json!({"nickname": "neo"}))
        );
        assert_eq!(
            prepared.localized_values,
            Some(serde_json::json!({"bio": "Привет"}))
        );
        assert_eq!(prepared.locale.as_deref(), Some("ru"));
    }

}
