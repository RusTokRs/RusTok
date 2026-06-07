use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::model::{
    AvailableModuleItem, AvailableOauthAppItem, ChannelDetail, ChannelResolutionActionRecord,
    ChannelResolutionPolicySetDetail, ChannelResolutionPolicySetRecord,
    ChannelResolutionPredicateRecord, ChannelResolutionRuleDefinitionRecord,
    ChannelResolutionRuleRecord, ResolvedChannelContext,
};
use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap,
    ChannelModuleBindingRecord, ChannelOauthAppRecord, ChannelRecord,
    ChannelResolutionPolicySetDetail, ChannelTargetRecord, CreateChannelPayload,
    CreateChannelTargetPayload, CreateResolutionPolicySetPayload, CreateResolutionRulePayload,
    ReorderResolutionRulesPayload, UpdateResolutionRulePayload,
};

#[cfg(feature = "ssr")]
fn ensure_manage_permission(permissions: &[rustok_core::Permission]) -> Result<(), ServerFnError> {
    use rustok_api::has_any_effective_permission;
    use rustok_core::Permission;

    if !has_any_effective_permission(
        permissions,
        &[Permission::SETTINGS_MANAGE, Permission::MODULES_MANAGE],
    ) {
        return Err(ServerFnError::new(
            "Permission denied: settings:manage or modules:manage required",
        ));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
async fn ensure_channel_belongs_to_tenant(
    service: &rustok_channel::ChannelService,
    tenant_id: uuid::Uuid,
    channel_id: uuid::Uuid,
) -> Result<ChannelRecord, ServerFnError> {
    let channel = service
        .get_channel(channel_id)
        .await
        .map_err(ServerFnError::new)?;
    let mapped = map_channel_record(channel);
    if mapped.tenant_id != tenant_id.to_string() {
        return Err(ServerFnError::new("Channel not found"));
    }
    Ok(mapped)
}

#[cfg(feature = "ssr")]
async fn ensure_policy_set_belongs_to_tenant(
    service: &rustok_channel::ChannelService,
    tenant_id: uuid::Uuid,
    policy_set_id: uuid::Uuid,
) -> Result<ChannelResolutionPolicySetRecord, ServerFnError> {
    let policy_set = service
        .get_resolution_policy_set(policy_set_id)
        .await
        .map_err(ServerFnError::new)?;
    let mapped = map_policy_set_record(policy_set);
    if mapped.tenant_id != tenant_id.to_string() {
        return Err(ServerFnError::new("Policy set not found"));
    }
    Ok(mapped)
}

#[cfg(feature = "ssr")]
fn map_current_channel(value: rustok_api::ChannelContext) -> ResolvedChannelContext {
    ResolvedChannelContext {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        slug: value.slug,
        name: value.name,
        is_active: value.is_active,
        status: value.status,
        target_type: value.target_type,
        target_value: value.target_value,
        settings: value.settings,
        resolution_source: value.resolution_source,
        resolution_trace: value.resolution_trace,
    }
}

#[cfg(feature = "ssr")]
fn map_channel_detail(value: rustok_channel::ChannelDetailResponse) -> ChannelDetail {
    ChannelDetail {
        channel: map_channel_record(value.channel),
        targets: value.targets.into_iter().map(map_target_record).collect(),
        module_bindings: value
            .module_bindings
            .into_iter()
            .map(map_module_binding_record)
            .collect(),
        oauth_apps: value
            .oauth_apps
            .into_iter()
            .map(map_oauth_app_record)
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_policy_set_detail(
    value: rustok_channel::ChannelResolutionPolicySetDetailResponse,
) -> ChannelResolutionPolicySetDetail {
    ChannelResolutionPolicySetDetail {
        policy_set: map_policy_set_record(value.policy_set),
        rules: value
            .rules
            .into_iter()
            .map(map_policy_rule_record)
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_channel_record(value: rustok_channel::ChannelResponse) -> ChannelRecord {
    ChannelRecord {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        slug: value.slug,
        name: value.name,
        is_active: value.is_active,
        is_default: value.is_default,
        status: value.status,
        settings: value.settings,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_policy_set_record(
    value: rustok_channel::ChannelResolutionPolicySetResponse,
) -> ChannelResolutionPolicySetRecord {
    ChannelResolutionPolicySetRecord {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        slug: value.slug,
        name: value.name,
        schema_version: value.schema_version,
        is_active: value.is_active,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_target_record(value: rustok_channel::ChannelTargetResponse) -> ChannelTargetRecord {
    ChannelTargetRecord {
        id: value.id.to_string(),
        channel_id: value.channel_id.to_string(),
        target_type: value.target_type,
        value: value.value,
        is_primary: value.is_primary,
        settings: value.settings,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_policy_rule_record(
    value: rustok_channel::ChannelResolutionRuleResponse,
) -> ChannelResolutionRuleRecord {
    ChannelResolutionRuleRecord {
        id: value.id.to_string(),
        policy_set_id: value.policy_set_id.to_string(),
        priority: value.priority,
        is_active: value.is_active,
        action_channel_id: value.action_channel_id.to_string(),
        definition: map_policy_rule_definition(value.definition),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_policy_rule_definition(
    value: rustok_channel::ChannelResolutionRuleDefinition,
) -> ChannelResolutionRuleDefinitionRecord {
    ChannelResolutionRuleDefinitionRecord {
        predicates: value
            .predicates
            .into_iter()
            .map(map_policy_predicate)
            .collect(),
        action: map_policy_action(value.action),
    }
}

#[cfg(feature = "ssr")]
fn map_policy_predicate(
    value: rustok_channel::ResolutionPredicate,
) -> ChannelResolutionPredicateRecord {
    match value {
        rustok_channel::ResolutionPredicate::HostEquals(value) => {
            ChannelResolutionPredicateRecord::HostEquals(value)
        }
        rustok_channel::ResolutionPredicate::HostSuffix(value) => {
            ChannelResolutionPredicateRecord::HostSuffix(value)
        }
        rustok_channel::ResolutionPredicate::OAuthAppEquals(value) => {
            ChannelResolutionPredicateRecord::OAuthAppEquals(value.to_string())
        }
        rustok_channel::ResolutionPredicate::SurfaceIs(rustok_channel::TargetSurface::Http) => {
            ChannelResolutionPredicateRecord::SurfaceIs("http".to_string())
        }
        rustok_channel::ResolutionPredicate::LocaleEquals(value) => {
            ChannelResolutionPredicateRecord::LocaleEquals(value)
        }
    }
}

#[cfg(feature = "ssr")]
fn map_policy_action(value: rustok_channel::ResolutionAction) -> ChannelResolutionActionRecord {
    match value {
        rustok_channel::ResolutionAction::ResolveToChannel { channel_id } => {
            ChannelResolutionActionRecord::ResolveToChannel {
                channel_id: channel_id.to_string(),
            }
        }
    }
}

#[cfg(feature = "ssr")]
fn map_module_binding_record(
    value: rustok_channel::ChannelModuleBindingResponse,
) -> ChannelModuleBindingRecord {
    ChannelModuleBindingRecord {
        id: value.id.to_string(),
        channel_id: value.channel_id.to_string(),
        module_slug: value.module_slug,
        is_enabled: value.is_enabled,
        settings: value.settings,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_oauth_app_record(value: rustok_channel::ChannelOauthAppResponse) -> ChannelOauthAppRecord {
    ChannelOauthAppRecord {
        id: value.id.to_string(),
        channel_id: value.channel_id.to_string(),
        oauth_app_id: value.oauth_app_id.to_string(),
        role: value.role,
        created_at: value.created_at.to_rfc3339(),
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/bootstrap")]
pub(super) async fn channel_bootstrap_native() -> Result<ChannelAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, OptionalChannel, TenantContext};
        use rustok_channel::ChannelService;
        use rustok_core::ModuleRegistry;
        use sea_orm::{ConnectionTrait, DbBackend, QueryResult, Statement};

        let app_ctx = expect_context::<AppContext>();
        let registry = expect_context::<ModuleRegistry>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let current_channel = leptos_axum::extract::<OptionalChannel>()
            .await
            .ok()
            .and_then(|value| value.0);

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channels = service
            .list_channel_details(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_channel_detail)
            .collect();
        let policy_sets = service
            .list_resolution_policy_sets(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_policy_set_detail)
            .collect();

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

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id, name, slug, app_type, is_active
            FROM oauth_apps
            WHERE tenant_id = $1
              AND is_active = TRUE
              AND revoked_at IS NULL
            ORDER BY slug ASC
            "#,
            vec![tenant.id.into()],
        );
        let oauth_rows = app_ctx
            .db
            .query_all(stmt)
            .await
            .map_err(ServerFnError::new)?;
        let oauth_apps = oauth_rows
            .into_iter()
            .map(
                |row: QueryResult| -> Result<AvailableOauthAppItem, ServerFnError> {
                    Ok(AvailableOauthAppItem {
                        id: row
                            .try_get::<uuid::Uuid>("", "id")
                            .map_err(ServerFnError::new)?
                            .to_string(),
                        name: row
                            .try_get::<String>("", "name")
                            .map_err(ServerFnError::new)?,
                        slug: row
                            .try_get::<String>("", "slug")
                            .map_err(ServerFnError::new)?,
                        app_type: row
                            .try_get::<String>("", "app_type")
                            .map_err(ServerFnError::new)?,
                        is_active: row
                            .try_get::<bool>("", "is_active")
                            .map_err(ServerFnError::new)?,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChannelAdminBootstrap {
            current_channel: current_channel.map(map_current_channel),
            channels,
            policy_sets,
            available_modules,
            oauth_apps,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "channel/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/create-channel")]
pub(super) async fn channel_create_native(
    payload: CreateChannelPayload,
) -> Result<ChannelRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, CreateChannelInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel = service
            .create_channel(CreateChannelInput {
                tenant_id: tenant.id,
                slug: payload.slug,
                name: payload.name,
                settings: payload.settings,
            })
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_channel_record(channel))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = payload;
        Err(ServerFnError::new(
            "channel/create-channel requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/set-default")]
pub(super) async fn channel_set_default_native(
    channel_id: String,
) -> Result<ChannelRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let updated = service
            .set_default_channel(channel_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_channel_record(updated))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = channel_id;
        Err(ServerFnError::new(
            "channel/set-default requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/create-target")]
pub(super) async fn channel_create_target_native(
    channel_id: String,
    payload: CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, CreateChannelTargetInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let target = service
            .add_target(
                channel_uuid,
                CreateChannelTargetInput {
                    target_type: payload.target_type,
                    value: payload.value,
                    is_primary: payload.is_primary,
                    settings: payload.settings,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_target_record(target))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, payload);
        Err(ServerFnError::new(
            "channel/create-target requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/update-target")]
pub(super) async fn channel_update_target_native(
    channel_id: String,
    target_id: String,
    payload: CreateChannelTargetPayload,
) -> Result<ChannelTargetRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, UpdateChannelTargetInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        let target_uuid = parse_uuid(&target_id, "target_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let target = service
            .update_target(
                channel_uuid,
                target_uuid,
                UpdateChannelTargetInput {
                    target_type: payload.target_type,
                    value: payload.value,
                    is_primary: payload.is_primary,
                    settings: payload.settings,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_target_record(target))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, target_id, payload);
        Err(ServerFnError::new(
            "channel/update-target requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/bind-module")]
pub(super) async fn channel_bind_module_native(
    channel_id: String,
    payload: BindChannelModulePayload,
) -> Result<ChannelModuleBindingRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{BindChannelModuleInput, ChannelService};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let binding = service
            .bind_module(
                channel_uuid,
                BindChannelModuleInput {
                    module_slug: payload.module_slug,
                    is_enabled: payload.is_enabled,
                    settings: payload.settings,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_module_binding_record(binding))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, payload);
        Err(ServerFnError::new(
            "channel/bind-module requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/bind-oauth-app")]
pub(super) async fn channel_bind_oauth_app_native(
    channel_id: String,
    payload: BindChannelOauthAppPayload,
) -> Result<ChannelOauthAppRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{BindChannelOauthAppInput, ChannelService};
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        let oauth_app_uuid = parse_uuid(&payload.oauth_app_id, "oauth_app_id")?;
        let service = ChannelService::new(app_ctx.db.clone());
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT id
            FROM oauth_apps
            WHERE tenant_id = $1
              AND id = $2
              AND is_active = TRUE
              AND revoked_at IS NULL
            LIMIT 1
            "#,
            vec![tenant.id.into(), oauth_app_uuid.into()],
        );
        let exists = app_ctx
            .db
            .query_one(stmt)
            .await
            .map_err(ServerFnError::new)?
            .is_some();
        if !exists {
            return Err(ServerFnError::new(
                "OAuth app does not belong to the current tenant",
            ));
        }

        let binding = service
            .bind_oauth_app(
                channel_uuid,
                BindChannelOauthAppInput {
                    oauth_app_id: oauth_app_uuid,
                    role: payload.role,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_oauth_app_record(binding))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, payload);
        Err(ServerFnError::new(
            "channel/bind-oauth-app requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/delete-target")]
pub(super) async fn channel_delete_target_native(
    channel_id: String,
    target_id: String,
) -> Result<ChannelTargetRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        let target_uuid = parse_uuid(&target_id, "target_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let target = service
            .delete_target(channel_uuid, target_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_target_record(target))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, target_id);
        Err(ServerFnError::new(
            "channel/delete-target requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/delete-module-binding")]
pub(super) async fn channel_delete_module_binding_native(
    channel_id: String,
    binding_id: String,
) -> Result<ChannelModuleBindingRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        let binding_uuid = parse_uuid(&binding_id, "binding_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let binding = service
            .remove_module_binding(channel_uuid, binding_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_module_binding_record(binding))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, binding_id);
        Err(ServerFnError::new(
            "channel/delete-module-binding requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/delete-oauth-app-binding")]
pub(super) async fn channel_delete_oauth_app_binding_native(
    channel_id: String,
    binding_id: String,
) -> Result<ChannelOauthAppRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let channel_uuid = parse_uuid(&channel_id, "channel_id")?;
        let binding_uuid = parse_uuid(&binding_id, "binding_id")?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, channel_uuid).await?;
        let binding = service
            .revoke_oauth_app_binding(channel_uuid, binding_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_oauth_app_record(binding))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, binding_id);
        Err(ServerFnError::new(
            "channel/delete-oauth-app-binding requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/create-resolution-policy-set")]
pub(super) async fn channel_create_resolution_policy_set_native(
    payload: CreateResolutionPolicySetPayload,
) -> Result<ChannelResolutionPolicySetRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, CreateChannelResolutionPolicySetInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let policy_set = service
            .create_resolution_policy_set(CreateChannelResolutionPolicySetInput {
                tenant_id: tenant.id,
                slug: payload.slug,
                name: payload.name,
                is_active: payload.is_active,
            })
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_policy_set_record(policy_set))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = payload;
        Err(ServerFnError::new(
            "channel/create-resolution-policy-set requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "channel/activate-resolution-policy-set"
)]
pub(super) async fn channel_activate_resolution_policy_set_native(
    policy_set_id: String,
) -> Result<ChannelResolutionPolicySetRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let policy_set_uuid = parse_uuid(&policy_set_id, "policy_set_id")?;
        ensure_policy_set_belongs_to_tenant(&service, tenant.id, policy_set_uuid).await?;
        let policy_set = service
            .set_active_resolution_policy_set(policy_set_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_policy_set_record(policy_set))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = policy_set_id;
        Err(ServerFnError::new(
            "channel/activate-resolution-policy-set requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/create-resolution-rule")]
pub(super) async fn channel_create_resolution_rule_native(
    policy_set_id: String,
    payload: CreateResolutionRulePayload,
) -> Result<ChannelResolutionRuleRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, CreateChannelResolutionRuleInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let policy_set_uuid = parse_uuid(&policy_set_id, "policy_set_id")?;
        let action_channel_id = parse_uuid(&payload.action_channel_id, "action_channel_id")?;
        let service = ChannelService::new(app_ctx.db.clone());
        ensure_policy_set_belongs_to_tenant(&service, tenant.id, policy_set_uuid).await?;
        ensure_channel_belongs_to_tenant(&service, tenant.id, action_channel_id).await?;

        let (priority, is_active, definition) =
            build_native_rule_definition_payload(payload, action_channel_id)?;

        let rule = service
            .create_resolution_rule(
                policy_set_uuid,
                CreateChannelResolutionRuleInput {
                    priority,
                    is_active,
                    definition,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_policy_rule_record(rule))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (policy_set_id, payload);
        Err(ServerFnError::new(
            "channel/create-resolution-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/update-resolution-rule")]
pub(super) async fn channel_update_resolution_rule_native(
    policy_set_id: String,
    rule_id: String,
    payload: UpdateResolutionRulePayload,
) -> Result<ChannelResolutionRuleRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, UpdateChannelResolutionRuleInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let policy_set_uuid = parse_uuid(&policy_set_id, "policy_set_id")?;
        let rule_uuid = parse_uuid(&rule_id, "rule_id")?;
        ensure_policy_set_belongs_to_tenant(&service, tenant.id, policy_set_uuid).await?;

        let action_channel_id = payload
            .action_channel_id
            .as_deref()
            .map(|value| parse_uuid(value, "action_channel_id"))
            .transpose()?;
        if let Some(action_channel_id) = action_channel_id {
            ensure_channel_belongs_to_tenant(&service, tenant.id, action_channel_id).await?;
        }

        let rule = service
            .update_resolution_rule(
                policy_set_uuid,
                rule_uuid,
                UpdateChannelResolutionRuleInput {
                    priority: payload.priority,
                    is_active: payload.is_active,
                    action_channel_id,
                    host_equals: payload.host_equals,
                    host_suffix: payload.host_suffix,
                    oauth_app_id: payload.oauth_app_id,
                    surface: payload.surface,
                    locale: payload.locale,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_policy_rule_record(rule))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (policy_set_id, rule_id, payload);
        Err(ServerFnError::new(
            "channel/update-resolution-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/reorder-resolution-rules")]
pub(super) async fn channel_reorder_resolution_rules_native(
    policy_set_id: String,
    payload: ReorderResolutionRulesPayload,
) -> Result<Vec<ChannelResolutionRuleRecord>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::{ChannelService, ReorderChannelResolutionRulesInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let policy_set_uuid = parse_uuid(&policy_set_id, "policy_set_id")?;
        ensure_policy_set_belongs_to_tenant(&service, tenant.id, policy_set_uuid).await?;
        let rule_ids = payload
            .rule_ids
            .iter()
            .map(|rule_id| parse_uuid(rule_id, "rule_id"))
            .collect::<Result<Vec<_>, _>>()?;

        let rules = service
            .reorder_resolution_rules(
                policy_set_uuid,
                ReorderChannelResolutionRulesInput { rule_ids },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(rules.into_iter().map(map_policy_rule_record).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (policy_set_id, payload);
        Err(ServerFnError::new(
            "channel/reorder-resolution-rules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "channel/delete-resolution-rule")]
pub(super) async fn channel_delete_resolution_rule_native(
    policy_set_id: String,
    rule_id: String,
) -> Result<ChannelResolutionRuleRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_manage_permission(&auth.permissions)?;

        let service = ChannelService::new(app_ctx.db.clone());
        let policy_set_uuid = parse_uuid(&policy_set_id, "policy_set_id")?;
        let rule_uuid = parse_uuid(&rule_id, "rule_id")?;
        ensure_policy_set_belongs_to_tenant(&service, tenant.id, policy_set_uuid).await?;
        let rule = service
            .remove_resolution_rule(policy_set_uuid, rule_uuid)
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_policy_rule_record(rule))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (policy_set_id, rule_id);
        Err(ServerFnError::new(
            "channel/delete-resolution-rule requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn build_native_rule_definition_payload(
    payload: CreateResolutionRulePayload,
    action_channel_id: uuid::Uuid,
) -> Result<(i32, bool, rustok_channel::ChannelResolutionRuleDefinition), ServerFnError> {
    use rustok_channel::{ResolutionAction, ResolutionPredicate, TargetSurface};

    let mut predicates = Vec::new();

    if let Some(host_equals) = payload
        .host_equals
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        predicates.push(ResolutionPredicate::HostEquals(host_equals.to_string()));
    }
    if let Some(host_suffix) = payload
        .host_suffix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        predicates.push(ResolutionPredicate::HostSuffix(host_suffix.to_string()));
    }
    if let Some(oauth_app_id) = payload
        .oauth_app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        predicates.push(ResolutionPredicate::OAuthAppEquals(parse_uuid(
            oauth_app_id,
            "oauth_app_id",
        )?));
    }
    if let Some(surface) = payload
        .surface
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let surface = match surface {
            "http" => TargetSurface::Http,
            other => {
                return Err(ServerFnError::new(format!(
                    "Unsupported surface `{other}`; only `http` is currently supported",
                )))
            }
        };
        predicates.push(ResolutionPredicate::SurfaceIs(surface));
    }
    if let Some(locale) = payload
        .locale
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        predicates.push(ResolutionPredicate::LocaleEquals(locale.to_string()));
    }

    Ok((
        payload.priority,
        payload.is_active,
        rustok_channel::ChannelResolutionRuleDefinition {
            predicates,
            action: ResolutionAction::ResolveToChannel {
                channel_id: action_channel_id,
            },
        },
    ))
}
