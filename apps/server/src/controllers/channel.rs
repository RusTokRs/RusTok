use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::{delete, get, patch, post},
};
use loco_rs::app::AppContext;
use loco_rs::controller::{ErrorDetail, Routes, format};
use rustok_channel::{
    BindChannelModuleInput, BindChannelOauthAppInput, ChannelDetailResponse,
    ChannelResolutionPolicySetDetailResponse, ChannelResponse, ChannelService,
    ChannelTargetResponse, CreateChannelInput, CreateChannelResolutionPolicySetInput,
    CreateChannelResolutionRuleInput, CreateChannelTargetInput, ResolutionAction,
    ResolutionPredicate, TargetSurface, UpdateChannelTargetInput,
};
use rustok_core::{ModuleRegistry, Permission};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::context::OptionalChannel;
use crate::error::{Error, Result};
use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::middleware::channel::invalidate_tenant_channel_cache;
use crate::models::oauth_apps;
use crate::services::rbac_service::RbacService;

#[derive(Debug, Serialize)]
struct ChannelBootstrapResponse {
    current_channel: Option<crate::context::ChannelContext>,
    channels: Vec<ChannelDetailResponse>,
    policy_sets: Vec<ChannelResolutionPolicySetDetailResponse>,
    available_modules: Vec<AvailableModuleItem>,
    oauth_apps: Vec<AvailableOauthAppItem>,
}

#[derive(Debug, Deserialize)]
struct CreateResolutionPolicySetRequest {
    slug: String,
    name: String,
    is_active: bool,
}

