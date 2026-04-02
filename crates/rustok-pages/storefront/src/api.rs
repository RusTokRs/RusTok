use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    PageBlock, PageBody, PageDetail, PageList, PageListItem, PageTranslation, StorefrontPagesData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_PAGES_QUERY: &str = "query StorefrontPages($pageSlug: String!, $filter: ListGqlPagesFilter, $locale: String) { selectedPage: pageBySlug(slug: $pageSlug, locale: $locale) { effectiveLocale translation { locale title slug metaTitle metaDescription } body { locale content format } blocks { id blockType position } } pages(filter: $filter) { total items { id title slug status template } } }";
const MODULE_SLUG: &str = "pages";
const PLATFORM_FALLBACK_LOCALE: &str = "en";

#[derive(Debug, Deserialize)]
struct StorefrontPagesResponse {
    #[serde(rename = "selectedPage")]
    selected_page: Option<PageDetail>,
    pages: PageList,
}

#[derive(Debug, Serialize)]
struct StorefrontPagesVariables {
    #[serde(rename = "pageSlug")]
    page_slug: String,
    filter: ListPagesFilter,
    locale: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ListPagesFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_pages(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    match fetch_storefront_pages_server(configured_tenant_slug(), page_slug.clone(), locale.clone())
        .await
    {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_pages_graphql(page_slug, locale).await,
    }
}

pub async fn fetch_storefront_pages_server(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    storefront_pages_native(tenant_slug, page_slug, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_pages_graphql(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    let response: StorefrontPagesResponse = request(
        STOREFRONT_PAGES_QUERY,
        StorefrontPagesVariables {
            page_slug,
            filter: ListPagesFilter {
                page: 1,
                per_page: 6,
            },
            locale,
        },
    )
    .await?;

    Ok(StorefrontPagesData {
        selected_page: response.selected_page,
        pages: response.pages,
    })
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
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_channel::ChannelService;
        use rustok_content::entities::node::ContentStatus;
        use rustok_core::SecurityContext;
        use rustok_pages::{ListPagesFilter as RuntimeListPagesFilter, PageService};
        use rustok_tenant::TenantService;

        let app_ctx = expect_context::<AppContext>();
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
            let tenant = TenantService::new(app_ctx.db.clone())
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
                let enabled = ChannelService::new(app_ctx.db.clone())
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

        let service = PageService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );

        let selected_page = service
            .get_by_slug_with_locale_fallback(
                tenant_id,
                SecurityContext::system(),
                requested_locale.as_str(),
                page_slug.as_str(),
                Some(fallback_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .filter(|page| {
                is_visible_for_public_channel(&page.channel_slugs, public_channel_slug.as_deref())
            })
            .map(map_page_detail);

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
fn map_page_detail(page: rustok_pages::PageResponse) -> PageDetail {
    PageDetail {
        effective_locale: page.effective_locale,
        translation: page.translation.map(|translation| PageTranslation {
            locale: translation.locale,
            title: translation.title,
            slug: translation.slug,
            meta_title: translation.meta_title,
            meta_description: translation.meta_description,
        }),
        body: page.body.map(|body| PageBody {
            locale: body.locale,
            content: body.content,
            format: body.format,
        }),
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
    }
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
