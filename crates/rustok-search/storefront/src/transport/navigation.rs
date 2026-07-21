use crate::model::SearchPreviewPayload;

const BLOG_SOURCE_MODULE: &str = "blog";
const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
const MAX_BLOG_SLUG_LEN: usize = 200;

/// Completes transport-neutral navigation after either native or GraphQL search
/// has returned. Existing backend URLs remain authoritative; Blog results only
/// receive a fallback when the indexed owner payload carries a valid slug.
pub(super) fn enrich_search_result_urls(payload: &mut SearchPreviewPayload) {
    for item in &mut payload.items {
        if item.url.is_some()
            || item.source_module != BLOG_SOURCE_MODULE
            || item.entity_type != BLOG_ENTITY_TYPE
        {
            continue;
        }

        item.url = blog_result_url(item.payload.as_str());
    }
}

fn blog_result_url(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let slug = value.get("slug")?.as_str()?.trim();
    if !valid_blog_slug(slug) {
        return None;
    }

    Some(format!("{BLOG_STOREFRONT_ROUTE}?slug={slug}"))
}

fn valid_blog_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= MAX_BLOG_SLUG_LEN
        && slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SearchPreviewPayload, SearchPreviewResultItem};

    fn payload(item: SearchPreviewResultItem) -> SearchPreviewPayload {
        SearchPreviewPayload {
            query_log_id: None,
            preset_key: None,
            items: vec![item],
            total: 1,
            took_ms: 1,
            engine: "postgres".to_string(),
            ranking_profile: "default".to_string(),
            facets: Vec::new(),
        }
    }

    fn item(payload: &str) -> SearchPreviewResultItem {
        SearchPreviewResultItem {
            id: "00000000-0000-0000-0000-000000000001".to_string(),
            entity_type: BLOG_ENTITY_TYPE.to_string(),
            source_module: BLOG_SOURCE_MODULE.to_string(),
            title: "Post".to_string(),
            snippet: None,
            score: 1.0,
            locale: Some("en".to_string()),
            url: None,
            payload: payload.to_string(),
        }
    }

    #[test]
    fn derives_canonical_blog_route_from_projected_slug() {
        let mut result = payload(item(r#"{"slug":"release-notes-2026"}"#));

        enrich_search_result_urls(&mut result);

        assert_eq!(
            result.items[0].url.as_deref(),
            Some("/modules/blog?slug=release-notes-2026")
        );
    }

    #[test]
    fn preserves_backend_url_and_rejects_invalid_slug() {
        let mut existing = item(r#"{"slug":"ignored"}"#);
        existing.url = Some("/custom/blog/ignored".to_string());
        let mut existing_result = payload(existing);
        enrich_search_result_urls(&mut existing_result);
        assert_eq!(
            existing_result.items[0].url.as_deref(),
            Some("/custom/blog/ignored")
        );

        for raw in [
            r#"{"slug":""}"#,
            r#"{"slug":"../admin"}"#,
            r#"{"slug":"hello world"}"#,
            r#"{"slug":7}"#,
            "not-json",
        ] {
            let mut result = payload(item(raw));
            enrich_search_result_urls(&mut result);
            assert_eq!(result.items[0].url, None, "payload must fail closed: {raw}");
        }
    }

    #[test]
    fn does_not_enrich_non_blog_documents() {
        let mut result = payload(SearchPreviewResultItem {
            source_module: "catalog".to_string(),
            entity_type: "product".to_string(),
            ..item(r#"{"slug":"boots"}"#)
        });

        enrich_search_result_urls(&mut result);

        assert_eq!(result.items[0].url, None);
    }
}
