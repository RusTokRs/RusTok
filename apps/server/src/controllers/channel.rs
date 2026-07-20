use axum::{
    Extension, Json,
    extract::{Path, State},
    response::Response,
    routing::{delete, get, patch, post},
};
use rustok_api::Permission;
use rustok_channel::{
    AvailableChannelModuleItem, AvailableChannelOauthAppItem, BindChannelModuleInput,
    BindChannelOauthAppInput, ChannelBootstrapResponse, ChannelResponse, ChannelService,
    ChannelTargetResponse, CreateChannelInput, CreateChannelTargetInput,
    ReorderChannelResolutionRulesInput, ReorderResolutionRulesRequest, UpdateChannelTargetInput,
    create_resolution_policy_set_input, create_resolution_rule_input, update_resolution_rule_input,
};
use rustok_core::ModuleRegistry;
use rustok_web::json_response;
use uuid::Uuid;

use crate::context::OptionalChannel;
use crate::error::{Error, Result, http_error};
use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::middleware::channel::invalidate_tenant_channel_cache;
use crate::models::oauth_apps;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

async fn bootstrap(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    OptionalChannel(current_channel): OptionalChannel,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db_clone());
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
        .map(|module| AvailableChannelModuleItem {
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

    let mut oauth_apps = oauth_apps::Entity::find_active_by_tenant(ctx.db(), tenant.id)
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(|app| AvailableChannelOauthAppItem {
            id: app.id,
            name: app.name.clone(),
            slug: app.slug.clone(),
            app_type: app.app_type.clone(),
            is_active: app.is_active(),
        })
        .collect::<Vec<_>>();
    oauth_apps.sort_by(|left, right| left.slug.cmp(&right.slug));

    Ok(json_response(ChannelBootstrapResponse::<
        crate::context::ChannelContext,
    > {
        current_channel,
        channels,
        policy_sets,
        available_modules,
        oauth_apps,
    }))
}

async fn create_channel(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(input): Json<CreateChannelInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let channel = service
        .create_channel(CreateChannelInput {
            tenant_id: tenant.id,
            slug: input.slug,
            name: input.name,
            settings: input.settings,
        })
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(channel))
}

async fn create_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<CreateChannelTargetInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .add_target(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn set_default_channel(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let channel = service
        .set_default_channel(channel_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(channel))
}

async fn update_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateChannelTargetInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .update_target(channel_id, target_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn delete_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .delete_target(channel_id, target_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn bind_module(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<BindChannelModuleInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .bind_module(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn bind_oauth_app(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(input): Json<BindChannelOauthAppInput>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let oauth_apps = oauth_apps::Entity::find_active_by_tenant(ctx.db(), tenant.id)
        .await
        .map_err(internal_error)?;
    if !oauth_apps.iter().any(|app| app.id == input.oauth_app_id) {
        return Err(Error::BadRequest(
            "OAuth app does not belong to the current tenant".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .bind_oauth_app(channel_id, input)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn delete_module_binding(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .remove_module_binding(channel_id, binding_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn delete_oauth_app_binding(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .revoke_oauth_app_binding(channel_id, binding_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn create_resolution_policy_set(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(input): Json<rustok_channel::CreateResolutionPolicySetRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let policy_set = service
        .create_resolution_policy_set(create_resolution_policy_set_input(tenant.id, input))
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(policy_set))
}

async fn create_resolution_rule(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
    Json(input): Json<rustok_channel::CreateResolutionRuleRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, input.action_channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .create_resolution_rule(
            policy_set_id,
            create_resolution_rule_input(input).map_err(Error::BadRequest)?,
        )
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(rule))
}

async fn update_resolution_rule(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((policy_set_id, rule_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<rustok_channel::UpdateResolutionRuleRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    if let Some(action_channel_id) = input.action_channel_id {
        ensure_channel_belongs_to_tenant(&ctx, tenant.id, action_channel_id).await?;
    }

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .update_resolution_rule(policy_set_id, rule_id, update_resolution_rule_input(input))
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(rule))
}

async fn reorder_resolution_rules(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
    Json(input): Json<ReorderResolutionRulesRequest>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let rules = service
        .reorder_resolution_rules(
            policy_set_id,
            ReorderChannelResolutionRulesInput {
                rule_ids: input.rule_ids,
            },
        )
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(rules))
}

async fn activate_resolution_policy_set(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let policy_set = service
        .set_active_resolution_policy_set(policy_set_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(policy_set))
}

async fn delete_resolution_rule(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((policy_set_id, rule_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .remove_resolution_rule(policy_set_id, rule_id)
        .await
        .map_err(internal_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(rule))
}

async fn ensure_channel_manage_access(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<()> {
    let allowed = RbacService::has_any_permission(
        ctx.db(),
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
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    channel_id: Uuid,
) -> Result<ChannelResponse> {
    let service = ChannelService::new(ctx.db_clone());
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
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    policy_set_id: Uuid,
) -> Result<()> {
    let service = ChannelService::new(ctx.db_clone());
    let policy_set = service
        .get_resolution_policy_set(policy_set_id)
        .await
        .map_err(internal_error)?;
    if policy_set.tenant_id != tenant_id {
        return Err(Error::NotFound);
    }
    Ok(())
}

fn internal_error(error: impl std::fmt::Display) -> Error {
    Error::Message(error.to_string())
}

fn forbidden_error(description: impl Into<String>) -> Error {
    http_error(rustok_web::HttpError::forbidden("forbidden", description))
}

async fn invalidate_channel_resolution_cache(ctx: &ServerRuntimeContext, tenant_id: Uuid) {
    invalidate_tenant_channel_cache(ctx, tenant_id).await;
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/channels/bootstrap", get(bootstrap))
        .route("/api/channels/", post(create_channel))
        .route(
            "/api/channels/{channel_id}/default",
            post(set_default_channel),
        )
        .route("/api/channels/{channel_id}/targets", post(create_target))
        .route(
            "/api/channels/{channel_id}/targets/{target_id}",
            patch(update_target).delete(delete_target),
        )
        .route("/api/channels/{channel_id}/modules", post(bind_module))
        .route(
            "/api/channels/{channel_id}/modules/{binding_id}",
            delete(delete_module_binding),
        )
        .route(
            "/api/channels/{channel_id}/oauth-apps",
            post(bind_oauth_app),
        )
        .route(
            "/api/channels/{channel_id}/oauth-apps/{binding_id}",
            delete(delete_oauth_app_binding),
        )
        .route("/api/channels/policies", post(create_resolution_policy_set))
        .route(
            "/api/channels/policies/{policy_set_id}/activate",
            post(activate_resolution_policy_set),
        )
        .route(
            "/api/channels/policies/{policy_set_id}/rules",
            post(create_resolution_rule),
        )
        .route(
            "/api/channels/policies/{policy_set_id}/rules/reorder",
            post(reorder_resolution_rules),
        )
        .route(
            "/api/channels/policies/{policy_set_id}/rules/{rule_id}",
            patch(update_resolution_rule).delete(delete_resolution_rule),
        )
}
