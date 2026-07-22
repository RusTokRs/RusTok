use leptos::prelude::*;

use super::ApiError;
use crate::model::{StorefrontMenu, StorefrontMenuLocation, StorefrontPagesData};

#[cfg(feature = "ssr")]
use crate::model::{
    PageBody, PageDetail, PageList, PageListItem, PageTranslation, StorefrontMenuItem,
};

#[cfg(feature = "ssr")]
const MODULE_SLUG: &str = "pages";
#[cfg(feature = "ssr")]
use rustok_api::PLATFORM_FALLBACK_LOCALE;

pub async fn fetch_storefront_pages_server(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    storefront_pages_native(tenant_slug, page_slug, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_active_menu_server(
    tenant_slug: Option<String>,
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, ApiError> {
    active_menu_native(tenant_slug, location, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "pages/active-menu")]
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
        use rustok_outbox::TransactionalEventBus;
        use rustok_pages::{
            MENU_LOCALE_NOT_FOUND_ERROR_CODE, MenuBindingService, MenuLocation, PagesError,
        };
        use rustok_tenant::TenantService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let Some(channel_id) = request_context.as_ref().and_then(|ctx| ctx.channel_id) else {
            return Ok(None);
        };
        let enabled = ChannelService::new(runtime_ctx.db_clone())
            .is_module_enabled(channel_id, MODULE_SLUG)
            .await
            .map_err(ServerFnError::new)?;
        if !enabled {
            return Ok(None);
        }

        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();
        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "pages/active-menu requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            let fallback = request_context
                .as_ref()
                .map(|ctx| ctx.locale.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
            (tenant.id, fallback)
        };
        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or(fallback_locale);
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "pages/active-menu requires TransactionalEventBus in host runtime context",
                )
            })?;
        let location = match location {
            StorefrontMenuLocation::Header => MenuLocation::Header,
            StorefrontMenuLocation::Footer => MenuLocation::Footer,
            StorefrontMenuLocation::Sidebar => MenuLocation::Sidebar,
            StorefrontMenuLocation::Mobile => MenuLocation::Mobile,
        };

        match MenuBindingService::new(runtime_ctx.db_clone(), event_bus)
            .get_active(
                tenant_id,
                SecurityContext::public_read(),
                channel_id,
                location,
                requested_locale.as_str(),
            )
            .await
        {
            Ok(menu) => Ok(menu.map(map_storefront_menu)),
            Err(PagesError::MenuNotFound(_)) => Ok(None),
            Err(PagesError::Rich(rich))
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
            "pages/active-menu requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pages/storefront-data")]
