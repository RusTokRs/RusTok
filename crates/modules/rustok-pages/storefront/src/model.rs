use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontPagesData {
    pub selected_page: Option<PageDetail>,
    pub pages: PageList,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageList {
    pub items: Vec<PageListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageListItem {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub status: String,
    pub template: String,
}

/// A published page as a visitor sees it.
///
/// Unknown fields are rejected so that a snapshot written with another shape (for example a
/// cached entry from a different release) is a decode error, which the cache treats as a miss,
/// instead of a page that silently lost its landing.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageDetail {
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: Option<String>,
    pub translation: Option<PageTranslation>,
    /// Published landing of the page; `None` when the page has no published landing for the
    /// request locale and channel.
    pub landing: Option<PublishedLanding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageTranslation {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "metaTitle")]
    pub meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    pub meta_description: Option<String>,
}

/// The published landing artifact of one page locale: the sanitized markup and stylesheet the
/// reviewed publish stored. It is the only page content the storefront renders; the authoring
/// document of a page never reaches this crate.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishedLanding {
    /// Locale the landing was published for; it can differ from the requested locale when a
    /// fallback locale was used.
    pub locale: String,
    /// Sanitized body markup.
    pub html: String,
    /// Stylesheet of the landing, not yet scoped to the host element.
    pub css: String,
    /// Identity of the artifact; changes whenever the markup or the stylesheet changes.
    #[serde(rename = "artifactHash")]
    pub artifact_hash: String,
}

#[cfg(test)]
mod tests {
    use super::{PageDetail, PublishedLanding};

    #[test]
    fn page_detail_decodes_the_graphql_landing_shape() {
        let page: PageDetail = serde_json::from_str(
            r#"{
                "effectiveLocale": "en",
                "translation": null,
                "landing": {
                    "locale": "en",
                    "html": "<h1>Landing</h1>",
                    "css": "h1 { color: red; }",
                    "artifactHash": "hash"
                }
            }"#,
        )
        .expect("page detail");
        assert_eq!(
            page.landing,
            Some(PublishedLanding {
                locale: "en".to_string(),
                html: "<h1>Landing</h1>".to_string(),
                css: "h1 { color: red; }".to_string(),
                artifact_hash: "hash".to_string(),
            })
        );
    }

    #[test]
    fn page_detail_round_trips_through_the_cache_encoding() {
        let page = PageDetail {
            effective_locale: Some("en".to_string()),
            translation: None,
            landing: Some(PublishedLanding {
                locale: "en".to_string(),
                html: "<h1>Landing</h1>".to_string(),
                css: String::new(),
                artifact_hash: "hash".to_string(),
            }),
        };
        let encoded = serde_json::to_string(&page).expect("encode");
        let decoded: PageDetail = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded.landing, page.landing);
        assert_eq!(decoded.effective_locale, page.effective_locale);
    }

    #[test]
    fn a_snapshot_with_another_shape_is_a_decode_error_not_an_empty_page() {
        let authoring_body_snapshot = r#"{
            "effectiveLocale": "en",
            "translation": null,
            "body": { "locale": "en", "content": "{}", "format": "grapesjs" }
        }"#;
        assert!(serde_json::from_str::<PageDetail>(authoring_body_snapshot).is_err());
    }
}
