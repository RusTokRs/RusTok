use anyhow::Result as AnyResult;
use async_trait::async_trait;
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
use uuid::Uuid;

use crate::constants::topic_status;
use crate::{
    CategoryListItem, CategoryResponse, CategoryService, ListTopicsFilter, TopicListItem,
    TopicResponse, TopicService,
};

const BULK_FETCH_SIZE: u64 = 48;

#[derive(Clone, Default)]
pub struct ForumCategorySeoTargetProvider;

#[derive(Clone, Default)]
pub struct ForumTopicSeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for ForumCategorySeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::FORUM_CATEGORY)
            .expect("builtin SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Forum category"
    }

    fn owner_module_slug(&self) -> &'static str {
        "forum"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let service = CategoryService::new(runtime.db.clone());
        let category = service
            .get_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await
            .ok();
        Ok(category.map(map_category_response))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(route) = parse_forum_route(request.route)? else {
            return Ok(None);
        };
        let ForumRoute::Category(category_id) = route else {
            return Ok(None);
        };
        let loaded = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id: request.tenant_id,
                    default_locale: request.default_locale,
                    locale: request.locale,
                    target_id: category_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug: request.channel_slug,
                },
            )
            .await?;
        Ok(loaded.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let service = CategoryService::new(runtime.db.clone());
        let mut page_number = 1_u64;
        let mut summaries = Vec::new();

        loop {
            let (items, total) = service
                .list_paginated_with_locale_fallback(
                    request.tenant_id,
                    SecurityContext::system(),
                    request.locale,
                    page_number,
                    BULK_FETCH_SIZE,
                    Some(request.default_locale),
                )
                .await?;
            if items.is_empty() {
                break;
            }

            for item in items {
                if let Some(summary) = load_category_summary(
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
        let service = CategoryService::new(runtime.db.clone());
        let mut page_number = 1_u64;
        let mut candidates = Vec::new();

        loop {
            let (items, total) = service
                .list_paginated_with_locale_fallback(
                    request.tenant_id,
                    SecurityContext::system(),
                    request.default_locale,
                    page_number,
                    BULK_FETCH_SIZE,
                    Some(request.default_locale),
                )
                .await?;
            if items.is_empty() {
                break;
            }

            for item in items {
                if let Some(candidate) = load_category_sitemap_candidate(
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

#[async_trait]
impl SeoTargetProvider for ForumTopicSeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::FORUM_TOPIC)
            .expect("builtin SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Forum topic"
    }

    fn owner_module_slug(&self) -> &'static str {
        "forum"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let service = TopicService::new(runtime.db.clone(), runtime.event_bus.clone());
        let topic = service
            .get_with_locale_fallback(
                request.tenant_id,
                SecurityContext::system(),
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await
            .ok();
        let Some(topic) = topic else {
            return Ok(None);
        };

        if matches!(request.scope, SeoTargetLoadScope::PublicRoute)
            && (topic.status != topic_status::OPEN
                || !channel_visible(topic.channel_slugs.as_slice(), request.channel_slug))
        {
            return Ok(None);
        }

        Ok(Some(map_topic_response(topic)))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(route) = parse_forum_route(request.route)? else {
            return Ok(None);
        };
        let ForumRoute::Topic(topic_id) = route else {
            return Ok(None);
        };
        let loaded = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id: request.tenant_id,
                    default_locale: request.default_locale,
                    locale: request.locale,
                    target_id: topic_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug: request.channel_slug,
                },
            )
            .await?;
        Ok(loaded.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let service = TopicService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u64;
        let mut summaries = Vec::new();

        loop {
            let (items, total) = service
                .list_with_locale_fallback(
                    request.tenant_id,
                    SecurityContext::system(),
                    ListTopicsFilter {
                        category_id: None,
                        status: None,
                        locale: Some(request.locale.to_string()),
                        page: page_number,
                        per_page: BULK_FETCH_SIZE,
                    },
                    Some(request.default_locale),
                )
                .await?;
            if items.is_empty() {
                break;
            }

            for item in items {
                if let Some(summary) = load_topic_summary(
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
        let service = TopicService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page_number = 1_u64;
        let mut candidates = Vec::new();

        loop {
            let (items, total) = service
                .list_storefront_visible_with_locale_fallback(
                    request.tenant_id,
                    SecurityContext::system(),
                    ListTopicsFilter {
                        category_id: None,
                        status: Some(topic_status::OPEN.to_string()),
                        locale: Some(request.default_locale.to_string()),
                        page: page_number,
                        per_page: BULK_FETCH_SIZE,
                    },
                    Some(request.default_locale),
                    None,
                )
                .await?;
            if items.is_empty() {
                break;
            }

            for item in items {
                if let Some(candidate) = load_topic_sitemap_candidate(
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

async fn load_category_summary(
    service: &CategoryService,
    tenant_id: Uuid,
    locale: &str,
    default_locale: &str,
    item: CategoryListItem,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let category = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(category) = category else {
        return Ok(None);
    };
    let mapped = map_category_response(category);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_category_sitemap_candidate(
    service: &CategoryService,
    tenant_id: Uuid,
    default_locale: &str,
    item: CategoryListItem,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let category = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            default_locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(category) = category else {
        return Ok(None);
    };
    let mapped = map_category_response(category);
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
    }))
}

async fn load_topic_summary(
    service: &TopicService,
    tenant_id: Uuid,
    locale: &str,
    default_locale: &str,
    item: TopicListItem,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let topic = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(topic) = topic else {
        return Ok(None);
    };
    let mapped = map_topic_response(topic);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_topic_sitemap_candidate(
    service: &TopicService,
    tenant_id: Uuid,
    default_locale: &str,
    item: TopicListItem,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let topic = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::system(),
            item.id,
            default_locale,
            Some(default_locale),
        )
        .await
        .ok();
    let Some(topic) = topic else {
        return Ok(None);
    };
    if topic.status != topic_status::OPEN || !channel_visible(topic.channel_slugs.as_slice(), None)
    {
        return Ok(None);
    }
    let mapped = map_topic_response(topic);
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
    }))
}

fn map_category_response(category: CategoryResponse) -> SeoLoadedTargetRecord {
    let title = category.name.clone();
    let description = category
        .description
        .clone()
        .or_else(|| summarize_text(category.name.as_str()));
    let canonical_route = format!("/modules/forum?category={}", category.id);
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("locale", category.effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());
    template_fields.insert("category_id", category.id.to_string());

    SeoLoadedTargetRecord {
        target_kind: SeoTargetSlug::new(builtin_slug::FORUM_CATEGORY)
            .expect("builtin SEO target slug must stay valid"),
        target_id: category.id,
        requested_locale: Some(category.requested_locale.clone()),
        effective_locale: category.effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_route: canonical_route.clone(),
        alternates: category
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
            kind: Some("website".to_string()),
            site_name: None,
            url: None,
            locale: Some(category.effective_locale.clone()),
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "CollectionPage",
            "name": category.name,
            "description": description,
            "inLanguage": category.effective_locale,
        }),
        fallback_source: "forum_category".to_string(),
        template_fields,
    }
}

fn map_topic_response(topic: TopicResponse) -> SeoLoadedTargetRecord {
    let title = topic.title.clone();
    let description =
        summarize_text(topic.body.as_str()).or_else(|| summarize_text(title.as_str()));
    let canonical_route = format!(
        "/modules/forum?category={}&topic={}",
        topic.category_id, topic.id
    );
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("locale", topic.effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());
    template_fields.insert("topic_id", topic.id.to_string());
    template_fields.insert("category_id", topic.category_id.to_string());

    SeoLoadedTargetRecord {
        target_kind: SeoTargetSlug::new(builtin_slug::FORUM_TOPIC)
            .expect("builtin SEO target slug must stay valid"),
        target_id: topic.id,
        requested_locale: Some(topic.requested_locale.clone()),
        effective_locale: topic.effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_route: canonical_route.clone(),
        alternates: topic
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
            locale: Some(topic.effective_locale.clone()),
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "DiscussionForumPosting",
            "headline": topic.title,
            "articleBody": topic.body,
            "description": description,
            "inLanguage": topic.effective_locale,
            "datePublished": topic.created_at,
            "dateModified": topic.updated_at,
        }),
        fallback_source: "forum_topic".to_string(),
        template_fields,
    }
}

enum ForumRoute {
    Category(Uuid),
    Topic(Uuid),
}

fn parse_forum_route(route: &str) -> AnyResult<Option<ForumRoute>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    if !matches_module_path(&parsed, "forum") {
        return Ok(None);
    }

    let mut category_id = None;
    let mut topic_id = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "category" => category_id = Uuid::parse_str(value.as_ref()).ok(),
            "topic" => topic_id = Uuid::parse_str(value.as_ref()).ok(),
            _ => {}
        }
    }

    if let Some(topic_id) = topic_id {
        return Ok(Some(ForumRoute::Topic(topic_id)));
    }
    Ok(category_id.map(ForumRoute::Category))
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
