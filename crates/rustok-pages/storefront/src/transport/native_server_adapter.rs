use leptos::prelude::*;

use super::ApiError;
use crate::model::StorefrontPagesData;

#[cfg(feature = "ssr")]
use crate::model::{PageBlock, PageBody, PageDetail, PageList, PageListItem, PageTranslation};

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
                    blocks: page
                        .blocks
                        .into_iter()
                        .map(|block| PageBlock {
                            id: block.id.to_string(),
                            block_type: match block.block_type {
                                rustok_pages::dto::BlockType::Hero => "hero",
                                rustok_pages::dto::BlockType::Text => "text",
                                rustok_pages::dto::BlockType::Image => "image",
                                rustok_pages::dto::BlockType::Gallery => "gallery",
                                rustok_pages::dto::BlockType::Cta => "cta",
                                rustok_pages::dto::BlockType::Features => "features",
                                rustok_pages::dto::BlockType::Testimonials => "testimonials",
                                rustok_pages::dto::BlockType::Pricing => "pricing",
                                rustok_pages::dto::BlockType::Faq => "faq",
                                rustok_pages::dto::BlockType::Contact => "contact",
                                rustok_pages::dto::BlockType::ProductGrid => "product_grid",
                                rustok_pages::dto::BlockType::Newsletter => "newsletter",
                                rustok_pages::dto::BlockType::Video => "video",
                                rustok_pages::dto::BlockType::Html => "html",
                                rustok_pages::dto::BlockType::Spacer => "spacer",
                            }
                            .to_string(),
                            position: block.position,
                        })
                        .collect(),
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

        Ok(StorefrontPagesData {
            selected_page,
            pages: PageList {
                items: items.into_iter().map(map_page_list_item).collect(),
                total,
            },
        })
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
    use super::{artifact_url, published_artifact_page_body};

    const PAGE_ID: &str = "00000000-0000-0000-0000-000000000000";

    #[test]
    fn missing_published_artifact_fails_closed() {
        assert!(published_artifact_page_body(PAGE_ID, None, Some("web")).is_none());
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
