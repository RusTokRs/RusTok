use super::ApiError;
use crate::model::{StorefrontMenu, StorefrontMenuItem, StorefrontMenuLocation};
use leptos::prelude::*;
use rustok_api::PLATFORM_FALLBACK_LOCALE;

pub async fn fetch_active_menu_server(
    tenant_slug: Option<String>,
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ApiError> {
    active_menu_native(tenant_slug, location, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "navigation/active-menu")]
async fn active_menu_native(
    tenant_slug: Option<String>,
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_channel::ChannelService;
        use rustok_core::SecurityContext;
        use rustok_navigation::{
            MENU_LOCALE_NOT_FOUND_ERROR_CODE, MenuBindingService, MenuLocation, NavigationError,
        };
        use rustok_tenant::TenantService;
        let runtime = expect_context::<HostRuntimeContext>();
        let request = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some(channel_id) = request.as_ref().and_then(|ctx| ctx.channel_id) else {
            return Ok(None);
        };
        if !ChannelService::new(runtime.db_clone())
            .is_module_enabled(channel_id, "navigation")
            .await
            .map_err(ServerFnError::new)?
        {
            return Ok(None);
        }
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();
        let (tenant_id, fallback) = if let Some(tenant) = tenant.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "navigation/active-menu requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(runtime.db_clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            let fallback = request
                .as_ref()
                .map(|ctx| ctx.locale.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
            (tenant.id, fallback)
        };
        let locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or(fallback);
        let location = match location {
            StorefrontMenuLocation::Header => MenuLocation::Header,
            StorefrontMenuLocation::Footer => MenuLocation::Footer,
            StorefrontMenuLocation::Sidebar => MenuLocation::Sidebar,
            StorefrontMenuLocation::Mobile => MenuLocation::Mobile,
        };
        match MenuBindingService::new(runtime.db_clone())
            .get_active(
                tenant_id,
                SecurityContext::public_read(),
                channel_id,
                location,
                &locale,
            )
            .await
        {
            Ok(menu) => Ok(menu.map(map_menu)),
            Err(NavigationError::MenuNotFound(_)) => Ok(None),
            Err(NavigationError::Rich(rich))
                if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
            {
                Ok(None)
            }
            Err(error) => Err(ServerFnError::new(error)),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, location, locale);
        Err(ServerFnError::new(
            "navigation/active-menu requires the `ssr` feature",
        ))
    }
}
#[cfg(feature = "ssr")]
fn map_menu(menu: rustok_navigation::MenuResponse) -> StorefrontMenu {
    StorefrontMenu {
        id: menu.id.to_string(),
        effective_locale: menu.effective_locale,
        name: menu.name,
        location: match menu.location {
            rustok_navigation::MenuLocation::Header => StorefrontMenuLocation::Header,
            rustok_navigation::MenuLocation::Footer => StorefrontMenuLocation::Footer,
            rustok_navigation::MenuLocation::Sidebar => StorefrontMenuLocation::Sidebar,
            rustok_navigation::MenuLocation::Mobile => StorefrontMenuLocation::Mobile,
        },
        items: menu.items.into_iter().map(map_item).collect(),
    }
}
#[cfg(feature = "ssr")]
fn map_item(item: rustok_navigation::MenuItemResponse) -> StorefrontMenuItem {
    StorefrontMenuItem {
        id: item.id.to_string(),
        title: item.title,
        url: item.url,
        icon: item.icon,
        children: item.children.into_iter().map(map_item).collect(),
    }
}
