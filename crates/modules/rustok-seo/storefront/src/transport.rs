use crate::model::SeoPageContext;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SeoPageContextVariables {
    pub route: String,
    pub locale: String,
}

#[derive(Debug, Deserialize)]
pub struct SeoPageContextResponse {
    #[serde(rename = "seoPageContext")]
    pub seo_page_context: Option<SeoPageContext>,
}

pub const SEO_PAGE_CONTEXT_QUERY: &str = r#"
    query SeoPageContext($route: String!, $locale: String) {
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