async fn storefront_pages_native(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_channel::ChannelService;
        use rustok_content::entities::node::ContentStatus;
        use rustok_core::SecurityContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_pages::{
            ListPagesFilter as RuntimeListPagesFilter, PageBuilderArtifactService, PageService,
            PagesCacheReadRuntime, storefront_pages_cache_key,
        };
        use rustok_tenant::TenantService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "pages/storefront-data requires TransactionalEventBus in host runtime context",
                )
            })?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();

        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (tenant.id, tenant.default_locale.clone())
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "pages/storefront-data requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            let fallback = request_context
                .as_ref()
                .map(|ctx| ctx.locale.clone())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
            (tenant.id, fallback)
        };

        if let Some(request_context) = request_context.as_ref() {
            if let Some(channel_id) = request_context.channel_id {
                let enabled = ChannelService::new(runtime_ctx.db_clone())
                    .is_module_enabled(channel_id, MODULE_SLUG)
                    .await
                    .map_err(ServerFnError::new)?;
                if !enabled {
                    return Err(ServerFnError::new(format!(
                        "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
                        request_context.channel_slug.as_deref().unwrap_or("current"),
                    )));
                }
            }
        }

        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or_else(|| fallback_locale.clone());
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_channel_slug(ctx.channel_slug.as_deref()));

        let cache_runtime = runtime_ctx.shared_get::<PagesCacheReadRuntime>();
        let cache_variant = storefront_cache_variant(
            page_slug.as_str(),
            requested_locale.as_str(),
            fallback_locale.as_str(),
            public_channel_slug.as_deref(),
        );
        let cache_key = if let Some(cache_runtime) = cache_runtime.as_ref() {
            match cache_runtime.generation_snapshot(tenant_id).await {
                Ok(generations) => match storefront_pages_cache_key(
                    tenant_id,
                    generations,
                    cache_variant.as_str(),
                ) {
                    Ok(key) => Some(key),
                    Err(error) => {
                        tracing::warn!(%error, %tenant_id, "Pages storefront cache key rejected");
                        None
                    }
                },
                Err(error) => {
                    tracing::warn!(%error, %tenant_id, "Pages storefront generation read failed; bypassing cache");
                    None
                }
            }
        } else {
            None
        };
        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key.as_ref())
        {
            match cache_runtime
                .get_json::<StorefrontPagesData>(cache_key)
                .await
            {
                Ok(Some(cached)) => {
                    tracing::debug!(%tenant_id, "Pages storefront cache hit");
                    return Ok(cached);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(%error, %tenant_id, "Pages storefront cache read failed; loading source data");
                }
            }
        }

        let service = PageService::new(runtime_ctx.db_clone(), event_bus);

        let selected_page = service
            .get_by_slug_with_locale_fallback(
                tenant_id,
                SecurityContext::public_read(),
                requested_locale.as_str(),
                page_slug.as_str(),
                Some(fallback_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .filter(|page| {
                is_visible_for_public_channel(&page.channel_slugs, public_channel_slug.as_deref())
            });
        let selected_page = match selected_page {
            Some(page) => {
                let page_id = page.id;
                let body = match page.body {
                    Some(body) if body.format.eq_ignore_ascii_case("grapesjs") => {
                        let published = PageBuilderArtifactService::new(runtime_ctx.db_clone())
                            .load_public_bound_artifact_with_fallback(
                                tenant_id,
                                page_id,
                                &body.locale,
                                Some(fallback_locale.as_str()),
                                public_channel_slug.as_deref(),
                            )
                            .await
                            .map_err(ServerFnError::new)?;
                        published_artifact_page_body(
                            page_id,
                            published,
                            public_channel_slug.as_deref(),
                        )
                    }
                    Some(body) => Some(PageBody {
                        locale: body.locale,
                        content: body.content,
                        format: body.format,
                    }),
                    None => None,
                };

                Some(PageDetail {
                    effective_locale: page.effective_locale,
                    translation: page.translation.map(|translation| PageTranslation {
                        locale: translation.locale,
                        title: translation.title,
                        slug: translation.slug,
                        meta_title: translation.meta_title,
                        meta_description: translation.meta_description,
                    }),
                    body,
                })
            }
            None => None,
        };

        let (items, total) = service
            .list_public_visible(
                tenant_id,
                RuntimeListPagesFilter {
                    status: Some(ContentStatus::Published),
                    template: None,
                    locale: Some(requested_locale),
                    page: 1,
                    per_page: 6,
                },
                public_channel_slug.as_deref(),
            )
            .await
            .map_err(ServerFnError::new)?;

        let data = StorefrontPagesData {
            selected_page,
            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
        };
        if let (Some(cache_runtime), Some(cache_key)) =
            (cache_runtime.as_ref(), cache_key)
        {
            if let Err(error) = cache_runtime.put_json(cache_key, &data).await {
                tracing::warn!(%error, %tenant_id, "Pages storefront cache fill failed");
            }
        }
        Ok(data)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, page_slug, locale);
        Err(ServerFnError::new(
            "pages/storefront-data requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn storefront_cache_variant(
    page_slug: &str,
    requested_locale: &str,
    fallback_locale: &str,
    channel_slug: Option<&str>,
) -> String {
    serde_json::to_string(&(
        page_slug.trim(),
        requested_locale.trim(),
        fallback_locale.trim(),
        channel_slug.unwrap_or_default(),
    ))
    .expect("serializing a tuple of strings cannot fail")
}

#[cfg(feature = "ssr")]
fn map_storefront_menu(menu: rustok_pages::MenuResponse) -> StorefrontMenu {
    StorefrontMenu {
        id: menu.id.to_string(),
        effective_locale: menu.effective_locale,
        name: menu.name,
        location: match menu.location {
            rustok_pages::MenuLocation::Header => StorefrontMenuLocation::Header,
            rustok_pages::MenuLocation::Footer => StorefrontMenuLocation::Footer,
            rustok_pages::MenuLocation::Sidebar => StorefrontMenuLocation::Sidebar,
            rustok_pages::MenuLocation::Mobile => StorefrontMenuLocation::Mobile,
        },
        items: menu.items.into_iter().map(map_storefront_menu_item).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_storefront_menu_item(item: rustok_pages::MenuItemResponse) -> StorefrontMenuItem {
    StorefrontMenuItem {
        id: item.id.to_string(),
        title: item.title,
        url: item.url,
        icon: item.icon,
        children: item
            .children
            .into_iter()
            .map(map_storefront_menu_item)
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn published_artifact_page_body(
    page_id: impl std::fmt::Display,
    published: Option<rustok_pages::PublishedLandingArtifact>,
    channel_slug: Option<&str>,
) -> Option<PageBody> {
    published.map(|artifact| PageBody {
        locale: artifact.locale.clone(),
        content: artifact_url(page_id, &artifact.locale, channel_slug),
        format: "fly_artifact_url".to_string(),
    })
}

#[cfg(feature = "ssr")]
fn artifact_url(
    page_id: impl std::fmt::Display,
    locale: &str,
    channel_slug: Option<&str>,
) -> String {
    let mut query = format!("locale={}", form_urlencode_component(locale));
    if let Some(channel_slug) = channel_slug {
        query.push_str("&channel=");
        query.push_str(&form_urlencode_component(channel_slug));
    }
    format!("/api/pages/{page_id}/artifact?{query}")
}

#[cfg(feature = "ssr")]
fn form_urlencode_component(value: &str) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail"),
        }
    }
    encoded
}

#[cfg(feature = "ssr")]
fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
fn is_visible_for_public_channel(
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }

    let Some(public_channel_slug) = public_channel_slug else {
        return false;
    };

    channel_slugs
        .iter()
        .any(|slug| slug.eq_ignore_ascii_case(public_channel_slug))
}

#[cfg(feature = "ssr")]
fn map_page_list_item(page: rustok_pages::PageListItem) -> PageListItem {
    PageListItem {
        id: page.id.to_string(),
        title: page.title,
        slug: page.slug,
        status: match page.status {
            rustok_content::entities::node::ContentStatus::Draft => "draft",
            rustok_content::entities::node::ContentStatus::Published => "published",
            rustok_content::entities::node::ContentStatus::Archived => "archived",
        }
        .to_string(),
        template: page.template,
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{artifact_url, published_artifact_page_body, storefront_cache_variant};

    const PAGE_ID: &str = "00000000-0000-0000-0000-000000000000";

    #[test]
    fn missing_published_artifact_fails_closed() {
        assert!(published_artifact_page_body(PAGE_ID, None, Some("web")).is_none());
    }

    #[test]
    fn cache_variant_binds_route_locale_fallback_and_channel() {
        let base = storefront_cache_variant("about", "en", "en", Some("web"));
        assert_ne!(base, storefront_cache_variant("home", "en", "en", Some("web")));
        assert_ne!(base, storefront_cache_variant("about", "fr", "en", Some("web")));
        assert_ne!(base, storefront_cache_variant("about", "en", "ru", Some("web")));
        assert_ne!(base, storefront_cache_variant("about", "en", "en", Some("mobile")));
    }

    #[test]
    fn artifact_url_carries_locale_and_resolved_channel() {
        assert_eq!(
            artifact_url(PAGE_ID, "pt-BR", Some("web store")),
            format!("/api/pages/{PAGE_ID}/artifact?locale=pt-BR&channel=web+store")
        );
    }

    #[test]
    fn artifact_url_omits_channel_for_unrestricted_requests() {
        assert_eq!(
            artifact_url(PAGE_ID, "en", None),
            format!("/api/pages/{PAGE_ID}/artifact?locale=en")
        );
    }

    #[test]
    fn artifact_url_percent_encodes_reserved_and_unicode_bytes() {
        assert_eq!(
            artifact_url(PAGE_ID, "pt BR/ç", Some("web/store?x=1")),
            format!(
                "/api/pages/{PAGE_ID}/artifact?locale=pt+BR%2F%C3%A7&channel=web%2Fstore%3Fx%3D1"
            )
        );
    }
}
