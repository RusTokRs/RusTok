use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;
use rustok_seo_targets::SeoTargetImageRecord;
use rustok_seo_targets::{
    SeoBulkSummaryPage, SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord,
    SeoSitemapCandidateRecord, SeoTargetAlternateRoute, SeoTargetBulkPageRequest,
    SeoTargetCapabilities, SeoTargetLoadRequest, SeoTargetLoadScope, SeoTargetOpenGraphRecord,
    SeoTargetProvider, SeoTargetRouteResolveRequest, SeoTargetRuntimeContext,
    SeoTargetSitemapRequest, SeoTargetSlug, SeoTemplateFieldMap, builtin_slug,
    populate_image_template_fields, schema,
};
use url::Url;

use crate::search_projection::strip_html_tags;
use crate::{
    PageBuilderArtifactService, PageListItem, PageResponse, PageService, PageTranslationResponse,
    PagesError,
};

const BULK_FETCH_SIZE: u64 = 48;

#[derive(Clone, Default)]
pub struct PagesSeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for PagesSeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::PAGE).expect("builtin SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Page"
    }

    fn owner_module_slug(&self) -> &'static str {
        "pages"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let page = load_page_if_present(
            &service,
            request.tenant_id,
            request.target_id,
            request.locale,
            request.default_locale,
        )
        .await?;
        let Some(page) = page else {
            return Ok(None);
        };

        if matches!(request.scope, SeoTargetLoadScope::PublicRoute)
            && (page.status != ContentStatus::Published
                || !channel_visible(page.channel_slugs.as_slice(), request.channel_slug))
        {
            return Ok(None);
        }

        let landing_markup = load_published_landing_markup(
            runtime,
            request.tenant_id,
            &page,
            request.locale,
            request.default_locale,
            landing_channel(
                request.scope,
                page.channel_slugs.as_slice(),
                request.channel_slug,
            ),
        )
        .await?;

        Ok(Some(map_page_response(
            page,
            request.locale,
            landing_markup.as_deref(),
        )))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(slug) = parse_page_route(request.route)? else {
            return Ok(None);
        };
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let page = service
            .get_by_slug_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.locale,
                slug.as_str(),
                Some(request.default_locale),
            )
            .await?;

        Ok(page
            .filter(|page| channel_visible(page.channel_slugs.as_slice(), request.channel_slug))
            .map(|page| SeoRouteMatchRecord {
                target_kind: self.slug(),
                target_id: page.id,
            }))
    }

    async fn list_bulk_summaries_page(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkPageRequest<'_>,
    ) -> AnyResult<SeoBulkSummaryPage> {
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let (items, next_cursor) = service
            .scan_published_pages(
                request.tenant_id,
                request.locale,
                request.after,
                request.limit,
            )
            .await?;

        let mut summaries = Vec::with_capacity(items.len());
        for item in items {
            if let Some(summary) = load_page_summary(
                &service,
                request.tenant_id,
                request.locale,
                request.default_locale,
                item,
            )
            .await?
            {
                summaries.push(summary);
            }
        }
        Ok(SeoBulkSummaryPage {
            items: summaries,
            next_cursor,
        })
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut after: Option<String> = None;
        let mut candidates = Vec::new();

        loop {
            let (items, next_cursor) = service
                .scan_public_published_pages(
                    request.tenant_id,
                    request.default_locale,
                    after.as_deref(),
                    BULK_FETCH_SIZE,
                )
                .await?;

            for item in items {
                if let Some(candidate) = load_page_sitemap_candidate(
                    runtime,
                    &service,
                    request.tenant_id,
                    request.default_locale,
                    item,
                )
                .await?
                {
                    candidates.push(candidate);
                }
            }

            match next_cursor {
                Some(cursor) => after = Some(cursor),
                None => break,
            }
        }

        Ok(candidates)
    }
}

