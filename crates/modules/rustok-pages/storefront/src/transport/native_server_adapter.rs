use leptos::prelude::*;

use super::ApiError;
use crate::model::StorefrontPagesData;

#[cfg(feature = "ssr")]
use crate::model::{PageDetail, PageList, PageListItem, PageTranslation, PublishedLanding};

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
        let channel_context = match leptos_axum::extract::<rustok_api::OptionalChannel>().await {
            Ok(rustok_api::OptionalChannel(Some(channel))) => Some(channel),
            _ => leptos_axum::extract::<rustok_api::ChannelContext>()
                .await
                .ok(),
        };

        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (
                tenant.id,
                normalize_tenant_fallback_locale(tenant.default_locale.as_str()),
            )
        } else {
            let slug = super::configured_fallback_tenant_slug(tenant_slug.as_deref())?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(&slug)
                .await
                .map_err(ServerFnError::new)?;
            (
                tenant.id,
                normalize_tenant_fallback_locale(tenant.default_locale.as_str()),
            )
        };

        let channel_id = request_context
            .as_ref()
            .and_then(|ctx| ctx.channel_id)
            .or_else(|| channel_context.as_ref().map(|ctx| ctx.id));
        if let Some(channel_id) = channel_id {
            let enabled = ChannelService::new(runtime_ctx.db_clone())
                .is_module_enabled(channel_id, MODULE_SLUG)
                .await
                .map_err(ServerFnError::new)?;
            if !enabled {
                return Err(ServerFnError::new(format!(
                    "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
                    request_context
                        .as_ref()
                        .and_then(|ctx| ctx.channel_slug.as_deref())
                        .or_else(|| channel_context.as_ref().map(|ctx| ctx.slug.as_str()))
                        .unwrap_or("current"),
                )));
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
            .and_then(|ctx| normalize_channel_slug(ctx.channel_slug.as_deref()))
            .or_else(|| {
                channel_context
                    .as_ref()
                    .and_then(|ctx| normalize_channel_slug(Some(&ctx.slug)))
            });

        let cache_runtime = runtime_ctx.shared_get::<PagesCacheReadRuntime>();
        let cache_variant = storefront_cache_variant(
            page_slug.as_str(),
            requested_locale.as_str(),
            fallback_locale.as_str(),
            public_channel_slug.as_deref(),
        );
        let cache_key = if let Some(cache_runtime) = cache_runtime.as_ref() {
            match cache_runtime.generation_snapshot(tenant_id).await {
                Ok(generations) => {
                    match storefront_pages_cache_key(tenant_id, generations, cache_variant.as_str())
                    {
                        Ok(key) => Some(key),
                        Err(error) => {
                            tracing::warn!(%error, %tenant_id, "Pages storefront cache key rejected");
                            None
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, %tenant_id, "Pages storefront generation read failed; bypassing cache");
                    None
                }
            }
        } else {
            None
        };
        if let (Some(cache_runtime), Some(cache_key)) = (cache_runtime.as_ref(), cache_key.as_ref())
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
                // The landing is resolved for the locale the page itself resolved to, by the
                // artifact owner: page status, channel visibility, the locale binding and
                // artifact integrity are decided there. Nothing is derived from the page body,
                // which a public read does not return.
                let landing_locale = page
                    .effective_locale
                    .as_deref()
                    .unwrap_or(requested_locale.as_str());
                let landing = PageBuilderArtifactService::new(runtime_ctx.db_clone())
                    .load_public_bound_artifact_with_fallback(
                        tenant_id,
                        page.id,
                        landing_locale,
                        Some(fallback_locale.as_str()),
                        public_channel_slug.as_deref(),
                    )
                    .await
                    .map_err(ServerFnError::new)?
                    .map(published_landing);

                Some(PageDetail {
                    effective_locale: page.effective_locale,
                    translation: page.translation.map(|translation| PageTranslation {
                        locale: translation.locale,
                        title: translation.title,
                        slug: translation.slug,
                        meta_title: translation.meta_title,
                        meta_description: translation.meta_description,
                    }),
                    landing,
                })
            }
            None => None,
        };

        let (items, total) = service
            .list_public_visible_with_locale_fallback(
                tenant_id,
                RuntimeListPagesFilter {
                    status: Some(ContentStatus::Published),
                    template: None,
                    locale: Some(requested_locale),
                    page: 1,
                    per_page: 6,
                },
                Some(fallback_locale.as_str()),
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
        if let (Some(cache_runtime), Some(cache_key)) = (cache_runtime.as_ref(), cache_key) {
            let put_result = cache_runtime.put_json(cache_key, &data).await;
            if let Err(error) = put_result {
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
fn normalize_tenant_fallback_locale(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        PLATFORM_FALLBACK_LOCALE.to_string()
    } else {
        value.to_string()
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

/// Projects the published landing artifact for the storefront: the stored body markup and
/// stylesheet, as they are. The artifact owner has already verified both against the artifact
/// hash, so nothing is re-derived from the full document here.
#[cfg(feature = "ssr")]
fn published_landing(artifact: rustok_pages::PublishedLandingArtifact) -> PublishedLanding {
    PublishedLanding {
        locale: artifact.locale,
        html: artifact.body_html,
        css: artifact.css,
        artifact_hash: artifact.artifact_hash,
    }
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
    use super::{normalize_tenant_fallback_locale, published_landing, storefront_cache_variant};

    #[test]
    fn published_landing_is_the_stored_body_markup_and_stylesheet() {
        let artifact = rustok_pages::PublishedLandingArtifact {
            page_id: uuid::Uuid::nil(),
            locale: "en".to_string(),
            build_hash: "build".to_string(),
            artifact_hash: "artifact".to_string(),
            materialization_hash: None,
            document_html: "<html><body><h1>Document</h1></body></html>".to_string(),
            body_html: "<h1>Hello</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            content_hash: "hash".to_string(),
        };
        let landing = published_landing(artifact);
        assert_eq!(landing.locale, "en");
        assert_eq!(landing.html, "<h1>Hello</h1>");
        assert_eq!(landing.css, "h1 { color: red; }");
        assert_eq!(landing.artifact_hash, "artifact");
    }

    /// An empty stored body is an empty landing. The body markup is never re-derived from the
    /// full document: that would render text the artifact owner did not store as the body.
    #[test]
    fn empty_body_markup_is_not_replaced_by_the_document() {
        let artifact = rustok_pages::PublishedLandingArtifact {
            page_id: uuid::Uuid::nil(),
            locale: "en".to_string(),
            build_hash: "build".to_string(),
            artifact_hash: "artifact".to_string(),
            materialization_hash: None,
            document_html: "<html><body><h1>Document</h1></body></html>".to_string(),
            body_html: String::new(),
            css: String::new(),
            content_hash: "hash".to_string(),
        };
        assert_eq!(published_landing(artifact).html, "");
    }

    #[test]
    fn tenant_fallback_locale_uses_platform_only_when_owner_value_is_blank() {
        assert_eq!(normalize_tenant_fallback_locale(" ru "), "ru");
        assert_eq!(
            normalize_tenant_fallback_locale("   "),
            super::PLATFORM_FALLBACK_LOCALE
        );
    }

    #[test]
    fn cache_variant_binds_route_locale_fallback_and_channel() {
        let base = storefront_cache_variant("about", "en", "en", Some("web"));
        assert_ne!(
            base,
            storefront_cache_variant("home", "en", "en", Some("web"))
        );
        assert_ne!(
            base,
            storefront_cache_variant("about", "fr", "en", Some("web"))
        );
        assert_ne!(
            base,
            storefront_cache_variant("about", "en", "ru", Some("web"))
        );
        assert_ne!(
            base,
            storefront_cache_variant("about", "en", "en", Some("mobile"))
        );
    }
}
