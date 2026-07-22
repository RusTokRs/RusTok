use std::collections::{BTreeMap, HashMap};

use rustok_ui_transport::UiTransportPath;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::api::{ApiError, configured_tenant_slug};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

const SEO_PAGE_CONTEXT_QUERY: &str = r#"
    query SeoPageContext($route: String!, $locale: String!) {
        seoPageContext(route: $route, locale: $locale) {
            route {
                targetKind
                targetId
                requestedLocale
                effectiveLocale
                canonicalUrl
                redirect {
                    targetUrl
                    statusCode
                }
                alternates {
                    locale
                    href
                    xDefault
                }
            }
            document {
                title
                description
                robots {
                    index
                    follow
                    noarchive
                    nosnippet
                    noimageindex
                    notranslate
                    maxSnippet
                    maxImagePreview
                    maxVideoPreview
                    custom
                }
                openGraph {
                    title
                    description
                    kind
                    siteName
                    url
                    locale
                    images {
                        url
                        alt
                        width
                        height
                        mimeType
                    }
                }
                twitter {
                    card
                    title
                    description
                    site
                    creator
                    images {
                        url
                        alt
                        width
                        height
                        mimeType
                    }
                }
                verification {
                    google
                    yandex
                    yahoo
                    other {
                        name
                        value
                    }
                }
                pagination {
                    prevUrl
                    nextUrl
                }
                structuredDataBlocks {
                    id
                    schemaKind
                    schemaType
                    kind
                    source
                    payload
                }
                metaTags {
                    name
                    property
                    httpEquiv
                    content
                }
                linkTags {
                    rel
                    href
                    hreflang
                    media
                    mimeType
                    title
                }
            }
        }
    }
"#;