/// Loads a page for SEO. A missing page is `None`; every other error propagates.
async fn load_page_if_present(
    service: &PageService,
    tenant_id: uuid::Uuid,
    page_id: uuid::Uuid,
    locale: &str,
    default_locale: &str,
) -> AnyResult<Option<PageResponse>> {
    match service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            page_id,
            locale,
            Some(default_locale),
        )
        .await
    {
        Ok(page) => Ok(Some(page)),
        Err(PagesError::PageNotFound(_)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn load_page_summary(
    service: &PageService,
    tenant_id: uuid::Uuid,
    locale: &str,
    default_locale: &str,
    item: PageListItem,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let page = load_page_if_present(service, tenant_id, item.id, locale, default_locale).await?;
    let Some(page) = page else {
        return Ok(None);
    };
    // A bulk summary carries the label and the route only; neither depends on page content.
    let mapped = map_page_response(page, locale, None);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_page_sitemap_candidate(
    runtime: &SeoTargetRuntimeContext,
    service: &PageService,
    tenant_id: uuid::Uuid,
    default_locale: &str,
    item: PageListItem,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let page =
        load_page_if_present(service, tenant_id, item.id, default_locale, default_locale).await?;
    let Some(page) = page else {
        return Ok(None);
    };
    // Sitemap candidates come from `scan_public_published_pages`: pages without a channel
    // restriction, so the landing is the one a visitor without a channel gets.
    let landing_markup = load_published_landing_markup(
        runtime,
        tenant_id,
        &page,
        default_locale,
        default_locale,
        None,
    )
    .await?;
    // A published page without a published landing has no storefront route. Listing its URL
    // would send crawlers to "not found".
    let Some(landing_markup) = landing_markup else {
        return Ok(None);
    };
    let mapped = map_page_response(page, default_locale, Some(landing_markup.as_str()));
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
        images: mapped.open_graph.images,
        alternates: mapped.alternates,
    }))
}

/// Channel on which the landing of a page is read for an SEO record.
///
/// A public route reads on the request channel: the record describes what that visitor gets.
/// An authoring view is not a storefront and carries no channel, while the artifact owner
/// admits a channel-restricted page only on one of its channels. The landing is the same on
/// every channel that admits the page, so an authoring view reads it on the request channel
/// when that channel admits the page and on the first channel of the page otherwise.
fn landing_channel<'a>(
    scope: SeoTargetLoadScope,
    page_channel_slugs: &'a [String],
    request_channel: Option<&'a str>,
) -> Option<&'a str> {
    match scope {
        SeoTargetLoadScope::PublicRoute => request_channel,
        SeoTargetLoadScope::Authoring => {
            if channel_visible(page_channel_slugs, request_channel) {
                request_channel
            } else {
                page_channel_slugs.first().map(String::as_str)
            }
        }
    }
}

/// Body markup of the landing a visitor of `channel_slug` gets for this page, if any.
///
/// SEO values that describe page content (the fallback description and the primary image) are
/// derived from this markup only. The page body is the authoring document: for a published
/// page it can hold unpublished edits, and the publish sanitizer has not processed it. Page
/// status, channel visibility and the locale binding are decided by the artifact owner.
async fn load_published_landing_markup(
    runtime: &SeoTargetRuntimeContext,
    tenant_id: uuid::Uuid,
    page: &PageResponse,
    requested_locale: &str,
    default_locale: &str,
    channel_slug: Option<&str>,
) -> AnyResult<Option<String>> {
    let locale = page
        .effective_locale
        .as_deref()
        .or(page.requested_locale.as_deref())
        .unwrap_or(requested_locale);
    let landing = PageBuilderArtifactService::new(runtime.db.clone())
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            page.id,
            locale,
            Some(default_locale),
            channel_slug,
        )
        .await?;
    Ok(landing.map(|landing| landing.body_html))
}

