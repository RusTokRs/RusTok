#[cfg(feature = "ssr")]
use crate::model::*;

#[cfg(feature = "ssr")]
pub async fn fetch_overview(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<MessengerOverviewPayload, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);
    let (active_conversations_count, registered_devices_count) = service
        .get_overview_counts(tenant.id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(MessengerOverviewPayload {
        active_conversations_count,
        stream_topic: "rustok-messenger-events".to_string(),
        consumer_group: "rustok-messenger-workers".to_string(),
        partitions_count: 16,
        registered_devices_count,
        matrix_server_name: "matrix.rustok.local".to_string(),
        federated_rooms_count: 0,
    })
}

#[cfg(feature = "ssr")]
pub async fn fetch_channels(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<Vec<ChannelListItemPayload>, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::domain::ConversationType;
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1);
    let items = service
        .list_channels_summary(tenant.id, 50, 0)
        .await
        .map_err(|e| e.to_string())?;

    let channels = items
        .into_iter()
        .map(|(conv, title, topic, member_count)| {
            let conv_type = ConversationType::try_from(conv.conversation_type)
                .unwrap_or(ConversationType::Channel)
                .as_str()
                .to_string();
            ChannelListItemPayload {
                id: conv.id,
                conversation_type: conv_type,
                title,
                topic,
                member_count,
                created_at: conv.created_at,
            }
        })
        .collect();

    Ok(channels)
}

#[cfg(feature = "ssr")]
pub async fn fetch_e2ee_devices(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<Vec<E2eeDeviceAuditPayload>, String> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
    use rustok_messenger::services::MessengerService;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|e| e.to_string())?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|e| e.to_string())?;

    if auth.tenant_id != tenant.id {
        return Err("Permission denied: tenant mismatch".to_string());
    }

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let device_keys = runtime_ctx
        .shared_get::<std::sync::Arc<rustok_messenger::e2ee::DeviceKeyRegistry>>()
        .unwrap_or_default();
    let service = MessengerService::new(runtime_ctx.db_clone(), 1).with_device_keys(device_keys);

    let audits = service
        .device_keys()
        .list_all_devices_audit()
        .into_iter()
        .map(|(dev, otk_count)| E2eeDeviceAuditPayload {
            device_id: dev.device_id,
            user_id: dev.user_id,
            has_curve25519: !dev.curve25519_key.is_empty(),
            has_ed25519: !dev.identity_key.is_empty(),
            otk_count,
        })
        .collect();

    Ok(audits)
}