#[derive(Debug, Clone, Serialize)]
struct SeoPageContextVariables {
    route: String,
    locale: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SeoPageContextResponse {
    #[serde(rename = "seoPageContext")]
    seo_page_context: Option<ResolvedSeoPageContext>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoAlternateLink {
    pub locale: String,
    pub href: String,
    #[serde(rename = "xDefault")]
    pub x_default: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoRedirectDecision {
    #[serde(rename = "targetUrl")]
    pub target_url: String,
    #[serde(rename = "statusCode")]
    pub status_code: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoRouteContext {
    #[serde(rename = "targetKind")]
    pub target_kind: Option<String>,
    #[serde(rename = "targetId")]
    pub target_id: Option<String>,
    #[serde(rename = "requestedLocale")]
    pub requested_locale: Option<String>,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "canonicalUrl")]
    pub canonical_url: String,
    pub redirect: Option<ResolvedSeoRedirectDecision>,
    pub alternates: Vec<ResolvedSeoAlternateLink>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoImageAsset {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoOpenGraph {
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    #[serde(rename = "siteName")]
    pub site_name: Option<String>,
    pub url: Option<String>,
    pub locale: Option<String>,
    pub images: Vec<ResolvedSeoImageAsset>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoTwitterCard {
    pub card: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub site: Option<String>,
    pub creator: Option<String>,
    pub images: Vec<ResolvedSeoImageAsset>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoVerificationTag {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoVerification {
    pub google: Vec<String>,
    pub yandex: Vec<String>,
    pub yahoo: Vec<String>,
    pub other: Vec<ResolvedSeoVerificationTag>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoPagination {
    #[serde(rename = "prevUrl")]
    pub prev_url: Option<String>,
    #[serde(rename = "nextUrl")]
    pub next_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoStructuredDataBlock {
    pub id: Option<String>,
    #[serde(rename = "schemaKind")]
    pub schema_kind: String,
    #[serde(rename = "schemaType")]
    pub schema_type: Option<String>,
    pub kind: Option<String>,
    pub source: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoMetaTag {
    pub name: Option<String>,
    pub property: Option<String>,
    #[serde(rename = "httpEquiv")]
    pub http_equiv: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoLinkTag {
    pub rel: String,
    pub href: String,
    pub hreflang: Option<String>,
    pub media: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResolvedSeoRobots {
    pub index: bool,
    pub follow: bool,
    pub noarchive: bool,
    pub nosnippet: bool,
    pub noimageindex: bool,
    pub notranslate: bool,
    #[serde(rename = "maxSnippet")]
    pub max_snippet: Option<i32>,
    #[serde(rename = "maxImagePreview")]
    pub max_image_preview: Option<String>,
    #[serde(rename = "maxVideoPreview")]
    pub max_video_preview: Option<i32>,
    pub custom: Vec<String>,
}

impl Default for ResolvedSeoRobots {
    fn default() -> Self {
        Self {
            index: true,
            follow: true,
            noarchive: false,
            nosnippet: false,
            noimageindex: false,
            notranslate: false,
            max_snippet: None,
            max_image_preview: None,
            max_video_preview: None,
            custom: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoDocument {
    pub title: String,
    pub description: Option<String>,
    pub robots: ResolvedSeoRobots,
    #[serde(rename = "openGraph")]
    pub open_graph: Option<ResolvedSeoOpenGraph>,
    pub twitter: Option<ResolvedSeoTwitterCard>,
    pub verification: Option<ResolvedSeoVerification>,
    pub pagination: Option<ResolvedSeoPagination>,
    #[serde(rename = "structuredDataBlocks")]
    pub structured_data_blocks: Vec<ResolvedSeoStructuredDataBlock>,
    #[serde(rename = "metaTags")]
    pub meta_tags: Vec<ResolvedSeoMetaTag>,
    #[serde(rename = "linkTags")]
    pub link_tags: Vec<ResolvedSeoLinkTag>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ResolvedSeoPageContext {
    pub route: ResolvedSeoRouteContext,
    pub document: ResolvedSeoDocument,
}

pub async fn fetch_seo_page_context(
    locale: &str,
    route_segment: &str,
    query_params: &HashMap<String, String>,
) -> Result<Option<ResolvedSeoPageContext>, ApiError> {
    let Some(tenant_slug) = configured_tenant_slug() else {
        return Ok(None);
    };

    let route = build_module_route(route_segment, query_params);
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            fetch_seo_page_context_server(tenant_slug, locale.to_string(), route).await
        }
        UiTransportPath::Graphql => {
            fetch_seo_page_context_graphql(tenant_slug, locale.to_string(), route).await
        }
    }
}

pub async fn fetch_seo_page_context_server(
    tenant_slug: String,
    locale: String,
    route: String,
) -> Result<Option<ResolvedSeoPageContext>, ApiError> {
    super::seo_page_context_native_server_adapter::resolve_seo_page_context(
        tenant_slug,
        locale,
        route,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_seo_page_context_graphql(
    tenant_slug: String,
    locale: String,
    route: String,
) -> Result<Option<ResolvedSeoPageContext>, ApiError> {
    let response: SeoPageContextResponse = crate::shared::api::request(
        SEO_PAGE_CONTEXT_QUERY,
        SeoPageContextVariables { route, locale },
        None,
        Some(tenant_slug),
    )
    .await?;
    Ok(response.seo_page_context)
}

#[cfg(feature = "ssr")]
pub fn to_seo_page_context(value: &ResolvedSeoPageContext) -> rustok_seo::SeoPageContext {
    rustok_seo::SeoPageContext {
        route: rustok_seo::SeoRouteContext {
            target_kind: value
                .route
                .target_kind
                .as_deref()
                .and_then(|item| rustok_seo::SeoTargetSlug::new(item).ok()),
            target_id: value
                .route
                .target_id
                .as_deref()
                .and_then(|item| rustok_core::prelude::Uuid::parse_str(item).ok()),
            requested_locale: value.route.requested_locale.clone(),
            effective_locale: value.route.effective_locale.clone(),
            canonical_url: value.route.canonical_url.clone(),
            redirect: value
                .route
                .redirect
                .as_ref()
                .map(|item| rustok_seo::SeoRedirectDecision {
                    target_url: item.target_url.clone(),
                    status_code: item.status_code,
                }),
            alternates: value
                .route
                .alternates
                .iter()
                .map(|item| rustok_seo::SeoAlternateLink {
                    locale: item.locale.clone(),
                    href: item.href.clone(),
                    x_default: item.x_default,
                })
                .collect(),
        },
        document: rustok_seo::SeoDocument {
            title: value.document.title.clone(),
            description: value.document.description.clone(),
            robots: rustok_seo::SeoRobots {
                index: value.document.robots.index,
                follow: value.document.robots.follow,
                noarchive: value.document.robots.noarchive,
                nosnippet: value.document.robots.nosnippet,
                noimageindex: value.document.robots.noimageindex,
                notranslate: value.document.robots.notranslate,
                max_snippet: value.document.robots.max_snippet,
                max_image_preview: value.document.robots.max_image_preview.clone(),
                max_video_preview: value.document.robots.max_video_preview,
                custom: value.document.robots.custom.clone(),
            },
            open_graph: value
                .document
                .open_graph
                .as_ref()
                .map(|item| rustok_seo::SeoOpenGraph {
                    title: item.title.clone(),
                    description: item.description.clone(),
                    kind: item.kind.clone(),
                    site_name: item.site_name.clone(),
                    url: item.url.clone(),
                    locale: item.locale.clone(),
                    images: item
                        .images
                        .iter()
                        .map(|image| rustok_seo::SeoImageAsset {
                            url: image.url.clone(),
                            alt: image.alt.clone(),
                            width: image.width,
                            height: image.height,
                            mime_type: image.mime_type.clone(),
                        })
                        .collect(),
                }),
            twitter: value
                .document
                .twitter
                .as_ref()
                .map(|item| rustok_seo::SeoTwitterCard {
                    card: item.card.clone(),
                    title: item.title.clone(),
                    description: item.description.clone(),
                    site: item.site.clone(),
                    creator: item.creator.clone(),
                    images: item
                        .images
                        .iter()
                        .map(|image| rustok_seo::SeoImageAsset {
                            url: image.url.clone(),
                            alt: image.alt.clone(),
                            width: image.width,
                            height: image.height,
                            mime_type: image.mime_type.clone(),
                        })
                        .collect(),
                }),
            verification: value.document.verification.as_ref().map(|item| {
                rustok_seo::SeoVerification {
                    google: item.google.clone(),
                    yandex: item.yandex.clone(),
                    yahoo: item.yahoo.clone(),
                    other: item
                        .other
                        .iter()
                        .map(|tag| rustok_seo::SeoVerificationTag {
                            name: tag.name.clone(),
                            value: tag.value.clone(),
                        })
                        .collect(),
                }
            }),
            pagination: value
                .document
                .pagination
                .as_ref()
                .map(|item| rustok_seo::SeoPagination {
                    prev_url: item.prev_url.clone(),
                    next_url: item.next_url.clone(),
                }),
            structured_data_blocks: value
                .document
                .structured_data_blocks
                .iter()
                .map(|item| rustok_seo::SeoStructuredDataBlock {
                    id: item.id.clone(),
                    schema_kind: resolved_schema_kind(item.schema_kind.as_str()),
                    schema_type: item.schema_type.clone(),
                    kind: item.kind.clone(),
                    source: resolved_field_source(item.source.as_str()),
                    payload: item.payload.clone().into(),
                })
                .collect(),
            meta_tags: value
                .document
                .meta_tags
                .iter()
                .map(|item| rustok_seo::SeoMetaTag {
                    name: item.name.clone(),
                    property: item.property.clone(),
                    http_equiv: item.http_equiv.clone(),
                    content: item.content.clone(),
                })
                .collect(),
            link_tags: value
                .document
                .link_tags
                .iter()
                .map(|item| rustok_seo::SeoLinkTag {
                    rel: item.rel.clone(),
                    href: item.href.clone(),
                    hreflang: item.hreflang.clone(),
                    media: item.media.clone(),
                    mime_type: item.mime_type.clone(),
                    title: item.title.clone(),
                })
                .collect(),
            effective_state: rustok_seo::SeoDocumentEffectiveState::default(),
        },
    }
}

#[cfg(feature = "ssr")]
fn resolved_schema_kind(value: &str) -> rustok_seo::SeoSchemaBlockKind {
    match value {
        "product" => rustok_seo::SeoSchemaBlockKind::Product,
        "offer" => rustok_seo::SeoSchemaBlockKind::Offer,
        "aggregate_offer" => rustok_seo::SeoSchemaBlockKind::AggregateOffer,
        "aggregate_rating" => rustok_seo::SeoSchemaBlockKind::AggregateRating,
        "review" => rustok_seo::SeoSchemaBlockKind::Review,
        "breadcrumb_list" => rustok_seo::SeoSchemaBlockKind::BreadcrumbList,
        "item_list" => rustok_seo::SeoSchemaBlockKind::ItemList,
        "organization" => rustok_seo::SeoSchemaBlockKind::Organization,
        "local_business" => rustok_seo::SeoSchemaBlockKind::LocalBusiness,
        "web_site" => rustok_seo::SeoSchemaBlockKind::WebSite,
        "search_action" => rustok_seo::SeoSchemaBlockKind::SearchAction,
        "article" => rustok_seo::SeoSchemaBlockKind::Article,
        "blog_posting" => rustok_seo::SeoSchemaBlockKind::BlogPosting,
        "news_article" => rustok_seo::SeoSchemaBlockKind::NewsArticle,
        "faq_page" => rustok_seo::SeoSchemaBlockKind::FAQPage,
        "how_to" => rustok_seo::SeoSchemaBlockKind::HowTo,
        "video_object" => rustok_seo::SeoSchemaBlockKind::VideoObject,
        "image_object" => rustok_seo::SeoSchemaBlockKind::ImageObject,
        "discussion_forum_posting" => rustok_seo::SeoSchemaBlockKind::DiscussionForumPosting,
        "question" => rustok_seo::SeoSchemaBlockKind::Question,
        "answer" => rustok_seo::SeoSchemaBlockKind::Answer,
        "web_page" => rustok_seo::SeoSchemaBlockKind::WebPage,
        "collection_page" => rustok_seo::SeoSchemaBlockKind::CollectionPage,
        "other" => rustok_seo::SeoSchemaBlockKind::Other,
        _ => rustok_seo::SeoSchemaBlockKind::Unknown,
    }
}

#[cfg(feature = "ssr")]
fn resolved_field_source(value: &str) -> rustok_seo::SeoFieldSource {
    match value {
        "explicit" => rustok_seo::SeoFieldSource::Explicit,
        "generated" => rustok_seo::SeoFieldSource::Generated,
        _ => rustok_seo::SeoFieldSource::Fallback,
    }
}

fn build_module_route(route_segment: &str, query_params: &HashMap<String, String>) -> String {
    let base = format!("/modules/{route_segment}");
    let filtered = query_params
        .iter()
        .filter(|(key, _)| key.as_str() != "lang")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if filtered.is_empty() {
        return base;
    }
    let query = serde_urlencoded::to_string(filtered)
        .expect("serializing module route query should not fail");
    format!("{base}?{query}")
}
