use serde_json::Value;

use crate::dto::{
    SeoDocument, SeoDocumentEffectiveState, SeoFieldSource, SeoImageAsset, SeoMetaTag,
    SeoOpenGraph, SeoRobots, SeoSchemaBlockKind, SeoStructuredDataBlock, SeoTwitterCard,
};

pub(super) fn normalize_robots(defaults: &[String]) -> Vec<String> {
    robots_to_directives(&robots_from_directives(defaults))
}

pub(super) fn apply_robots(noindex: bool, nofollow: bool, defaults: &[String]) -> SeoRobots {
    let mut robots = robots_from_directives(defaults);
    if noindex {
        robots.index = false;
    }
    if nofollow {
        robots.follow = false;
    }
    robots
}

pub(super) fn robots_from_directives(directives: &[String]) -> SeoRobots {
    let mut robots = SeoRobots::default();
    for directive in directives {
        let token = directive.trim().to_ascii_lowercase();
        if token.is_empty() {
            continue;
        }

        match token.as_str() {
            "index" => robots.index = true,
            "noindex" => robots.index = false,
            "follow" => robots.follow = true,
            "nofollow" => robots.follow = false,
            "noarchive" => robots.noarchive = true,
            "nosnippet" => robots.nosnippet = true,
            "noimageindex" => robots.noimageindex = true,
            "notranslate" => robots.notranslate = true,
            _ if token.starts_with("max-snippet:") => {
                robots.max_snippet = token
                    .split_once(':')
                    .and_then(|(_, value)| value.parse::<i32>().ok());
            }
            _ if token.starts_with("max-image-preview:") => {
                robots.max_image_preview = token
                    .split_once(':')
                    .map(|(_, value)| value.to_string())
                    .filter(|value| !value.is_empty());
            }
            _ if token.starts_with("max-video-preview:") => {
                robots.max_video_preview = token
                    .split_once(':')
                    .and_then(|(_, value)| value.parse::<i32>().ok());
            }
            _ => robots.custom.push(token),
        }
    }

    robots.custom.sort();
    robots.custom.dedup();
    robots
}

pub(super) fn robots_to_directives(robots: &SeoRobots) -> Vec<String> {
    let mut directives = vec![
        if robots.index {
            "index".to_string()
        } else {
            "noindex".to_string()
        },
        if robots.follow {
            "follow".to_string()
        } else {
            "nofollow".to_string()
        },
    ];

    if robots.noarchive {
        directives.push("noarchive".to_string());
    }
    if robots.nosnippet {
        directives.push("nosnippet".to_string());
    }
    if robots.noimageindex {
        directives.push("noimageindex".to_string());
    }
    if robots.notranslate {
        directives.push("notranslate".to_string());
    }
    if let Some(value) = robots.max_snippet {
        directives.push(format!("max-snippet:{value}"));
    }
    if let Some(value) = robots.max_image_preview.as_deref() {
        directives.push(format!("max-image-preview:{value}"));
    }
    if let Some(value) = robots.max_video_preview {
        directives.push(format!("max-video-preview:{value}"));
    }
    directives.extend(robots.custom.iter().cloned());
    directives
}

pub(super) fn image_assets_from_optional_url(url: Option<String>) -> Vec<SeoImageAsset> {
    url.into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(|url| SeoImageAsset {
            url,
            alt: None,
            width: None,
            height: None,
            mime_type: None,
        })
        .collect()
}

pub(super) fn first_open_graph_image_url(open_graph: &SeoOpenGraph) -> Option<String> {
    open_graph
        .images
        .iter()
        .find(|item| !item.url.trim().is_empty())
        .map(|item| item.url.clone())
}

pub(super) fn merge_open_graph(
    fallback: &SeoOpenGraph,
    title: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
    canonical_url: &str,
    effective_locale: &str,
) -> SeoOpenGraph {
    let mut open_graph = fallback.clone();
    open_graph.title = title.or(open_graph.title);
    open_graph.description = description.or(open_graph.description);
    if let Some(image_url) = image_url {
        open_graph.images = image_assets_from_optional_url(Some(image_url));
    }
    open_graph.url = Some(canonical_url.to_string());
    open_graph.locale = Some(effective_locale.to_string());
    open_graph
}

