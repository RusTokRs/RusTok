use leptos::prelude::*;

use crate::entities::oauth_app::model::{AppType, OAuthApp};

#[cfg(feature = "ssr")]
fn map_app_type(value: rustok_auth_admin::model::AppType) -> AppType {
    match value {
        rustok_auth_admin::model::AppType::Embedded => AppType::Embedded,
        rustok_auth_admin::model::AppType::FirstParty => AppType::FirstParty,
        rustok_auth_admin::model::AppType::Mobile => AppType::Mobile,
        rustok_auth_admin::model::AppType::Service => AppType::Service,
        rustok_auth_admin::model::AppType::ThirdParty => AppType::ThirdParty,
    }
}

#[cfg(feature = "ssr")]
fn map_oauth_app(value: rustok_auth_admin::model::OAuthApp) -> OAuthApp {
    OAuthApp {
        id: value.id,
        name: value.name,
        slug: value.slug,
        description: value.description,
        icon_url: value.icon_url,
        app_type: map_app_type(value.app_type),
        client_id: value.client_id,
        redirect_uris: value.redirect_uris,
        scopes: value.scopes,
        grant_types: value.grant_types,
        manifest_ref: value.manifest_ref,
        auto_created: value.auto_created,
        managed_by_manifest: value.managed_by_manifest,
        is_active: value.is_active,
        can_edit: value.can_edit,
        can_rotate_secret: value.can_rotate_secret,
        can_revoke: value.can_revoke,
        active_token_count: value.active_token_count,
        last_used_at: value.last_used_at,
        created_at: value.created_at,
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-oauth-apps")]
pub(super) async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        rustok_auth_admin::transport::native_server_adapter::list_oauth_apps_native(limit)
            .await
            .map(|apps| apps.into_iter().map(map_oauth_app).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/list-oauth-apps requires the `ssr` feature",
        ))
    }
}
