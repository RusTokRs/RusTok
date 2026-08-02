use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_seo_targets::{
    SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord, SeoSitemapCandidateRecord,
    SeoTargetBulkListRequest, SeoTargetCapabilities, SeoTargetLoadRequest, SeoTargetLoadScope,
    SeoTargetProvider, SeoTargetRouteResolveRequest, SeoTargetRuntimeContext,
    SeoTargetSitemapRequest, SeoTargetSlug,
};

use crate::ForumPublicDiscoveryService;

#[derive(Clone, Default)]
pub struct ForumCategorySeoTargetProvider;

#[derive(Clone, Default)]
pub struct ForumTopicSeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for ForumCategorySeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        category_provider().slug()
    }

    fn display_name(&self) -> &'static str {
        category_provider().display_name()
    }

    fn owner_module_slug(&self) -> &'static str {
        category_provider().owner_module_slug()
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        category_provider().capabilities()
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        if matches!(request.scope, SeoTargetLoadScope::Authoring) {
            return category_provider().load_target(runtime, request).await;
        }

        let discovery = public_discovery(runtime);
        if discovery
            .get_public_category_with_locale_fallback(
                request.tenant_id,
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await?
            .is_none()
        {
            return Ok(None);
        }

        category_provider().load_target(runtime, request).await
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(candidate) = category_provider().resolve_route(runtime, request).await? else {
            return Ok(None);
        };
        let visible = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id: request.tenant_id,
                    default_locale: request.default_locale,
                    locale: request.locale,
                    target_id: candidate.target_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug: request.channel_slug,
                },
            )
            .await?;
        Ok(visible.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let candidates = category_provider()
            .list_bulk_summaries(runtime, request)
            .await?;
        let discovery = public_discovery(runtime);
        let mut visible = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if discovery
                .get_public_category_with_locale_fallback(
                    request.tenant_id,
                    candidate.target_id,
                    request.locale,
                    Some(request.default_locale),
                )
                .await?
                .is_some()
            {
                visible.push(candidate);
            }
        }
        Ok(visible)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let candidates = category_provider()
            .sitemap_candidates(runtime, request)
            .await?;
        let discovery = public_discovery(runtime);
        let mut visible = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if discovery
                .get_public_category_with_locale_fallback(
                    request.tenant_id,
                    candidate.target_id,
                    candidate.locale.as_str(),
                    Some(request.default_locale),
                )
                .await?
                .is_some()
            {
                visible.push(candidate);
            }
        }
        Ok(visible)
    }
}

#[async_trait]
impl SeoTargetProvider for ForumTopicSeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        topic_provider().slug()
    }

    fn display_name(&self) -> &'static str {
        topic_provider().display_name()
    }

    fn owner_module_slug(&self) -> &'static str {
        topic_provider().owner_module_slug()
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        topic_provider().capabilities()
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        if matches!(request.scope, SeoTargetLoadScope::Authoring) {
            return topic_provider().load_target(runtime, request).await;
        }

        let discovery = public_discovery(runtime);
        if discovery
            .get_public_topic_with_locale_fallback(
                request.tenant_id,
                request.target_id,
                request.locale,
                Some(request.default_locale),
                request.channel_slug,
            )
            .await?
            .is_none()
        {
            return Ok(None);
        }

        topic_provider().load_target(runtime, request).await
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(candidate) = topic_provider().resolve_route(runtime, request).await? else {
            return Ok(None);
        };
        let visible = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id: request.tenant_id,
                    default_locale: request.default_locale,
                    locale: request.locale,
                    target_id: candidate.target_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug: request.channel_slug,
                },
            )
            .await?;
        Ok(visible.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let candidates = topic_provider()
            .list_bulk_summaries(runtime, request)
            .await?;
        let discovery = public_discovery(runtime);
        let mut visible = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if discovery
                .get_public_topic_with_locale_fallback(
                    request.tenant_id,
                    candidate.target_id,
                    request.locale,
                    Some(request.default_locale),
                    None,
                )
                .await?
                .is_some()
            {
                visible.push(candidate);
            }
        }
        Ok(visible)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let candidates = topic_provider()
            .sitemap_candidates(runtime, request)
            .await?;
        let discovery = public_discovery(runtime);
        let mut visible = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            if discovery
                .get_public_topic_with_locale_fallback(
                    request.tenant_id,
                    candidate.target_id,
                    candidate.locale.as_str(),
                    Some(request.default_locale),
                    None,
                )
                .await?
                .is_some()
            {
                visible.push(candidate);
            }
        }
        Ok(visible)
    }
}

fn public_discovery(runtime: &SeoTargetRuntimeContext) -> ForumPublicDiscoveryService {
    ForumPublicDiscoveryService::new(runtime.db.clone(), runtime.event_bus.clone())
}

fn category_provider() -> crate::seo_targets::ForumCategorySeoTargetProvider {
    crate::seo_targets::ForumCategorySeoTargetProvider
}

fn topic_provider() -> crate::seo_targets::ForumTopicSeoTargetProvider {
    crate::seo_targets::ForumTopicSeoTargetProvider
}