pub(super) fn build_document(
    title: String,
    description: Option<String>,
    robots: SeoRobots,
    mut open_graph: Option<SeoOpenGraph>,
    structured_data: Value,
    keywords: Option<String>,
    canonical_url: &str,
    effective_locale: &str,
    effective_state: SeoDocumentEffectiveState,
    twitter_title: Option<String>,
    twitter_description: Option<String>,
) -> SeoDocument {
    if let Some(open_graph_value) = open_graph.as_mut() {
        if open_graph_value.url.is_none() {
            open_graph_value.url = Some(canonical_url.to_string());
        }
        if open_graph_value.locale.is_none() {
            open_graph_value.locale = Some(effective_locale.to_string());
        }
    }
    let twitter = open_graph
        .as_ref()
        .map(twitter_from_open_graph)
        .map(|mut twitter| {
            if twitter_title.is_some() {
                twitter.title = twitter_title;
            }
            if twitter_description.is_some() {
                twitter.description = twitter_description;
            }
            twitter
        });
    let mut meta_tags = Vec::new();
    if let Some(keywords) = keywords.filter(|value| !value.trim().is_empty()) {
        meta_tags.push(SeoMetaTag {
            name: Some("keywords".to_string()),
            property: None,
            http_equiv: None,
            content: keywords,
        });
    }

    SeoDocument {
        title,
        description,
        robots,
        open_graph,
        twitter,
        verification: None,
        pagination: None,
        structured_data_blocks: schema_blocks_from_value(
            structured_data,
            effective_state.structured_data.source,
        ),
        meta_tags,
        link_tags: Vec::new(),
        effective_state,
    }
}

pub(super) fn schema_blocks_from_value(
    value: Value,
    source: SeoFieldSource,
) -> Vec<SeoStructuredDataBlock> {
    let inherited_context = value.get("@context").cloned();
    let mut blocks = Vec::new();
    collect_schema_blocks(value, source, inherited_context.as_ref(), &mut blocks);
    blocks
}

fn collect_schema_blocks(
    value: Value,
    source: SeoFieldSource,
    inherited_context: Option<&Value>,
    blocks: &mut Vec<SeoStructuredDataBlock>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_schema_blocks(item, source, inherited_context, blocks);
            }
        }
        Value::Object(mut object) => {
            if let Some(Value::Array(graph)) = object.remove("@graph") {
                let graph_context = object.get("@context").or(inherited_context).cloned();
                for item in graph {
                    collect_schema_blocks(item, source, graph_context.as_ref(), blocks);
                }
                return;
            }

            if !object.contains_key("@context") {
                if let Some(context) = inherited_context {
                    object.insert("@context".to_string(), context.clone());
                } else {
                    object.insert(
                        "@context".to_string(),
                        Value::String("https://schema.org".to_string()),
                    );
                }
            }

            let value = Value::Object(object);
            let schema_type = first_schema_type(&value);
            let schema_kind = schema_type
                .as_deref()
                .map(SeoSchemaBlockKind::from_schema_type)
                .unwrap_or(SeoSchemaBlockKind::Unknown);
            let id = value
                .get("@id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            blocks.push(SeoStructuredDataBlock {
                id,
                schema_kind,
                schema_type: schema_type.clone(),
                kind: schema_type,
                source,
                payload: async_graphql::Json(value),
            });
        }
        _ => {}
    }
}

fn first_schema_type(value: &Value) -> Option<String> {
    let raw = value.get("@type")?;
    if let Some(schema_type) = raw.as_str() {
        return Some(schema_type.to_string()).filter(|value| !value.trim().is_empty());
    }
    raw.as_array().and_then(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .find(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
    })
}

pub(super) fn twitter_from_open_graph(open_graph: &SeoOpenGraph) -> SeoTwitterCard {
    SeoTwitterCard {
        card: Some(if open_graph.images.is_empty() {
            "summary".to_string()
        } else {
            "summary_large_image".to_string()
        }),
        title: open_graph.title.clone(),
        description: open_graph.description.clone(),
        site: None,
        creator: None,
        images: open_graph.images.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::dto::{SeoFieldSource, SeoSchemaBlockKind};

    use super::schema_blocks_from_value;

    #[test]
    fn schema_blocks_flatten_graph_and_keep_typed_kinds() {
        let blocks = schema_blocks_from_value(
            json!({
                "@context": "https://schema.org",
                "@graph": [
                    {"@id": "#product", "@type": "Product", "name": "Demo"},
                    {"@type": "BreadcrumbList", "itemListElement": []}
                ]
            }),
            SeoFieldSource::Fallback,
        );

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id.as_deref(), Some("#product"));
        assert_eq!(blocks[0].schema_kind, SeoSchemaBlockKind::Product);
        assert_eq!(blocks[0].schema_type.as_deref(), Some("Product"));
        assert_eq!(blocks[0].kind.as_deref(), Some("Product"));
        assert_eq!(blocks[0].source, SeoFieldSource::Fallback);
        assert_eq!(blocks[1].schema_kind, SeoSchemaBlockKind::BreadcrumbList);
        assert_eq!(
            blocks[1]
                .payload
                .0
                .get("@context")
                .and_then(|value| value.as_str()),
            Some("https://schema.org")
        );
    }

    #[test]
    fn schema_blocks_ignore_non_object_json_ld_payloads() {
        let blocks = schema_blocks_from_value(json!("not-json-ld"), SeoFieldSource::Explicit);

        assert!(blocks.is_empty());
    }
}
