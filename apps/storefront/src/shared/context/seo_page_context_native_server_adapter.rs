use leptos::prelude::*;

use super::seo_page_context::{
    ResolvedSeoAlternateLink, ResolvedSeoDocument, ResolvedSeoImageAsset, ResolvedSeoLinkTag,
    ResolvedSeoMetaTag, ResolvedSeoOpenGraph, ResolvedSeoPageContext, ResolvedSeoPagination,
    ResolvedSeoRedirectDecision, ResolvedSeoRobots, ResolvedSeoRouteContext,
    ResolvedSeoStructuredDataBlock, ResolvedSeoTwitterCard, ResolvedSeoVerification,
    ResolvedSeoVerificationTag,
};

#[server(prefix = "/api/fn", endpoint = "storefront/seo-page-context")]
pub(crate) async fn resolve_seo_page_context(
    tenant_slug: String,
    locale: String,
    route: String,
) -> Result<Option<ResolvedSeoPageContext>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRuntimeExtensions;
        use rustok_tenant::TenantService;

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = TenantService::new(runtime.db_clone())
            .get_tenant_by_slug(tenant_slug.as_str())
            .await
            .map_err(ServerFnError::new)?;

        let event_bus = runtime
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "SEO transactional event bus is not initialized; host bootstrap must provide TransactionalEventBus",
                )
            })?;
        let extensions = runtime
            .shared_get::<std::sync::Arc<ModuleRuntimeExtensions>>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "SEO runtime extensions are not initialized; host bootstrap must provide ModuleRuntimeExtensions",
                )
            })?;
        let service = rustok_seo::SeoApplicationServices::from_runtime_extensions(
            runtime.db_clone(),
            event_bus,
            &extensions,
        )
        .map_err(|err| ServerFnError::new(err.to_string()))?;
        let default_locale = tenant
            .settings
            .get("default_locale")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| rustok_api::PLATFORM_FALLBACK_LOCALE.to_string());
        let resolved = service
            .routing().resolve_page_context_for_channel(
                &rustok_api::TenantContext {
                    id: tenant.id,
                    name: tenant.name,
                    slug: tenant.slug,
                    domain: tenant.domain,
                    settings: tenant.settings,
                    default_locale,
                    is_active: tenant.is_active,
                },
                locale.as_str(),
                route.as_str(),
                request_context
                    .as_ref()
                    .and_then(|context| context.channel_slug.as_deref()),
            )
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))?;

        Ok(resolved.map(|value| ResolvedSeoPageContext {
            route: ResolvedSeoRouteContext {
                target_kind: value
                    .route
                    .target_kind
                    .map(|item| item.as_str().to_string()),
                target_id: value.route.target_id.map(|item| item.to_string()),
                requested_locale: value.route.requested_locale,
                effective_locale: value.route.effective_locale,
                canonical_url: value.route.canonical_url,
                redirect: value
                    .route
                    .redirect
                    .map(|item| ResolvedSeoRedirectDecision {
                        target_url: item.target_url,
                        status_code: item.status_code,
                    }),
                alternates: value
                    .route
                    .alternates
                    .into_iter()
                    .map(|item| ResolvedSeoAlternateLink {
                        locale: item.locale,
                        href: item.href,
                        x_default: item.x_default,
                    })
                    .collect(),
            },
            document: ResolvedSeoDocument {
                title: value.document.title,
                description: value.document.description,
                robots: ResolvedSeoRobots {
                    index: value.document.robots.index,
                    follow: value.document.robots.follow,
                    noarchive: value.document.robots.noarchive,
                    nosnippet: value.document.robots.nosnippet,
                    noimageindex: value.document.robots.noimageindex,
                    notranslate: value.document.robots.notranslate,
                    max_snippet: value.document.robots.max_snippet,
                    max_image_preview: value.document.robots.max_image_preview,
                    max_video_preview: value.document.robots.max_video_preview,
                    custom: value.document.robots.custom,
                },
                open_graph: value.document.open_graph.map(map_open_graph),
                twitter: value.document.twitter.map(map_twitter_card),
                verification: value
                    .document
                    .verification
                    .map(|item| ResolvedSeoVerification {
                        google: item.google,
                        yandex: item.yandex,
                        yahoo: item.yahoo,
                        other: item
                            .other
                            .into_iter()
                            .map(|tag| ResolvedSeoVerificationTag {
                                name: tag.name,
                                value: tag.value,
                            })
                            .collect(),
                    }),
                pagination: value.document.pagination.map(|item| ResolvedSeoPagination {
                    prev_url: item.prev_url,
                    next_url: item.next_url,
                }),
                structured_data_blocks: value
                    .document
                    .structured_data_blocks
                    .into_iter()
                    .map(|item| ResolvedSeoStructuredDataBlock {
                        id: item.id,
                        schema_kind: item.schema_kind.as_str().to_string(),
                        schema_type: item.schema_type,
                        kind: item.kind,
                        source: item.source.as_str().to_string(),
                        payload: item.payload.0,
                    })
                    .collect(),
                meta_tags: value
                    .document
                    .meta_tags
                    .into_iter()
                    .map(|item| ResolvedSeoMetaTag {
                        name: item.name,
                        property: item.property,
                        http_equiv: item.http_equiv,
                        content: item.content,
                    })
                    .collect(),
                link_tags: value
                    .document
                    .link_tags
                    .into_iter()
                    .map(|item| ResolvedSeoLinkTag {
                        rel: item.rel,
                        href: item.href,
                        hreflang: item.hreflang,
                        media: item.media,
                        mime_type: item.mime_type,
                        title: item.title,
                    })
                    .collect(),
            },
        }))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, locale, route);
        Err(ServerFnError::new(
            "storefront/seo-page-context requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_open_graph(value: rustok_seo::SeoOpenGraph) -> ResolvedSeoOpenGraph {
    ResolvedSeoOpenGraph {
        title: value.title,
        description: value.description,
        kind: value.kind,
        site_name: value.site_name,
        url: value.url,
        locale: value.locale,
        images: value
            .images
            .into_iter()
            .map(|item| ResolvedSeoImageAsset {
                url: item.url,
                alt: item.alt,
                width: item.width,
                height: item.height,
                mime_type: item.mime_type,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_twitter_card(value: rustok_seo::SeoTwitterCard) -> ResolvedSeoTwitterCard {
    ResolvedSeoTwitterCard {
        card: value.card,
        title: value.title,
        description: value.description,
        site: value.site,
        creator: value.creator,
        images: value
            .images
            .into_iter()
            .map(|item| ResolvedSeoImageAsset {
                url: item.url,
                alt: item.alt,
                width: item.width,
                height: item.height,
                mime_type: item.mime_type,
            })
            .collect(),
    }
}
