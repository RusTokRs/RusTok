use rustok_core::html_escape;
use rustok_seo::{SeoImageAsset, SeoLinkTag, SeoMetaTag, SeoPageContext, SeoRobots};

pub fn render_head_html(context: &SeoPageContext) -> String {
    let mut head = String::new();
    let route = &context.route;
    let document = &context.document;

    if let Some(description) = document.description.as_deref() {
        if !description.trim().is_empty() {
            head.push_str(&format!(
                r#"<meta name="description" content="{}" />"#,
                html_escape(description)
            ));
        }
    }

    if !route.canonical_url.trim().is_empty() {
        head.push_str(&format!(
            r#"<link rel="canonical" href="{}" />"#,
            html_escape(route.canonical_url.as_str())
        ));
    }

    let robots = robots_directives(&document.robots);
    if !robots.is_empty() {
        head.push_str(&format!(
            r#"<meta name="robots" content="{}" />"#,
            html_escape(robots.join(", ").as_str())
        ));
    }

    for alternate in &route.alternates {
        head.push_str(&format!(
            r#"<link rel="alternate" hreflang="{}" href="{}" />"#,
            html_escape(alternate.locale.as_str()),
            html_escape(alternate.href.as_str())
        ));
    }

    if let Some(open_graph) = document.open_graph.as_ref() {
        if let Some(title) = open_graph.title.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:title" content="{}" />"#,
                html_escape(title)
            ));
        }
        if let Some(description) = open_graph.description.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:description" content="{}" />"#,
                html_escape(description)
            ));
        }
        if let Some(kind) = open_graph.kind.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:type" content="{}" />"#,
                html_escape(kind)
            ));
        }
        if let Some(site_name) = open_graph.site_name.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:site_name" content="{}" />"#,
                html_escape(site_name)
            ));
        }
        if let Some(url) = open_graph.url.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:url" content="{}" />"#,
                html_escape(url)
            ));
        }
        if let Some(locale) = open_graph.locale.as_deref() {
            head.push_str(&format!(
                r#"<meta property="og:locale" content="{}" />"#,
                html_escape(locale)
            ));
        }
        for image in &open_graph.images {
            render_open_graph_image(&mut head, image);
        }
    }

    if let Some(twitter) = document.twitter.as_ref() {
        if let Some(card) = twitter.card.as_deref() {
            head.push_str(&format!(
                r#"<meta name="twitter:card" content="{}" />"#,
                html_escape(card)
            ));
        }
        if let Some(title) = twitter.title.as_deref() {
            head.push_str(&format!(
                r#"<meta name="twitter:title" content="{}" />"#,
                html_escape(title)
            ));
        }
        if let Some(description) = twitter.description.as_deref() {
            head.push_str(&format!(
                r#"<meta name="twitter:description" content="{}" />"#,
                html_escape(description)
            ));
        }
        if let Some(site) = twitter.site.as_deref() {
            head.push_str(&format!(
                r#"<meta name="twitter:site" content="{}" />"#,
                html_escape(site)
            ));
        }
        if let Some(creator) = twitter.creator.as_deref() {
            head.push_str(&format!(
                r#"<meta name="twitter:creator" content="{}" />"#,
                html_escape(creator)
            ));
        }
        for image in &twitter.images {
            head.push_str(&format!(
                r#"<meta name="twitter:image" content="{}" />"#,
                html_escape(image.url.as_str())
            ));
        }
    }

    if let Some(verification) = document.verification.as_ref() {
        for token in &verification.google {
            head.push_str(&format!(
                r#"<meta name="google-site-verification" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }
        for token in &verification.yandex {
            head.push_str(&format!(
                r#"<meta name="yandex-verification" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }
        for token in &verification.yahoo {
            head.push_str(&format!(
                r#"<meta name="y_key" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }
        for token in &verification.other {
            head.push_str(&format!(
                r#"<meta name="{}" content="{}" />"#,
                html_escape(token.name.as_str()),
                html_escape(token.value.as_str())
            ));
        }
    }

    if let Some(pagination) = document.pagination.as_ref() {
        if let Some(prev_url) = pagination.prev_url.as_deref() {
            head.push_str(&format!(
                r#"<link rel="prev" href="{}" />"#,
                html_escape(prev_url)
            ));
        }
        if let Some(next_url) = pagination.next_url.as_deref() {
            head.push_str(&format!(
                r#"<link rel="next" href="{}" />"#,
                html_escape(next_url)
            ));
        }
    }

    for tag in &document.meta_tags {
        render_meta_tag(&mut head, tag);
    }

    for tag in &document.link_tags {
        render_link_tag(&mut head, tag);
    }

    for block in &document.structured_data_blocks {
        if let Ok(payload) = serde_json::to_string(&block.payload.0) {
            head.push_str(&format!(
                r#"<script type="application/ld+json">{payload}</script>"#
            ));
        }
    }

    head
}

pub fn robots_directives(robots: &SeoRobots) -> Vec<String> {
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

fn render_open_graph_image(head: &mut String, image: &SeoImageAsset) {
    head.push_str(&format!(
        r#"<meta property="og:image" content="{}" />"#,
        html_escape(image.url.as_str())
    ));
    if let Some(alt) = image.alt.as_deref() {
        head.push_str(&format!(
            r#"<meta property="og:image:alt" content="{}" />"#,
            html_escape(alt)
        ));
    }
    if let Some(width) = image.width {
        head.push_str(&format!(
            r#"<meta property="og:image:width" content="{width}" />"#
        ));
    }
    if let Some(height) = image.height {
        head.push_str(&format!(
            r#"<meta property="og:image:height" content="{height}" />"#
        ));
    }
    if let Some(mime_type) = image.mime_type.as_deref() {
        head.push_str(&format!(
            r#"<meta property="og:image:type" content="{}" />"#,
            html_escape(mime_type)
        ));
    }
}

fn render_meta_tag(head: &mut String, tag: &SeoMetaTag) {
    if let Some(name) = tag.name.as_deref() {
        head.push_str(&format!(
            r#"<meta name="{}" content="{}" />"#,
            html_escape(name),
            html_escape(tag.content.as_str())
        ));
    } else if let Some(property) = tag.property.as_deref() {
        head.push_str(&format!(
            r#"<meta property="{}" content="{}" />"#,
            html_escape(property),
            html_escape(tag.content.as_str())
        ));
    } else if let Some(http_equiv) = tag.http_equiv.as_deref() {
        head.push_str(&format!(
            r#"<meta http-equiv="{}" content="{}" />"#,
            html_escape(http_equiv),
            html_escape(tag.content.as_str())
        ));
    }
}

fn render_link_tag(head: &mut String, tag: &SeoLinkTag) {
    let mut attributes = vec![
        format!(r#"rel="{}""#, html_escape(tag.rel.as_str())),
        format!(r#"href="{}""#, html_escape(tag.href.as_str())),
    ];
    if let Some(hreflang) = tag.hreflang.as_deref() {
        attributes.push(format!(r#"hreflang="{}""#, html_escape(hreflang)));
    }
    if let Some(media) = tag.media.as_deref() {
        attributes.push(format!(r#"media="{}""#, html_escape(media)));
    }
    if let Some(mime_type) = tag.mime_type.as_deref() {
        attributes.push(format!(r#"type="{}""#, html_escape(mime_type)));
    }
    if let Some(title) = tag.title.as_deref() {
        attributes.push(format!(r#"title="{}""#, html_escape(title)));
    }
    head.push_str(&format!(r#"<link {} />"#, attributes.join(" ")));
}

#[cfg(test)]
mod tests {
    use rustok_seo::{
        SeoAlternateLink, SeoDocument, SeoFieldSource, SeoImageAsset, SeoLinkTag, SeoMetaTag,
        SeoOpenGraph, SeoPageContext, SeoRobots, SeoRouteContext, SeoSchemaBlockKind,
        SeoStructuredDataBlock, SeoTwitterCard, SeoVerification, SeoVerificationTag,
    };

    use super::{render_head_html, robots_directives};

    #[test]
    fn renders_typed_robots_directives() {
        let directives = robots_directives(&SeoRobots {
            index: false,
            follow: false,
            noarchive: true,
            nosnippet: true,
            noimageindex: true,
            notranslate: true,
            max_snippet: Some(80),
            max_image_preview: Some("large".to_string()),
            max_video_preview: Some(120),
            custom: vec!["unavailable_after:25 Jun 2026 15:00:00 PST".to_string()],
        });

        assert_eq!(
            directives,
            vec![
                "noindex",
                "nofollow",
                "noarchive",
                "nosnippet",
                "noimageindex",
                "notranslate",
                "max-snippet:80",
                "max-image-preview:large",
                "max-video-preview:120",
                "unavailable_after:25 Jun 2026 15:00:00 PST",
            ]
        );
    }

    #[test]
    fn renders_rich_head_html_from_context() {
        let context = SeoPageContext {
            route: SeoRouteContext {
                canonical_url: "https://example.com/products/demo".to_string(),
                alternates: vec![SeoAlternateLink {
                    locale: "en-US".to_string(),
                    href: "https://example.com/en-US/products/demo".to_string(),
                    x_default: false,
                }],
                ..SeoRouteContext::default()
            },
            document: SeoDocument {
                title: "Demo".to_string(),
                description: Some("SEO description".to_string()),
                open_graph: Some(SeoOpenGraph {
                    title: Some("OG Demo".to_string()),
                    images: vec![SeoImageAsset {
                        url: "https://cdn.example.com/demo.png".to_string(),
                        alt: Some("Demo image".to_string()),
                        width: Some(1200),
                        height: Some(630),
                        mime_type: Some("image/png".to_string()),
                    }],
                    ..SeoOpenGraph::default()
                }),
                twitter: Some(SeoTwitterCard {
                    card: Some("summary_large_image".to_string()),
                    images: vec![SeoImageAsset {
                        url: "https://cdn.example.com/demo.png".to_string(),
                        ..SeoImageAsset::default()
                    }],
                    ..SeoTwitterCard::default()
                }),
                verification: Some(SeoVerification {
                    google: vec!["google-token".to_string()],
                    other: vec![SeoVerificationTag {
                        name: "p:domain_verify".to_string(),
                        value: "pinterest-token".to_string(),
                    }],
                    ..SeoVerification::default()
                }),
                meta_tags: vec![SeoMetaTag {
                    name: Some("author".to_string()),
                    property: None,
                    http_equiv: None,
                    content: "RusToK".to_string(),
                }],
                link_tags: vec![SeoLinkTag {
                    rel: "preload".to_string(),
                    href: "https://cdn.example.com/font.woff2".to_string(),
                    mime_type: Some("font/woff2".to_string()),
                    ..SeoLinkTag::default()
                }],
                structured_data_blocks: vec![SeoStructuredDataBlock {
                    id: Some("product".to_string()),
                    schema_kind: SeoSchemaBlockKind::Product,
                    schema_type: Some("Product".to_string()),
                    kind: Some("Product".to_string()),
                    source: SeoFieldSource::Fallback,
                    payload: serde_json::json!({
                        "@context": "https://schema.org",
                        "@type": "Product",
                        "name": "Demo",
                    })
                    .into(),
                }],
                ..SeoDocument::default()
            },
        };

        let head = render_head_html(&context);

        assert!(
            head.contains(r#"<link rel="canonical" href="https://example.com/products/demo" />"#)
        );
        assert!(head.contains(r#"<meta name="robots" content="index, follow" />"#));
        assert!(head.contains(r#"<link rel="alternate" hreflang="en-US" href="https://example.com/en-US/products/demo" />"#));
        assert!(head.contains(r#"<meta property="og:title" content="OG Demo" />"#));
        assert!(head.contains(r#"<meta name="twitter:card" content="summary_large_image" />"#));
        assert!(head.contains(r#"<meta name="google-site-verification" content="google-token" />"#));
        assert!(head.contains(r#"<meta name="author" content="RusToK" />"#));
        assert!(head.contains(
            r#"<link rel="preload" href="https://cdn.example.com/font.woff2" type="font/woff2" />"#
        ));
        assert!(head.contains(r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Product","name":"Demo"}</script>"#));
    }
}