/// Maps a page to its SEO record. `landing_markup` is the body markup of the published landing
/// (see [`load_published_landing_markup`]); without it the record has no content-derived
/// description and no image. Nothing here reads `page.body`.
fn map_page_response(
    page: PageResponse,
    requested_locale: &str,
    landing_markup: Option<&str>,
) -> SeoLoadedTargetRecord {
    let effective_locale = page
        .effective_locale
        .clone()
        .or_else(|| page.requested_locale.clone())
        .unwrap_or_else(|| requested_locale.to_string());
    let translation = page
        .translation
        .clone()
        .or_else(|| page.translations.first().cloned())
        .unwrap_or_else(fallback_page_translation);
    let title = translation
        .meta_title
        .clone()
        .or_else(|| translation.title.clone())
        .unwrap_or_else(|| "Untitled page".to_string());
    let description = translation
        .meta_description
        .clone()
        .or_else(|| {
            landing_markup.and_then(|markup| summarize_text(strip_html_tags(markup).as_str()))
        })
        .or_else(|| translation.title.as_deref().and_then(summarize_text));
    let primary_image =
        landing_markup.and_then(|markup| landing_primary_image(markup, title.as_str()));
    let open_graph_images = primary_image.clone().into_iter().collect::<Vec<_>>();
    let canonical_route = page_route_for_slug(&effective_locale, translation.slug.as_deref());
    let structured_name = translation.title.clone().unwrap_or_else(|| title.clone());
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("locale", effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());
    if let Some(slug) = translation.slug.as_deref() {
        template_fields.insert("slug", slug);
    }
    populate_image_template_fields(&mut template_fields, open_graph_images.as_slice());

    SeoLoadedTargetRecord {
        target_kind: SeoTargetSlug::new(builtin_slug::PAGE)
            .expect("builtin SEO target slug must stay valid"),
        target_id: page.id,
        requested_locale: page.requested_locale.clone(),
        effective_locale: effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_route: canonical_route.clone(),
        alternates: page
            .translations
            .iter()
            .filter_map(|item| {
                item.slug.as_ref().map(|slug| SeoTargetAlternateRoute {
                    locale: item.locale.clone(),
                    route: page_route_for_slug(&item.locale, Some(slug.as_str())),
                })
            })
            .collect(),
        open_graph: SeoTargetOpenGraphRecord {
            title: Some(title.clone()),
            description: description.clone(),
            kind: Some("website".to_string()),
            site_name: None,
            url: None,
            locale: Some(effective_locale.clone()),
            images: open_graph_images,
        },
        structured_data: schema::web_page_with_image(
            structured_name.as_str(),
            description.as_deref(),
            primary_image.as_ref(),
            effective_locale.as_str(),
        ),
        fallback_source: "pages".to_string(),
        template_fields,
    }
}

fn landing_primary_image(markup: &str, fallback_alt: &str) -> Option<SeoTargetImageRecord> {
    let (url, alt) = first_image_in_markup(markup)?;
    let alt =
        normalize_image_text(alt).or_else(|| normalize_image_text(Some(fallback_alt.to_string())));
    SeoTargetImageRecord::from_parts(url, alt, None, None, None)
}

/// First `<img>` of published landing markup: its `src` and, when present, its `alt`.
///
/// The markup is the stored landing body, which the artifact owner wrote with escaped text and
/// quoted attribute values. The scan is quote-aware and reads tags only; anything it cannot
/// read as a complete tag ends the scan without a result, so malformed markup yields no image
/// rather than a guessed one. `data:` sources are skipped: they are not shareable image URLs.
fn first_image_in_markup(markup: &str) -> Option<(String, Option<String>)> {
    let bytes = markup.as_bytes();
    let mut cursor = 0;
    while let Some(offset) = markup[cursor..].find('<') {
        let start = cursor + offset + 1;
        if markup[start..].starts_with("!--") {
            cursor = start + markup[start..].find("-->")? + 3;
            continue;
        }
        if !bytes.get(start).is_some_and(u8::is_ascii_alphabetic) {
            cursor = start;
            continue;
        }
        let name_end = start
            + markup[start..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
                .unwrap_or(markup.len() - start);
        let is_image = markup[start..name_end].eq_ignore_ascii_case("img");
        let (attributes, tag_end) = read_tag_attributes(markup, name_end)?;
        cursor = tag_end;
        if !is_image {
            continue;
        }
        let value_of = |wanted: &str| {
            attributes
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
                .map(|(_, value)| decode_attribute_value(value).trim().to_string())
                .filter(|value| !value.is_empty())
        };
        if let Some(src) = value_of("src")
            && !src
                .get(..5)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
        {
            return Some((src, value_of("alt")));
        }
    }
    None
}

/// Reads the attributes of one start tag, beginning right after the tag name. Returns the
/// attributes and the offset just past the closing `>`, or `None` for an unterminated tag.
fn read_tag_attributes(markup: &str, mut cursor: usize) -> Option<(Vec<(&str, &str)>, usize)> {
    let bytes = markup.as_bytes();
    let mut attributes = Vec::new();
    loop {
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'/')
        {
            cursor += 1;
        }
        if *bytes.get(cursor)? == b'>' {
            return Some((attributes, cursor + 1));
        }
        let name_start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'>' | b'/'))
        {
            cursor += 1;
        }
        let name = &markup[name_start..cursor];
        if name.is_empty() {
            // A stray `=`: not an attribute this scan understands.
            return None;
        }
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            attributes.push((name, ""));
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let value = match *bytes.get(cursor)? {
            quote @ (b'"' | b'\'') => {
                let value_start = cursor + 1;
                let length = markup[value_start..].find(quote as char)?;
                cursor = value_start + length + 1;
                &markup[value_start..value_start + length]
            }
            _ => {
                let value_start = cursor;
                while bytes
                    .get(cursor)
                    .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'>')
                {
                    cursor += 1;
                }
                &markup[value_start..cursor]
            }
        };
        attributes.push((name, value));
    }
}

