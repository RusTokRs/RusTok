use leptos::prelude::*;

use crate::model::{
    MediaListItem, MediaListPayload, MediaTranslationPayload, MediaUsageSnapshot,
    UpsertTranslationPayload,
};

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value).map_err(|err| ServerFnError::new(err.to_string()))
}

#[cfg(feature = "ssr")]
fn require_permission(
    auth: &rustok_api::AuthContext,
    permission: rustok_api::Permission,
) -> Result<(), ServerFnError> {
    if rustok_api::has_effective_permission(&auth.permissions, &permission) {
        Ok(())
    } else {
        Err(ServerFnError::new(format!("{permission} required")))
    }
}

#[cfg(feature = "ssr")]
fn media_service() -> Result<rustok_media::MediaService, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::HostRuntimeContext;

    let runtime = expect_context::<HostRuntimeContext>();
    let storage = runtime
        .shared_get::<rustok_storage::StorageRuntime>()
        .ok_or_else(|| ServerFnError::new("StorageRuntime not available"))?;
    Ok(rustok_media::MediaService::new(runtime.db_clone(), storage))
}

#[server(prefix = "/api/fn", endpoint = "media/library")]
pub(super) async fn media_library_native(
    page: i32,
    per_page: i32,
) -> Result<MediaListPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::List))?;
        let service = media_service()?;
        let limit = per_page.clamp(1, 100) as u64;
        let offset = (page.max(1) - 1) as u64 * limit;
        let (items, total) = service
            .list(tenant.id, limit, offset)
            .await
            .map_err(ServerFnError::new)?;

        Ok(MediaListPayload {
            items: items
                .into_iter()
                .map(|item| MediaListItem {
                    id: item.id.to_string(),
                    tenant_id: item.tenant_id.to_string(),
                    uploaded_by: item.uploaded_by.map(|value| value.to_string()),
                    filename: item.filename,
                    original_name: item.original_name,
                    mime_type: item.mime_type,
                    size: item.size,
                    storage_driver: item.storage_driver,
                    public_url: item.public_url,
                    width: item.width,
                    height: item.height,
                    created_at: item.created_at.to_rfc3339(),
                })
                .collect(),
            total,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (page, per_page);
        Err(ServerFnError::new(
            "media/library requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/detail")]
pub(super) async fn media_detail_native(
    media_id: String,
) -> Result<Option<MediaListItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Read))?;
        let service = media_service()?;
        match service.get(tenant.id, parse_uuid(&media_id)?).await {
            Ok(item) => Ok(Some(MediaListItem {
                id: item.id.to_string(),
                tenant_id: item.tenant_id.to_string(),
                uploaded_by: item.uploaded_by.map(|value| value.to_string()),
                filename: item.filename,
                original_name: item.original_name,
                mime_type: item.mime_type,
                size: item.size,
                storage_driver: item.storage_driver,
                public_url: item.public_url,
                width: item.width,
                height: item.height,
                created_at: item.created_at.to_rfc3339(),
            })),
            Err(rustok_media::MediaError::NotFound(_)) => Ok(None),
            Err(err) => Err(ServerFnError::new(err.to_string())),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/translations")]
pub(super) async fn media_translations_native(
    media_id: String,
) -> Result<Vec<MediaTranslationPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Read))?;
        let service = media_service()?;
        let items = service
            .get_translations(tenant.id, parse_uuid(&media_id)?)
            .await
            .map_err(ServerFnError::new)?;
        Ok(items
            .into_iter()
            .map(|item| MediaTranslationPayload {
                id: item.id.to_string(),
                media_id: item.media_id.to_string(),
                locale: item.locale,
                title: item.title,
                alt_text: item.alt_text,
                caption: item.caption,
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/translations requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/upsert-translation")]
pub(super) async fn media_upsert_translation_native(
    media_id: String,
    payload: UpsertTranslationPayload,
) -> Result<MediaTranslationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Update))?;
        let service = media_service()?;
        let item = service
            .upsert_translation(
                tenant.id,
                parse_uuid(&media_id)?,
                rustok_media::UpsertTranslationInput {
                    locale: payload.locale,
                    title: payload.title,
                    alt_text: payload.alt_text,
                    caption: payload.caption,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(MediaTranslationPayload {
            id: item.id.to_string(),
            media_id: item.media_id.to_string(),
            locale: item.locale,
            title: item.title,
            alt_text: item.alt_text,
            caption: item.caption,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (media_id, payload);
        Err(ServerFnError::new(
            "media/upsert-translation requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/delete")]
pub(super) async fn media_delete_native(media_id: String) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::Delete))?;
        let service = media_service()?;
        service
            .delete(tenant.id, parse_uuid(&media_id)?)
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = media_id;
        Err(ServerFnError::new(
            "media/delete requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "media/usage")]
pub(super) async fn media_usage_native() -> Result<MediaUsageSnapshot, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{Action, Permission, Resource};

        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_permission(&auth, Permission::new(Resource::Media, Action::List))?;

        let service = media_service()?;
        let snapshot = service
            .usage_snapshot(tenant.id)
            .await
            .map_err(ServerFnError::new)?;

        Ok(MediaUsageSnapshot {
            tenant_id: tenant.id.to_string(),
            file_count: snapshot.file_count,
            total_bytes: snapshot.total_bytes,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("media/usage requires the `ssr` feature"))
    }
}
