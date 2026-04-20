use serde_json::Value;

use crate::dto::{
    SeoDocument, SeoImageAsset, SeoMetaTag, SeoOpenGraph, SeoRobots, SeoStructuredDataBlock,
    SeoTwitterCard,
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
) -> SeoDocument {
    if let Some(open_graph_value) = open_graph.as_mut() {
        if open_graph_value.url.is_none() {
            open_graph_value.url = Some(canonical_url.to_string());
        }
        if open_graph_value.locale.is_none() {
            open_graph_value.locale = Some(effective_locale.to_string());
        }
    }
    let twitter = open_graph.as_ref().map(twitter_from_open_graph);
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
        structured_data_blocks: vec![SeoStructuredDataBlock {
            id: None,
            kind: structured_data
                .get("@type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            payload: async_graphql::Json(structured_data),
        }],
        meta_tags,
        link_tags: Vec::new(),
    }
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