fn decode_attribute_value(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

fn normalize_image_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn fallback_page_translation() -> PageTranslationResponse {
    PageTranslationResponse {
        locale: "en".to_string(),
        title: Some("Untitled page".to_string()),
        slug: None,
        meta_title: None,
        meta_description: None,
    }
}

fn page_route_for_slug(locale: &str, slug: Option<&str>) -> String {
    match slug.filter(|value| !value.trim().is_empty()) {
        Some(slug) => format!("/{locale}/modules/pages?slug={slug}"),
        None => format!("/{locale}/modules/pages"),
    }
}

fn parse_page_route(route: &str) -> AnyResult<Option<String>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    let mut segments = parsed
        .path_segments()
        .map(|items| items.filter(|item| !item.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();

    if segments.len() >= 2
        && segments
            .first()
            .and_then(|item| rustok_api::normalize_locale_tag(item))
            .is_some()
    {
        segments.remove(0);
    }

    if segments.as_slice() == ["modules", "pages"] {
        return Ok(parsed
            .query_pairs()
            .find(|(key, _)| key == "slug")
            .map(|(_, value)| value.to_string())
            .filter(|value| !value.trim().is_empty()));
    }

    if segments.len() == 2 && segments[0] == "pages" {
        let slug = segments[1].trim();
        if !slug.is_empty() {
            return Ok(Some(slug.to_string()));
        }
    }

    Ok(None)
}

fn channel_visible(channel_slugs: &[String], requested_channel: Option<&str>) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(requested_channel) = normalize_channel_slug(requested_channel) else {
        return false;
    };
    channel_slugs
        .iter()
        .any(|slug| normalize_channel_slug(Some(slug.as_str())) == Some(requested_channel.clone()))
}

fn normalize_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

fn summarize_text(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(rustok_core::truncate(normalized.as_str(), 180))
    }
}

#[cfg(test)]
mod landing_markup_tests {
    use super::{first_image_in_markup, landing_primary_image, strip_html_tags, summarize_text};

    fn image(src: &str, alt: Option<&str>) -> Option<(String, Option<String>)> {
        Some((src.to_string(), alt.map(str::to_string)))
    }

    #[test]
    fn finds_the_first_image_with_its_alt_text() {
        let markup = r#"<section class="hero"><h1>Hi</h1><img class="a" src="/img/hero.webp" alt="Hero &amp; more"><img src="/img/second.png"></section>"#;
        assert_eq!(
            first_image_in_markup(markup),
            image("/img/hero.webp", Some("Hero & more"))
        );
    }

    #[test]
    fn reads_single_quoted_unquoted_and_self_closing_tags() {
        assert_eq!(
            first_image_in_markup("<IMG SRC='/a.png' ALT='A'/>"),
            image("/a.png", Some("A"))
        );
        assert_eq!(
            first_image_in_markup("<img src=/b.png>"),
            image("/b.png", None)
        );
        assert_eq!(
            first_image_in_markup("<img alt = \"B\" src = \"/c.png\" />"),
            image("/c.png", Some("B"))
        );
    }

    #[test]
    fn a_tag_closing_character_inside_a_quoted_value_does_not_end_the_tag() {
        let markup = r#"<a title="a > b" href="/x">x</a><img alt="1 > 0" src="/d.png">"#;
        assert_eq!(
            first_image_in_markup(markup),
            image("/d.png", Some("1 > 0"))
        );
    }

