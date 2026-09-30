use super::SitemapUrlRecord;

pub(in super::super) fn render_sitemap_file(
    urls: &[SitemapUrlRecord],
    changefreq: &str,
    priority: &str,
    include_images: bool,
    include_hreflang: bool,
) -> String {
    let body = urls
        .iter()
        .map(|record| {
            let alternates = if include_hreflang {
                record
                    .alternates
                    .iter()
                    .map(|alternate| {
                        format!(
                            "<xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{}\"/>",
                            xml_escape(alternate.locale.as_str()),
                            xml_escape(alternate.route.as_str()),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                String::new()
            };
            let images = if include_images {
                record
                    .images
                    .iter()
                    .map(|image| {
                        let caption = image
                            .alt
                            .as_deref()
                            .map(|alt| format!("<image:caption>{}</image:caption>", xml_escape(alt)))
                            .unwrap_or_default();
                        format!(
                            "<image:image><image:loc>{}</image:loc>{}</image:image>",
                            xml_escape(image.url.as_str()),
                            caption,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                String::new()
            };
            format!(
                "<url><loc>{}</loc><changefreq>{}</changefreq><priority>{}</priority>{}{}</url>",
                xml_escape(record.url.as_str()),
                xml_escape(changefreq),
                xml_escape(priority),
                alternates,
                images,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let image_namespace = include_images
        .then_some(" xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\"")
        .unwrap_or_default();
    let hreflang_namespace = include_hreflang
        .then_some(" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\"")
        .unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"{image_namespace}{hreflang_namespace}>{body}</urlset>"
    )
}

pub(in super::super) fn render_sitemap_index(urls: &[String]) -> String {
    let body = urls
        .iter()
        .map(|url| format!("<sitemap><loc>{}</loc></sitemap>", xml_escape(url)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{body}</sitemapindex>"#
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::render_sitemap_file;
    use crate::services::sitemaps::SitemapUrlRecord;
    use rustok_seo_targets::{SeoTargetAlternateRoute, SeoTargetImageRecord};

    fn fixture() -> SitemapUrlRecord {
        SitemapUrlRecord {
            url: "https://example.com/en/catalog".to_string(),
            images: vec![SeoTargetImageRecord {
                media_asset_id: None,
                url: "https://cdn.example.com/catalog.jpg".to_string(),
                alt: Some("Catalog image".to_string()),
                width: None,
                height: None,
                mime_type: None,
            }],
            alternates: vec![SeoTargetAlternateRoute {
                locale: "de".to_string(),
                route: "https://example.com/de/catalog".to_string(),
            }],
        }
    }

    #[test]
    fn sitemap_extensions_follow_settings() {
        let record = fixture();
        let enabled = render_sitemap_file(&[record.clone()], "daily", "0.5", true, true);
        assert!(enabled.contains("xmlns:image="));
        assert!(enabled.contains("xmlns:xhtml="));
        assert!(enabled.contains("<image:loc>https://cdn.example.com/catalog.jpg</image:loc>"));
        assert!(enabled.contains("hreflang=\"de\""));

        let disabled = render_sitemap_file(&[record], "daily", "0.5", false, false);
        assert!(!disabled.contains("xmlns:image="));
        assert!(!disabled.contains("xmlns:xhtml="));
        assert!(!disabled.contains("<image:image>"));
        assert!(!disabled.contains("xhtml:link"));
    }
}
