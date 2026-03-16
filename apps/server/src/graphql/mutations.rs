use async_graphql::{Context, FieldError, Object, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

use crate::auth::hash_password;
use crate::context::{AuthContext, TenantContext};
use crate::graphql::errors::GraphQLError;
use crate::graphql::types::{
    BuildJob, CreateUserInput, DeleteUserPayload, TenantModule, UpdateUserInput, User,
};
use crate::models::_entities::users::Column as UsersColumn;
use crate::models::release::{Column as ReleaseColumn, Entity as ReleaseEntity, ReleaseStatus};
use crate::models::users;
use crate::modules::{ManifestDiff, ManifestError, ManifestManager, ModulesManifest};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::build_event_hub::{
    build_event_hub_from_context, BuildEventHubPublisher, CompositeBuildEventPublisher,
};
use crate::services::build_service::EventBusBuildEventPublisher;
use crate::services::build_service::{BuildRequest, BuildService};
use crate::services::event_bus::event_bus_from_context;
use crate::services::module_lifecycle::{ModuleLifecycleService, ToggleModuleError};
use crate::services::rbac_service::RbacService;
use crate::services::user_field_service::UserFieldService;
use rustok_core::{Action, ModuleRegistry, Permission, Resource};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub struct RootMutation;

/// Validate `custom_fields` against the active Flex schema for the tenant.
///
/// Applies defaults, then validates. Returns the processed metadata on success,
/// or a [`FieldError`] listing all validation failures.
async fn validate_custom_fields(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    custom_fields: Option<serde_json::Value>,
) -> Result<Option<serde_json::Value>> {
    let schema = UserFieldService::get_schema(db, tenant_id)
        .await
        .map_err(|e| <FieldError as GraphQLError>::internal_error(&e.to_string()))?;

    // Nothing to validate when no schema and no input
    if schema.active_definitions().is_empty() {
        return Ok(custom_fields);
    }

    let mut metadata = custom_fields.unwrap_or(serde_json::json!({}));

    schema.apply_defaults(&mut metadata);

    let errors = schema.validate(&metadata);
    if !errors.is_empty() {
        let messages: Vec<String> = errors
            .iter()
            .map(|e| format!("{}: {}", e.field_key, e.message))
            .collect();
        return Err(FieldError::new(format!(
            "Custom field validation failed: {}",
            messages.join("; ")
        ))
        .extend_with(|_, ext| {
            ext.set("code", "CUSTOM_FIELD_VALIDATION_FAILED");
            if let Ok(v) = serde_json::to_value(&errors) {
                ext.set("fields", v);
            }
        }));
    }

    Ok(Some(metadata))
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
        | ManifestError::MissingInRegistry(_)
        | ManifestError::RequiredMismatch(_)
        | ManifestError::DependencyMismatch(_)
        | ManifestError::MissingModulePackageManifest { .. }
        | ManifestError::InvalidModuleOwnership { .. }
        | ManifestError::InvalidModuleTrustLevel { .. }
        | ManifestError::InvalidModuleAdminSurface { .. }
        | ManifestError::ConflictingModuleAdminSurface { .. } => FieldError::new(err.to_string()),
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
        &Permission::new(Resource::Modules, Action::Manage),
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

async fn request_build_for_manifest(
    app_ctx: &loco_rs::app::AppContext,
    tenant_id: Uuid,
    manifest: &ModulesManifest,
    manifest_diff: &ManifestDiff,
    requested_by: &str,
    reason: &str,
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

    let build = BuildService::with_event_publisher(app_ctx.db.clone(), event_publisher)
        .request_build(BuildRequest {
            manifest_ref: ManifestManager::manifest_ref(),
            requested_by: requested_by.to_string(),
            reason: Some(reason.to_string()),
            modules_delta: manifest_diff.summary(),
            modules: ManifestManager::build_modules(manifest),
            profile: ManifestManager::deployment_profile(manifest),
            execution_plan: ManifestManager::build_execution_plan(manifest),
        })
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

    Ok(BuildJob::from_model(&build))
}

async fn persist_manifest_and_request_build(
    app_ctx: &loco_rs::app::AppContext,
    tenant_id: Uuid,
    registry: &ModuleRegistry,
    original_manifest: ModulesManifest,
    manifest: ModulesManifest,
    manifest_diff: ManifestDiff,
    requested_by: &str,
    reason: String,
) -> Result<BuildJob> {
    ManifestManager::validate_with_registry(&manifest, registry).map_err(map_manifest_error)?;
    ManifestManager::save(&manifest).map_err(map_manifest_error)?;

    match request_build_for_manifest(
        app_ctx,
        tenant_id,
        &manifest,
        &manifest_diff,
        requested_by,
        &reason,
    )
    .await
    {
        Ok(build) => Ok(build),
        Err(err) => {
            if let Err(restore_err) = ManifestManager::save(&original_manifest) {
                return Err(<FieldError as GraphQLError>::internal_error(&format!(
                    "failed to request build after manifest update: {:?}; rollback failed: {:?}",
                    err, restore_err
                )));
            }
            Err(err)
        }
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

        // Validate custom_fields before creating the user (fail fast)
        let validated_metadata =
            validate_custom_fields(&app_ctx.db, tenant.id, input.custom_fields).await?;

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

        // Apply validated custom_fields to metadata
        if let Some(metadata) = validated_metadata {
            let mut active: users::ActiveModel = user.into();
            active.metadata = Set(metadata);
            user = active
                .update(&app_ctx.db)
                .await
                .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;
        }

        Ok(User::from(&user))
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

        // Validate and apply custom_fields if provided
        let validated_metadata =
            validate_custom_fields(&app_ctx.db, tenant.id, input.custom_fields).await?;

        if let Some(metadata) = validated_metadata {
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

        let user = users::Entity::find_by_id(id)
            .filter(UsersColumn::TenantId.eq(tenant.id))
            .one(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?
            .ok_or_else(|| FieldError::new("User not found"))?;

        let model: users::ActiveModel = user.into();
        model
            .delete(&app_ctx.db)
            .await
            .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        Ok(DeleteUserPayload { success: true })
    }

    async fn install_module(
        &self,
        ctx: &Context<'_>,
        slug: String,
        version: String,
    ) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let mut manifest = ManifestManager::load().map_err(map_manifest_error)?;
        let original_manifest = manifest.clone();
        let manifest_diff =
            ManifestManager::install_builtin_module(&mut manifest, &slug, Some(version))
                .map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            original_manifest,
            manifest,
            manifest_diff,
            &auth.user_id.to_string(),
            format!("install module {slug}"),
        )
        .await
    }

    async fn uninstall_module(&self, ctx: &Context<'_>, slug: String) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let mut manifest = ManifestManager::load().map_err(map_manifest_error)?;
        let original_manifest = manifest.clone();
        let manifest_diff =
            ManifestManager::uninstall_module(&mut manifest, &slug).map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            original_manifest,
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
    ) -> Result<BuildJob> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let app_ctx = ctx.data::<loco_rs::app::AppContext>()?;
        let registry = ctx.data::<ModuleRegistry>()?;

        let mut manifest = ManifestManager::load().map_err(map_manifest_error)?;
        let original_manifest = manifest.clone();
        let manifest_diff = ManifestManager::upgrade_module(&mut manifest, &slug, version)
            .map_err(map_manifest_error)?;

        persist_manifest_and_request_build(
            app_ctx,
            tenant.id,
            registry,
            original_manifest,
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
            &Permission::new(Resource::Modules, Action::Manage),
        )
        .await
        .map_err(|err| <FieldError as GraphQLError>::internal_error(&err.to_string()))?;

        if !can_manage_modules {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            ));
        }

        let registry = ctx.data::<ModuleRegistry>()?;

        let module = ModuleLifecycleService::toggle_module(
            &app_ctx.db,
            registry,
            tenant.id,
            &module_slug,
            enabled,
        )
        .await
        .map_err(|err| match err {
            ToggleModuleError::UnknownModule => FieldError::new("Unknown module"),
            ToggleModuleError::CoreModuleCannotBeDisabled(module_slug) => {
                FieldError::new(format!("Core module cannot be disabled: {}", module_slug))
            }
            ToggleModuleError::MissingDependencies(missing) => {
                FieldError::new(format!("Missing module dependencies: {}", missing))
            }
            ToggleModuleError::HasDependents(dependents) => {
                FieldError::new(format!("Module is required by: {}", dependents))
            }
            ToggleModuleError::Database(err) => {
                <FieldError as GraphQLError>::internal_error(&err.to_string())
            }
            ToggleModuleError::HookFailed(err) => FieldError::new(format!(
                "Module lifecycle hook failed, state rolled back: {}",
                err
            )),
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
    use super::{map_create_user_error, map_manifest_error, AuthLifecycleError, ManifestError};

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
    fn manifest_error_maps_validation_errors_to_user_messages() {
        let err = map_manifest_error(ManifestError::RequiredModule("pages".to_string()));
        assert!(err.message.contains("required"));
    }
}
