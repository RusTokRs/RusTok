use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_content::entities::node::ContentStatus;
use rustok_core::SecurityContext;
use rustok_seo_targets::{
    builtin_slug, SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord,
    SeoSitemapCandidateRecord, SeoTargetAlternateRoute, SeoTargetBulkListRequest,
    SeoTargetCapabilities, SeoTargetLoadRequest, SeoTargetLoadScope, SeoTargetOpenGraphRecord,
    SeoTargetProvider, SeoTargetRouteResolveRequest, SeoTargetRuntimeContext,
    SeoTargetSitemapRequest, SeoTargetSlug, SeoTemplateFieldMap,
};
use serde_json::json;
use url::Url;

use crate::{ListPagesFilter, PageListItem, PageResponse, PageService, PageTranslationResponse};

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
        let page = service
            .get_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await
            .ok();
        let Some(page) = page else {
            return Ok(None);
        };

        if matches!(request.scope, SeoTargetLoadScope::PublicRoute)
            && (page.status != ContentStatus::Published
                || !channel_visible(page.channel_slugs.as_slice(), request.channel_slug))
        {
            return Ok(None);
        }

        Ok(Some(map_page_response(page, request.locale)))
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

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u64;
        let mut summaries = Vec::new();

        loop {
            let (items, total) = service
                .list(
                    request.tenant_id,
                    SecurityContext::system(),
                    ListPagesFilter {
                        status: Some(ContentStatus::Published),
                        template: None,
                        locale: Some(request.locale.to_string()),
                        page: page_number,
                        per_page: BULK_FETCH_SIZE,
                    },
                )
                .await?;
            if items.is_empty() {
                break;
            }

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

            if page_number.saturating_mul(BULK_FETCH_SIZE) >= total {
                break;
            }
            page_number += 1;
        }

        Ok(summaries)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let service = PageService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u64;
        let mut candidates = Vec::new();

        loop {
            let (items, total) = service
                .list_public_visible(
                    request.tenant_id,
                    ListPagesFilter {
                        status: Some(ContentStatus::Published),
                        template: None,
                        locale: Some(request.default_locale.to_string()),
                        page: page_number,
                        per_page: BULK_FETCH_SIZE,
                    },
                    None,
                )
                .await?;
            if items.is_empty() {
                break;
            }

            for item in items {
                if let Some(candidate) = load_page_sitemap_candidate(
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

            if page_number.saturating_mul(BULK_FETCH_SIZE) >= total {
                break;
            }
            page_number += 1;
        }

        Ok(candidates)
    }
}

async fn load_page_summary(
    service: &PageService,
    tenant_id: uuid::Uuid,
    locale: &str,
    default_locale: &str,
    item: PageListItem,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let page = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(page) = page else {
        return Ok(None);
    };
    let mapped = map_page_response(page, locale);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_page_sitemap_candidate(
    service: &PageService,
    tenant_id: uuid::Uuid,
    default_locale: &str,
    item: PageListItem,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let page = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            default_locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(page) = page else {
        return Ok(None);
    };
    let mapped = map_page_response(page, default_locale);
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
    }))
}

fn map_page_response(page: PageResponse, requested_locale: &str) -> SeoLoadedTargetRecord {
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
            page.body
                .as_ref()
                .and_then(|body| summarize_text(body.content.as_str()))
        })
        .or_else(|| translation.title.as_deref().and_then(summarize_text));
    let canonical_route = page_route_for_slug(translation.slug.as_deref());
    let structured_name = translation.title.clone().unwrap_or_else(|| title.clone());
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("locale", effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());
    if let Some(slug) = translation.slug.as_deref() {
        template_fields.insert("slug", slug);
    }

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
                    route: page_route_for_slug(Some(slug.as_str())),
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
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "WebPage",
            "name": structured_name,
            "description": description,
            "inLanguage": effective_locale,
        }),
        fallback_source: "pages".to_string(),
        template_fields,
    }
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

fn page_route_for_slug(slug: Option<&str>) -> String {
    match slug.filter(|value| !value.trim().is_empty()) {
        Some(slug) => format!("/modules/pages?slug={slug}"),
        None => "/modules/pages".to_string(),
    }
}

fn parse_page_route(route: &str) -> AnyResult<Option<String>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    if !matches_module_path(&parsed, "pages") {
        return Ok(None);
    }
    Ok(parsed
        .query_pairs()
        .find(|(key, _)| key == "slug")
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty()))
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

fn matches_module_path(parsed: &Url, module: &str) -> bool {
    let mut segments = parsed
        .path_segments()
        .map(|items| items.filter(|item| !item.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();
    if segments.len() > 2
        && segments
            .first()
            .and_then(|item| rustok_core::normalize_locale_tag(item))
            .is_some()
        && segments.get(1) == Some(&"modules")
    {
        segments.remove(0);
    }

    segments.as_slice() == ["modules", module]
}

fn summarize_text(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(rustok_core::truncate(normalized.as_str(), 180))
    }
}