#[derive(Debug, Deserialize)]
struct CreateResolutionRuleRequest {
    priority: i32,
    is_active: bool,
    action_channel_id: Uuid,
    host_equals: Option<String>,
    host_suffix: Option<String>,
    oauth_app_id: Option<Uuid>,
    surface: Option<String>,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct AvailableModuleItem {
    slug: String,
    name: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct AvailableOauthAppItem {
    id: Uuid,
    name: String,
    slug: String,
    app_type: String,
    is_active: bool,
}

async fn bootstrap(
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    OptionalChannel(current_channel): OptionalChannel,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let channels = service
        .list_channel_details(tenant.id)
        .await
        .map_err(internal_error)?;
    let policy_sets = service
        .list_resolution_policy_sets(tenant.id)
        .await
        .map_err(internal_error)?;

    let mut available_modules = registry
        .list()
        .into_iter()
        .map(|module| AvailableModuleItem {
            slug: module.slug().to_string(),
            name: module.name().to_string(),
            kind: if registry.is_core(module.slug()) {
                "core".to_string()
            } else {
                "optional".to_string()
            },
        })
        .collect::<Vec<_>>();
    available_modules.sort_by(|left, right| left.slug.cmp(&right.slug));

    let mut oauth_apps = oauth_apps::Entity::find_active_by_tenant(&ctx.db, tenant.id)
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(|app| AvailableOauthAppItem {
            id: app.id,
            name: app.name.clone(),
            slug: app.slug.clone(),
            app_type: app.app_type.clone(),
            is_active: app.is_active(),
        })
        .collect::<Vec<_>>();
    oauth_apps.sort_by(|left, right| left.slug.cmp(&right.slug));

    format::json(ChannelBootstrapResponse {
        current_channel,
        channels,
        policy_sets,
        available_modules,
        oauth_apps,
    })
}

async fn create_channel(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(input): Json<CreateChannelInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let channel = service
        .create_channel(CreateChannelInput {
            tenant_id: tenant.id,
            slug: input.slug,
            name: input.name,
            settings: input.settings,
        })
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(channel)
}

async fn create_target(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<CreateChannelTargetInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let target: ChannelTargetResponse = service
        .add_target(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(target)
}

async fn set_default_channel(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let channel = service
        .set_default_channel(channel_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(channel)
}

async fn update_target(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateChannelTargetInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let target: ChannelTargetResponse = service
        .update_target(channel_id, target_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(target)
}

async fn delete_target(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let target: ChannelTargetResponse = service
        .delete_target(channel_id, target_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(target)
}

async fn bind_module(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<BindChannelModuleInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let binding = service
        .bind_module(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(binding)
}

async fn bind_oauth_app(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<BindChannelOauthAppInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let oauth_apps = oauth_apps::Entity::find_active_by_tenant(&ctx.db, tenant.id)
        .await
        .map_err(internal_error)?;
    if !oauth_apps.iter().any(|app| app.id == input.oauth_app_id) {
        return Err(Error::BadRequest(
            "OAuth app does not belong to the current tenant".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db.clone());
    let binding = service
        .bind_oauth_app(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(binding)
}

async fn delete_module_binding(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let binding = service
        .remove_module_binding(channel_id, binding_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(binding)
}

async fn delete_oauth_app_binding(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let binding = service
        .revoke_oauth_app_binding(channel_id, binding_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(binding)
}

async fn create_resolution_policy_set(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(input): Json<CreateResolutionPolicySetRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let policy_set = service
        .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
            tenant_id: tenant.id,
            slug: input.slug,
            name: input.name,
            is_active: input.is_active,
        })
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(policy_set)
}

async fn create_resolution_rule(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
    Json(input): Json<CreateResolutionRuleRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, input.action_channel_id).await?;

    let (priority, is_active, definition) =
        build_rule_definition(input).map_err(Error::BadRequest)?;
    let service = ChannelService::new(ctx.db.clone());
    let rule = service
        .create_resolution_rule(
            policy_set_id,
            CreateChannelResolutionRuleInput {
                priority,
                is_active,
                definition,
            },
        )
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(rule)
}

async fn activate_resolution_policy_set(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let policy_set = service
        .set_active_resolution_policy_set(policy_set_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(policy_set)
}

async fn delete_resolution_rule(
    State(ctx): State<AppContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((policy_set_id, rule_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db.clone());
    let rule = service
        .remove_resolution_rule(policy_set_id, rule_id)
        .await
        .map_err(internal_error)?;
    invalidate_tenant_channel_cache(&ctx, tenant.id).await;

    format::json(rule)
}

async fn ensure_channel_manage_access(
    ctx: &AppContext,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    let allowed = RbacService::has_any_permission(
        &ctx.db,
        &tenant_id,
        &user_id,
        &[Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE],
    )
    .await
    .map_err(|error| {
        tracing::error!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            %error,
            "Failed to evaluate RBAC permissions for channel management"
        );
        Error::InternalServerError
    })?;

    if !allowed {
        return Err(forbidden_error(
            "Permission denied: settings:manage or modules:manage required",
        ));
    }

    Ok(())
}

async fn ensure_channel_belongs_to_tenant(
    ctx: &AppContext,
    tenant_id: Uuid,
    channel_id: Uuid,
) -> Result<ChannelResponse> {
    let service = ChannelService::new(ctx.db.clone());
    let channel = service
        .get_channel(channel_id)
        .await
        .map_err(internal_error)?;
    if channel.tenant_id != tenant_id {
        return Err(Error::NotFound);
    }
    Ok(channel)
}

async fn ensure_policy_set_belongs_to_tenant(
    ctx: &AppContext,
    tenant_id: Uuid,
    policy_set_id: Uuid,
) -> Result<()> {
    let service = ChannelService::new(ctx.db.clone());
    let policy_set = service
        .get_resolution_policy_set(policy_set_id)
        .await
        .map_err(internal_error)?;
    if policy_set.tenant_id != tenant_id {
        return Err(Error::NotFound);
    }
    Ok(())
}

fn build_rule_definition(
    input: CreateResolutionRuleRequest,
) -> std::result::Result<(i32, bool, rustok_channel::ChannelResolutionRuleDefinition), String> {
    let CreateResolutionRuleRequest {
        priority,
        is_active,
        action_channel_id,
        host_equals,
        host_suffix,
        oauth_app_id,
        surface,
        locale,
    } = input;
    let mut predicates = Vec::new();

    if let Some(host_equals) = host_equals.filter(|value| !value.trim().is_empty()) {
        predicates.push(ResolutionPredicate::HostEquals(host_equals));
    }
    if let Some(host_suffix) = host_suffix.filter(|value| !value.trim().is_empty()) {
        predicates.push(ResolutionPredicate::HostSuffix(host_suffix));
    }
    if let Some(oauth_app_id) = oauth_app_id {
        predicates.push(ResolutionPredicate::OAuthAppEquals(oauth_app_id));
    }
    if let Some(surface) = surface
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let surface = match surface {
            "http" => TargetSurface::Http,
            other => {
                return Err(format!(
                    "Unsupported surface `{other}`; only `http` is currently supported"
                ));
            }
        };
        predicates.push(ResolutionPredicate::SurfaceIs(surface));
    }
    if let Some(locale) = locale.filter(|value| !value.trim().is_empty()) {
        predicates.push(ResolutionPredicate::LocaleEquals(locale));
    }

    Ok((
        priority,
        is_active,
        rustok_channel::ChannelResolutionRuleDefinition {
            predicates,
            action: ResolutionAction::ResolveToChannel {
                channel_id: action_channel_id,
            },
        },
    ))
}

fn internal_error(error: impl std::fmt::Display) -> Error {
    Error::Message(error.to_string())
}

fn forbidden_error(description: impl Into<String>) -> Error {
    let description = description.into();
    Error::CustomError(
        StatusCode::FORBIDDEN,
        ErrorDetail::new("forbidden", description.as_str()),
    )
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/channels")
        .add("/bootstrap", get(bootstrap))
        .add("/", post(create_channel))
        .add("/{channel_id}/default", post(set_default_channel))
        .add("/{channel_id}/targets", post(create_target))
        .add("/{channel_id}/targets/{target_id}", patch(update_target))
        .add("/{channel_id}/targets/{target_id}", delete(delete_target))
        .add("/{channel_id}/modules", post(bind_module))
        .add(
            "/{channel_id}/modules/{binding_id}",
            delete(delete_module_binding),
        )
        .add("/{channel_id}/oauth-apps", post(bind_oauth_app))
        .add(
            "/{channel_id}/oauth-apps/{binding_id}",
            delete(delete_oauth_app_binding),
        )
        .add("/policies", post(create_resolution_policy_set))
        .add(
            "/policies/{policy_set_id}/activate",
            post(activate_resolution_policy_set),
        )
        .add(
            "/policies/{policy_set_id}/rules",
            post(create_resolution_rule),
        )
        .add(
            "/policies/{policy_set_id}/rules/{rule_id}",
            delete(delete_resolution_rule),
        )
}
