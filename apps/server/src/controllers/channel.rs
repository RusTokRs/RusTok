use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::{delete, get, patch, post},
};
use rustok_api::Permission;
use rustok_channel::{
    AvailableChannelModuleItem, AvailableChannelOauthAppItem, BindChannelModuleInput,
    BindChannelOauthAppInput, ChannelBootstrapResponse, ChannelResponse, ChannelService,
    ChannelTargetResponse, CreateChannelRequest, CreateChannelTargetInput,
    ReorderChannelResolutionRulesInput, ReorderResolutionRulesRequest, UpdateChannelTargetInput,
    create_channel_input, create_resolution_policy_set_input, create_resolution_rule_input,
    update_resolution_rule_input,
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        current_channel_id = ?current_channel.as_ref().map(|c| c.id),
        "Handling channel bootstrap request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let channels = service
        .list_channel_details(tenant.id)
        .await
        .map_err(map_channel_error)?;
    let policy_sets = service
        .list_resolution_policy_sets(tenant.id)
        .await
        .map_err(map_channel_error)?;

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
        .map_err(Error::from)?
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
    Json(mut input): Json<CreateChannelRequest>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        slug = %input.slug,
        "Handling create channel request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    input.slug = input.slug.trim().to_string();
    input.name = input.name.trim().to_string();
    if input.slug.is_empty() || input.name.is_empty() {
        return Err(Error::BadRequest(
            "Channel slug and name must not be empty".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let channel = service
        .create_channel(create_channel_input(tenant.id, input))
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(channel))
}

async fn create_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(mut input): Json<CreateChannelTargetInput>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        target_type = %input.target_type,
        "Handling create channel target request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    input.target_type = input.target_type.trim().to_string();
    input.value = input.value.trim().to_string();
    if input.target_type.is_empty() || input.value.is_empty() {
        return Err(Error::BadRequest(
            "Target target_type and value must not be empty".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .add_target(channel_id, input)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn set_default_channel(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        "Handling set default channel request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let channel = service
        .set_default_channel(channel_id)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(channel))
}

async fn update_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
    Json(mut input): Json<UpdateChannelTargetInput>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        target_id = %target_id,
        "Handling update channel target request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    input.target_type = input.target_type.trim().to_string();
    input.value = input.value.trim().to_string();
    if input.target_type.is_empty() || input.value.is_empty() {
        return Err(Error::BadRequest(
            "Target target_type and value must not be empty".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .update_target(channel_id, target_id, input)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn delete_target(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, target_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        target_id = %target_id,
        "Handling delete channel target request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let target: ChannelTargetResponse = service
        .delete_target(channel_id, target_id)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(target))
}

async fn bind_module(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(channel_id): Path<Uuid>,
    Json(mut input): Json<BindChannelModuleInput>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        module_slug = %input.module_slug,
        "Handling bind module to channel request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    input.module_slug = input.module_slug.trim().to_string();
    if input.module_slug.is_empty() {
        return Err(Error::BadRequest(
            "module_slug must not be empty".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .bind_module(channel_id, input)
        .await
        .map_err(map_channel_error)?;
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        oauth_app_id = %input.oauth_app_id,
        "Handling bind oauth app to channel request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;
    ensure_oauth_app_belongs_to_tenant(&ctx, tenant.id, input.oauth_app_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .bind_oauth_app(channel_id, input)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn delete_module_binding(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        binding_id = %binding_id,
        "Handling delete module binding request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .remove_module_binding(channel_id, binding_id)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn delete_oauth_app_binding(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((channel_id, binding_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        channel_id = %channel_id,
        binding_id = %binding_id,
        "Handling delete oauth app binding request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, channel_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let binding = service
        .revoke_oauth_app_binding(channel_id, binding_id)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(binding))
}

async fn create_resolution_policy_set(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(mut input): Json<rustok_channel::CreateResolutionPolicySetRequest>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        slug = %input.slug,
        "Handling create resolution policy set request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;

    input.slug = input.slug.trim().to_string();
    input.name = input.name.trim().to_string();
    if input.slug.is_empty() || input.name.is_empty() {
        return Err(Error::BadRequest(
            "Policy set slug and name must not be empty".to_string(),
        ));
    }

    let service = ChannelService::new(ctx.db_clone());
    let policy_set = service
        .create_resolution_policy_set(create_resolution_policy_set_input(tenant.id, input))
        .await
        .map_err(map_channel_error)?;
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        policy_set_id = %policy_set_id,
        action_channel_id = %input.action_channel_id,
        "Handling create resolution rule request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;
    ensure_channel_belongs_to_tenant(&ctx, tenant.id, input.action_channel_id).await?;

    if let Some(oauth_app_id) = input.oauth_app_id {
        ensure_oauth_app_belongs_to_tenant(&ctx, tenant.id, oauth_app_id).await?;
    }

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .create_resolution_rule(
            policy_set_id,
            create_resolution_rule_input(input).map_err(Error::BadRequest)?,
        )
        .await
        .map_err(map_channel_error)?;
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        policy_set_id = %policy_set_id,
        rule_id = %rule_id,
        "Handling update resolution rule request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    if let Some(action_channel_id) = input.action_channel_id {
        ensure_channel_belongs_to_tenant(&ctx, tenant.id, action_channel_id).await?;
    }

    if let Some(ref oauth_app_id_str) = input.oauth_app_id {
        let trimmed = oauth_app_id_str.trim();
        if !trimmed.is_empty() {
            let app_uuid = Uuid::parse_str(trimmed)
                .map_err(|_| Error::BadRequest("Invalid oauth_app_id UUID".to_string()))?;
            ensure_oauth_app_belongs_to_tenant(&ctx, tenant.id, app_uuid).await?;
        }
    }

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .update_resolution_rule(policy_set_id, rule_id, update_resolution_rule_input(input))
        .await
        .map_err(map_channel_error)?;
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
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        policy_set_id = %policy_set_id,
        rule_count = input.rule_ids.len(),
        "Handling reorder resolution rules request"
    );

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
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(rules))
}

async fn activate_resolution_policy_set(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(policy_set_id): Path<Uuid>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        policy_set_id = %policy_set_id,
        "Handling activate resolution policy set request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let policy_set = service
        .set_active_resolution_policy_set(policy_set_id)
        .await
        .map_err(map_channel_error)?;
    invalidate_channel_resolution_cache(&ctx, tenant.id).await;

    Ok(json_response(policy_set))
}

async fn delete_resolution_rule(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((policy_set_id, rule_id)): Path<(Uuid, Uuid)>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        policy_set_id = %policy_set_id,
        rule_id = %rule_id,
        "Handling delete resolution rule request"
    );

    ensure_channel_manage_access(&ctx, tenant.id, current.user.id).await?;
    ensure_policy_set_belongs_to_tenant(&ctx, tenant.id, policy_set_id).await?;

    let service = ChannelService::new(ctx.db_clone());
    let rule = service
        .remove_resolution_rule(policy_set_id, rule_id)
        .await
        .map_err(map_channel_error)?;
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
        .map_err(map_channel_error)?;
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
        .map_err(map_channel_error)?;
    if policy_set.tenant_id != tenant_id {
        return Err(Error::NotFound);
    }
    Ok(())
}

async fn ensure_oauth_app_belongs_to_tenant(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    oauth_app_id: Uuid,
) -> Result<()> {
    let oauth_apps = oauth_apps::Entity::find_active_by_tenant(ctx.db(), tenant_id)
        .await
        .map_err(Error::from)?;
    if !oauth_apps.iter().any(|app| app.id == oauth_app_id) {
        return Err(Error::BadRequest(
            "OAuth app does not belong to the current tenant".to_string(),
        ));
    }
    Ok(())
}

fn map_channel_error(error: rustok_channel::ChannelError) -> Error {
    use rustok_channel::ChannelError;

    match error {
        ChannelError::NotFound(_) => Error::NotFound,
        ChannelError::Validation(message)
        | ChannelError::InvalidTargetType(message)
        | ChannelError::InvalidTargetValue(message)
        | ChannelError::InvalidPolicyDefinition(message)
        | ChannelError::InvalidPolicyOperation(message) => Error::BadRequest(message),
        ChannelError::InactiveChannel(_) => http_error(rustok_web::HttpError::new(
            StatusCode::CONFLICT,
            "channel_inactive",
            "Channel is not active",
        )),
        ChannelError::SlugAlreadyExists(_)
        | ChannelError::TargetAlreadyExists(_, _)
        | ChannelError::PolicySetSlugAlreadyExists(_) => http_error(rustok_web::HttpError::new(
            StatusCode::CONFLICT,
            "channel_conflict",
            "The requested channel resource conflicts with an existing resource",
        )),
        ChannelError::Database(error) => {
            tracing::error!(%error, "Channel controller database operation failed");
            Error::InternalServerError
        }
        ChannelError::Serialization(error) => {
            tracing::error!(%error, "Channel controller serialization failed");
            Error::InternalServerError
        }
    }
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
        .route("/api/channels", post(create_channel))
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

#[cfg(test)]
mod tests {
    use super::map_channel_error;
    use axum::{http::StatusCode, response::IntoResponse};
    use rustok_channel::ChannelError;
    use uuid::Uuid;

    #[test]
    fn maps_channel_not_found_to_not_found() {
        let response = map_channel_error(ChannelError::NotFound(Uuid::new_v4())).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn maps_channel_validation_to_bad_request() {
        let response = map_channel_error(ChannelError::InvalidTargetValue(
            "invalid target".to_string(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn maps_channel_conflicts_to_conflict() {
        let response = map_channel_error(ChannelError::SlugAlreadyExists("default".to_string()))
            .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = map_channel_error(ChannelError::TargetAlreadyExists(
            "web_domain".to_string(),
            "shop.example.test".to_string(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = map_channel_error(ChannelError::PolicySetSlugAlreadyExists(
            "default".to_string(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn maps_inactive_channel_to_conflict() {
        let response =
            map_channel_error(ChannelError::InactiveChannel(Uuid::new_v4())).into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn maps_channel_database_error_to_internal_server_error() {
        let response = map_channel_error(ChannelError::Database(sea_orm::DbErr::Custom(
            "database failure".to_string(),
        )))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn create_channel_request_converts_with_tenant_isolation() {
        let tenant_id = Uuid::new_v4();
        let request: rustok_channel::CreateChannelRequest =
            serde_json::from_str(r#"{"slug":"online-store","name":"Online Store"}"#)
                .expect("valid create channel request JSON");

        assert_eq!(request.slug, "online-store");
        assert_eq!(request.name, "Online Store");
        assert!(request.settings.is_none());

        let input = rustok_channel::create_channel_input(tenant_id, request);
        assert_eq!(input.tenant_id, tenant_id);
        assert_eq!(input.slug, "online-store");
        assert_eq!(input.name, "Online Store");
    }

    #[test]
    fn maps_invalid_policy_definition_to_bad_request() {
        let response = map_channel_error(ChannelError::InvalidPolicyDefinition(
            "syntax error".to_string(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn maps_channel_serialization_error_to_internal_server_error() {
        let response = map_channel_error(ChannelError::Serialization(
            serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn input_trimming_rejects_empty_or_whitespace() {
        let raw_slug = "   ";
        let raw_name = "\t\n ";
        assert!(raw_slug.trim().is_empty());
        assert!(raw_name.trim().is_empty());

        let valid_slug = "  web-store  ";
        assert_eq!(valid_slug.trim(), "web-store");
    }
}
