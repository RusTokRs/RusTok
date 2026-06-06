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

    let mut alternates = route.alternates.clone();
    alternates.sort_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then_with(|| left.href.cmp(&right.href))
            .then_with(|| left.x_default.cmp(&right.x_default))
    });
    for alternate in alternates {
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
        let mut google_tokens = verification.google.clone();
        google_tokens.sort();
        for token in google_tokens {
            head.push_str(&format!(
                r#"<meta name="google-site-verification" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }

        let mut yandex_tokens = verification.yandex.clone();
        yandex_tokens.sort();
        for token in yandex_tokens {
            head.push_str(&format!(
                r#"<meta name="yandex-verification" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }

        let mut yahoo_tokens = verification.yahoo.clone();
        yahoo_tokens.sort();
        for token in yahoo_tokens {
            head.push_str(&format!(
                r#"<meta name="y_key" content="{}" />"#,
                html_escape(token.as_str())
            ));
        }

        let mut other_tokens = verification.other.clone();
        other_tokens.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.value.cmp(&right.value))
        });
        for token in other_tokens {
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

    let mut meta_tags = document.meta_tags.clone();
    meta_tags.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.property.cmp(&right.property))
            .then_with(|| left.http_equiv.cmp(&right.http_equiv))
            .then_with(|| left.content.cmp(&right.content))
    });
    for tag in &meta_tags {
        render_meta_tag(&mut head, tag);
    }

    let mut link_tags = document.link_tags.clone();
    link_tags.sort_by(|left, right| {
        left.rel
            .cmp(&right.rel)
            .then_with(|| left.href.cmp(&right.href))
            .then_with(|| left.hreflang.cmp(&right.hreflang))
            .then_with(|| left.media.cmp(&right.media))
            .then_with(|| left.mime_type.cmp(&right.mime_type))
            .then_with(|| left.title.cmp(&right.title))
    });
    for tag in &link_tags {
        render_link_tag(&mut head, tag);
    }

    let mut structured_data_blocks = document.structured_data_blocks.clone();
    structured_data_blocks.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.schema_kind.as_str().cmp(right.schema_kind.as_str()))
            .then_with(|| left.schema_type.cmp(&right.schema_type))
            .then_with(|| {
                serde_json::to_string(&left.payload.0)
                    .unwrap_or_default()
                    .cmp(&serde_json::to_string(&right.payload.0).unwrap_or_default())
            })
    });
    for block in &structured_data_blocks {
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
        SeoOpenGraph, SeoPageContext, SeoPagination, SeoRobots, SeoRouteContext,
        SeoSchemaBlockKind, SeoStructuredDataBlock, SeoTwitterCard, SeoVerification,
        SeoVerificationTag,
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

    #[test]
    fn renders_complex_head_combinations_with_deterministic_ordering() {
        let context = SeoPageContext {
            route: SeoRouteContext {
                canonical_url: "https://example.com/catalog".to_string(),
                alternates: vec![
                    SeoAlternateLink {
                        locale: "fr-FR".to_string(),
                        href: "https://example.com/fr/catalog".to_string(),
                        x_default: false,
                    },
                    SeoAlternateLink {
                        locale: "en-US".to_string(),
                        href: "https://example.com/en/catalog".to_string(),
                        x_default: true,
                    },
                ],
                ..SeoRouteContext::default()
            },
            document: SeoDocument {
                description: Some("Catalog metadata".to_string()),
                robots: SeoRobots {
                    index: false,
                    follow: true,
                    ..SeoRobots::default()
                },
                verification: Some(SeoVerification {
                    google: vec!["z-token".to_string(), "a-token".to_string()],
                    other: vec![
                        SeoVerificationTag {
                            name: "zeta".to_string(),
                            value: "2".to_string(),
                        },
                        SeoVerificationTag {
                            name: "alpha".to_string(),
                            value: "1".to_string(),
                        },
                    ],
                    ..SeoVerification::default()
                }),
                pagination: Some(SeoPagination {
                    prev_url: Some("https://example.com/catalog?page=1".to_string()),
                    next_url: Some("https://example.com/catalog?page=3".to_string()),
                }),
                meta_tags: vec![
                    SeoMetaTag {
                        name: Some("viewport".to_string()),
                        property: None,
                        http_equiv: None,
                        content: "width=device-width".to_string(),
                    },
                    SeoMetaTag {
                        name: Some("author".to_string()),
                        property: None,
                        http_equiv: None,
                        content: "RusToK".to_string(),
                    },
                ],
                link_tags: vec![
                    SeoLinkTag {
                        rel: "preload".to_string(),
                        href: "https://cdn.example.com/z.js".to_string(),
                        ..SeoLinkTag::default()
                    },
                    SeoLinkTag {
                        rel: "dns-prefetch".to_string(),
                        href: "https://cdn.example.com".to_string(),
                        ..SeoLinkTag::default()
                    },
                ],
                structured_data_blocks: vec![
                    SeoStructuredDataBlock {
                        id: Some("z-article".to_string()),
                        schema_kind: SeoSchemaBlockKind::Article,
                        schema_type: Some("Article".to_string()),
                        kind: Some("Article".to_string()),
                        source: SeoFieldSource::Explicit,
                        payload: serde_json::json!({"@type":"Article","headline":"Z"}).into(),
                    },
                    SeoStructuredDataBlock {
                        id: Some("a-product".to_string()),
                        schema_kind: SeoSchemaBlockKind::Product,
                        schema_type: Some("Product".to_string()),
                        kind: Some("Product".to_string()),
                        source: SeoFieldSource::Generated,
                        payload: serde_json::json!({"@type":"Product","name":"A"}).into(),
                    },
                ],
                ..SeoDocument::default()
            },
        };

        let head = render_head_html(&context);

        let canonical_position = head
            .find(r#"<link rel="canonical" href="https://example.com/catalog" />"#)
            .expect("canonical tag should exist");
        let alternate_en_position = head
            .find(r#"<link rel="alternate" hreflang="en-US" href="https://example.com/en/catalog" />"#)
            .expect("en alternate should exist");
        let alternate_fr_position = head
            .find(r#"<link rel="alternate" hreflang="fr-FR" href="https://example.com/fr/catalog" />"#)
            .expect("fr alternate should exist");
        assert!(canonical_position < alternate_en_position);
        assert!(alternate_en_position < alternate_fr_position);

        let google_a_position = head
            .find(r#"<meta name="google-site-verification" content="a-token" />"#)
            .expect("a-token should exist");
        let google_z_position = head
            .find(r#"<meta name="google-site-verification" content="z-token" />"#)
            .expect("z-token should exist");
        assert!(google_a_position < google_z_position);

        let meta_author_position = head
            .find(r#"<meta name="author" content="RusToK" />"#)
            .expect("author meta should exist");
        let meta_viewport_position = head
            .find(r#"<meta name="viewport" content="width=device-width" />"#)
            .expect("viewport meta should exist");
        assert!(meta_author_position < meta_viewport_position);

        let link_dns_prefetch_position = head
            .find(r#"<link rel="dns-prefetch" href="https://cdn.example.com" />"#)
            .expect("dns-prefetch tag should exist");
        let link_preload_position = head
            .find(r#"<link rel="preload" href="https://cdn.example.com/z.js" />"#)
            .expect("preload tag should exist");
        assert!(link_dns_prefetch_position < link_preload_position);

        let script_product_position = head
            .find(r#"<script type="application/ld+json">{"@type":"Product","name":"A"}</script>"#)
            .expect("product json-ld should exist");
        let script_article_position = head
            .find(
                r#"<script type="application/ld+json">{"@type":"Article","headline":"Z"}</script>"#,
            )
            .expect("article json-ld should exist");
        assert!(script_product_position < script_article_position);

        assert!(head.contains(r#"<meta name="robots" content="noindex, follow" />"#));
    }
}
