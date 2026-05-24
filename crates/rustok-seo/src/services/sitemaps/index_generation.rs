pub(super) fn render_sitemap_file(urls: &[String]) -> String {
    let body = urls
        .iter()
        .map(|url| format!("<url><loc>{}</loc></url>", xml_escape(url)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{body}</urlset>"#
    )
}

pub(super) fn render_sitemap_index(urls: &[String]) -> String {
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