    #[test]
    fn markup_shown_as_text_or_inside_an_attribute_is_not_an_image() {
        assert_eq!(
            first_image_in_markup("<p>&lt;img src=&quot;/text.png&quot;&gt;</p>"),
            None
        );
        assert_eq!(
            first_image_in_markup(r#"<p title="<img src='/attr.png'>">x</p>"#),
            None
        );
        assert_eq!(
            first_image_in_markup("<!-- <img src=\"/comment.png\"> --><p>x</p>"),
            None
        );
        assert_eq!(first_image_in_markup("<imgx src=\"/other.png\">"), None);
        assert_eq!(first_image_in_markup("<picture-img src=\"/o.png\">"), None);
    }

    #[test]
    fn images_without_a_shareable_source_are_skipped() {
        let markup = r#"<img alt="none"><img src="  "><img src="data:image/png;base64,AAAA"><img src="https://cdn.example.com/e.png">"#;
        assert_eq!(
            first_image_in_markup(markup),
            image("https://cdn.example.com/e.png", None)
        );
        assert_eq!(
            first_image_in_markup("<img src=\"DATA:image/png,x\">"),
            None
        );
    }

    #[test]
    fn incomplete_markup_yields_no_image() {
        assert_eq!(first_image_in_markup("<img src=\"/f.png"), None);
        assert_eq!(first_image_in_markup("<img src=\"/f.png\""), None);
        assert_eq!(first_image_in_markup("<p class=\"a\"><img"), None);
        assert_eq!(
            first_image_in_markup("<!-- open <img src=\"/g.png\">"),
            None
        );
        assert_eq!(first_image_in_markup("<p =x><img src=\"/h.png\">"), None);
        assert_eq!(first_image_in_markup(""), None);
        assert_eq!(first_image_in_markup("a < b and 2 > 1"), None);
        assert_eq!(first_image_in_markup("trailing <"), None);
    }

    #[test]
    fn non_ascii_text_around_tags_is_handled() {
        assert_eq!(
            first_image_in_markup("<p>Привет — мир</p><img src=\"/й.png\" alt=\"Фото\">"),
            image("/й.png", Some("Фото"))
        );
    }

    #[test]
    fn an_authoring_view_reads_the_landing_on_a_channel_that_admits_the_page() {
        let restricted = vec!["web".to_string(), "mobile".to_string()];
        let unrestricted: Vec<String> = Vec::new();

        // The SEO editor passes no channel: a restricted page is read on its first channel.
        assert_eq!(
            landing_channel(SeoTargetLoadScope::Authoring, &restricted, None),
            Some("web")
        );
        assert_eq!(
            landing_channel(SeoTargetLoadScope::Authoring, &restricted, Some("pos")),
            Some("web")
        );
        // A request channel that admits the page is kept.
        assert_eq!(
            landing_channel(SeoTargetLoadScope::Authoring, &restricted, Some("mobile")),
            Some("mobile")
        );
        assert_eq!(
            landing_channel(SeoTargetLoadScope::Authoring, &unrestricted, None),
            None
        );
        assert_eq!(
            landing_channel(SeoTargetLoadScope::Authoring, &unrestricted, Some("pos")),
            Some("pos")
        );
    }

    #[test]
    fn a_public_route_reads_the_landing_on_the_request_channel_only() {
        let restricted = vec!["web".to_string()];

        for request_channel in [None, Some("web"), Some("pos")] {
            assert_eq!(
                landing_channel(
                    SeoTargetLoadScope::PublicRoute,
                    &restricted,
                    request_channel
                ),
                request_channel
            );
        }
    }

    #[test]
    fn primary_image_falls_back_to_the_page_title_for_alt_text() {
        let with_alt =
            landing_primary_image("<img src=\"/a.png\" alt=\"Team\">", "About").expect("image");
        assert_eq!(with_alt.url, "/a.png");
        assert_eq!(with_alt.alt.as_deref(), Some("Team"));

        let without_alt = landing_primary_image("<img src=\"/a.png\">", "About").expect("image");
        assert_eq!(without_alt.alt.as_deref(), Some("About"));

        assert!(landing_primary_image("<p>No image</p>", "About").is_none());
    }

    /// The fallback description is the visible text of the landing, never its markup.
    #[test]
    fn description_text_is_the_visible_text_of_the_landing() {
        let markup = "<section><h1>About us</h1><p>We build &amp; ship.</p></section>";
        assert_eq!(
            summarize_text(strip_html_tags(markup).as_str()).as_deref(),
            Some("About us We build & ship.")
        );
        assert_eq!(
            summarize_text(strip_html_tags("<div></div>").as_str()),
            None
        );
    }
}
