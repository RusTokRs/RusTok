use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_core::SecurityContext;
use rustok_seo_targets::{
    builtin_slug, SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord,
    SeoSitemapCandidateRecord, SeoTargetAlternateRoute, SeoTargetBulkListRequest,
    SeoTargetCapabilities, SeoTargetImageRecord, SeoTargetLoadRequest, SeoTargetLoadScope,
    SeoTargetOpenGraphRecord, SeoTargetProvider, SeoTargetRouteResolveRequest,
    SeoTargetRuntimeContext, SeoTargetSitemapRequest, SeoTargetSlug, SeoTemplateFieldMap,
};
use serde_json::json;
use url::Url;

use crate::state_machine::BlogPostStatus;
use crate::{PostListQuery, PostResponse, PostService, PostSummary};

const BULK_FETCH_SIZE: u32 = 48;

#[derive(Clone, Default)]
pub struct BlogSeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for BlogSeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::BLOG_POST)
            .expect("builtin SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Blog post"
    }

    fn owner_module_slug(&self) -> &'static str {
        "blog"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let service = PostService::new(runtime.db.clone(), runtime.event_bus.clone());
        let post = service
            .get_post_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await
            .ok();
        let Some(post) = post else {
            return Ok(None);
        };

        if matches!(request.scope, SeoTargetLoadScope::PublicRoute)
            && (post.status != BlogPostStatus::Published
                || !channel_visible(post.channel_slugs.as_slice(), request.channel_slug))
        {
            return Ok(None);
        }

        Ok(Some(map_post_response(post)))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(slug) = parse_blog_route(request.route)? else {
            return Ok(None);
        };
        let service = PostService::new(runtime.db.clone(), runtime.event_bus.clone());
        let post = service
            .get_post_by_slug_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.locale,
                slug.as_str(),
                Some(request.default_locale),
            )
            .await?;

        Ok(post
            .filter(|post| {
                post.status == BlogPostStatus::Published
                    && channel_visible(post.channel_slugs.as_slice(), request.channel_slug)
            })
            .map(|post| SeoRouteMatchRecord {
                target_kind: self.slug(),
                target_id: post.id,
            }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let service = PostService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u32;
        let mut summaries = Vec::new();

        loop {
            let page = service
                .list_posts_with_locale_fallback(
                    request.tenant_id,
                    SecurityContext::system(),
                    PostListQuery {
                        status: Some(BlogPostStatus::Published),
                        category_id: None,
                        tag: None,
                        author_id: None,
                        search: None,
                        locale: Some(request.locale.to_string()),
                        page: Some(page_number),
                        per_page: Some(BULK_FETCH_SIZE),
                        sort_by: Some("published_at".to_string()),
                        sort_order: Some("desc".to_string()),
                    },
                    Some(request.default_locale),
                )
                .await?;
            if page.items.is_empty() {
                break;
            }

            for item in page.items {
                if let Some(summary) = load_post_summary(
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

            if page_number >= page.total_pages.max(1) {
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
        let service = PostService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u32;
        let mut candidates = Vec::new();

        loop {
            let page = service
                .list_public_visible_with_locale_fallback(
                    request.tenant_id,
                    PostListQuery {
                        status: Some(BlogPostStatus::Published),
                        category_id: None,
                        tag: None,
                        author_id: None,
                        search: None,
                        locale: Some(request.default_locale.to_string()),
                        page: Some(page_number),
                        per_page: Some(BULK_FETCH_SIZE),
                        sort_by: Some("published_at".to_string()),
                        sort_order: Some("desc".to_string()),
                    },
                    Some(request.default_locale),
                    None,
                )
                .await?;
            if page.items.is_empty() {
                break;
            }

            for item in page.items {
                if let Some(candidate) = load_post_sitemap_candidate(
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

            if page_number >= page.total_pages.max(1) {
                break;
            }
            page_number += 1;
        }

        Ok(candidates)
    }
}

async fn load_post_summary(
    service: &PostService,
    tenant_id: uuid::Uuid,
    locale: &str,
    default_locale: &str,
    item: PostSummary,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let post = service
        .get_post_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(post) = post else {
        return Ok(None);
    };
    let mapped = map_post_response(post);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_post_sitemap_candidate(
    service: &PostService,
    tenant_id: uuid::Uuid,
    default_locale: &str,
    item: PostSummary,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let post = service
        .get_post_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            default_locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(post) = post else {
        return Ok(None);
    };
    let mapped = map_post_response(post);
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
    }))
}

fn map_post_response(post: PostResponse) -> SeoLoadedTargetRecord {
    let title = post.seo_title.clone().unwrap_or_else(|| post.title.clone());
    let description = post
        .seo_description
        .clone()
        .or_else(|| post.excerpt.clone())
        .or_else(|| summarize_text(post.body.as_str()))
        .or_else(|| summarize_text(post.title.as_str()));
    let canonical_route = format!("/modules/blog?slug={}", post.slug);
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("slug", post.slug.clone());
    template_fields.insert("locale", post.effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());

    SeoLoadedTargetRecord {
        target_kind: SeoTargetSlug::new(builtin_slug::BLOG_POST)
            .expect("builtin SEO target slug must stay valid"),
        target_id: post.id,
        requested_locale: Some(post.requested_locale.clone()),
        effective_locale: post.effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_route: canonical_route.clone(),
        alternates: post
            .available_locales
            .iter()
            .map(|locale| SeoTargetAlternateRoute {
                locale: locale.clone(),
                route: canonical_route.clone(),
            })
            .collect(),
        open_graph: SeoTargetOpenGraphRecord {
            title: Some(title.clone()),
            description: description.clone(),
            kind: Some("article".to_string()),
            site_name: None,
            url: None,
            locale: Some(post.effective_locale.clone()),
            images: post
                .featured_image_url
                .clone()
                .map(|url| {
                    vec![SeoTargetImageRecord {
                        url,
                        alt: Some(post.title.clone()),
                        width: None,
                        height: None,
                        mime_type: None,
                    }]
                })
                .unwrap_or_default(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "BlogPosting",
            "headline": post.title,
            "description": description,
            "image": post.featured_image_url,
            "inLanguage": post.effective_locale,
            "datePublished": post.published_at,
            "dateModified": post.updated_at,
        }),
        fallback_source: "blog".to_string(),
        template_fields,
    }
}

fn parse_blog_route(route: &str) -> AnyResult<Option<String>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    if !matches_module_path(&parsed, "blog") {
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
    channel_slugs.iter().any(|slug| {
        normalize_channel_slug(Some(slug.as_str())).as_deref() == Some(requested_channel.as_str())
    })
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
