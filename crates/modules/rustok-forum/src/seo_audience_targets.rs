use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result as AnyResult, anyhow};
use async_trait::async_trait;
use rustok_seo_targets::{
    SeoBulkSummaryPage, SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord,
    SeoSitemapCandidateRecord, SeoTargetAlternateRoute, SeoTargetBulkPageRequest,
    SeoTargetCapabilities, SeoTargetLoadRequest, SeoTargetLoadScope, SeoTargetProvider,
    SeoTargetRouteResolveRequest, SeoTargetRuntimeContext, SeoTargetSitemapRequest, SeoTargetSlug,
};
use url::Url;
use uuid::Uuid;

use crate::{
    ForumCategoryRouteService, ForumError, ForumPublicDiscoveryService,
    ForumTopicAudienceListService, ForumTopicRouteDisposition, ForumTopicRouteService,
    ListTopicsFilter, MAX_FORUM_READ_LIMIT,
};

const MAX_FORUM_SEO_ALTERNATE_ROUTES: usize = 64;

#[derive(Clone, Default)]
pub struct ForumCategorySeoTargetProvider;

#[derive(Clone, Default)]
pub struct ForumTopicSeoTargetProvider;

#[derive(Clone, Debug, Eq, PartialEq)]
enum CanonicalForumRoute {
    Category {
        locale: String,
        slug: String,
    },
    Topic {
        locale: String,
        short_id: String,
        slug: String,
    },
}

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

        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
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

        let Some(record) = category_provider().load_target(runtime, request).await? else {
            return Ok(None);
        };
        Ok(Some(
            rewrite_category_target(runtime, tenant_id, default_locale, record).await?,
        ))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let request_locale = request.locale;
        let channel_slug = request.channel_slug;

        let candidate = match parse_canonical_forum_route(request.route)? {
            Some(CanonicalForumRoute::Category { locale, slug }) => {
                let resolution = match ForumCategoryRouteService::new(runtime.db.clone())
                    .resolve(
                        tenant_id,
                        locale.as_str(),
                        slug.as_str(),
                        Some(default_locale),
                    )
                    .await
                {
                    Ok(resolution) => resolution,
                    Err(error) if category_route_absent(&error) => return Ok(None),
                    Err(error) => return Err(error.into()),
                };
                let visible = self
                    .load_target(
                        runtime,
                        SeoTargetLoadRequest {
                            tenant_id,
                            default_locale,
                            locale: resolution.canonical.locale.as_str(),
                            target_id: resolution.canonical.category_id,
                            scope: SeoTargetLoadScope::PublicRoute,
                            channel_slug,
                        },
                    )
                    .await?;
                return Ok(visible.map(|record| SeoRouteMatchRecord {
                    target_kind: record.target_kind,
                    target_id: record.target_id,
                }));
            }
            Some(CanonicalForumRoute::Topic { .. }) => return Ok(None),
            None => category_provider().resolve_route(runtime, request).await?,
        };

        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let visible = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id,
                    default_locale,
                    locale: request_locale,
                    target_id: candidate.target_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug,
                },
            )
            .await?;
        Ok(visible.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries_page(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkPageRequest<'_>,
    ) -> AnyResult<SeoBulkSummaryPage> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let discovery = public_discovery(runtime);
        if !discovery.anonymous_reading_allowed(tenant_id).await? {
            return Ok(SeoBulkSummaryPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        // The legacy mapper scans a raw category keyset. Its cursor may point at a
        // category that the public owner would hide; never forward that cursor or
        // return a sparse page when hidden candidates separate visible ones.
        let limit = request.limit.clamp(1, MAX_FORUM_READ_LIMIT) as usize;
        let route_service = ForumCategoryRouteService::new(runtime.db.clone());
        let mut scan_after = request.after.map(str::to_owned);
        let mut visible = Vec::with_capacity(limit);
        let mut has_more = false;
        'scan: loop {
            let page = category_provider()
                .list_bulk_summaries_page(
                    runtime,
                    SeoTargetBulkPageRequest {
                        after: scan_after.as_deref(),
                        limit: limit as u64,
                        ..request
                    },
                )
                .await?;
            for mut candidate in page.items {
                if discovery
                    .get_public_category_with_locale_fallback(
                        tenant_id,
                        candidate.target_id,
                        candidate.effective_locale.as_str(),
                        Some(default_locale),
                    )
                    .await?
                    .is_none()
                {
                    continue;
                }
                if visible.len() == limit {
                    has_more = true;
                    break 'scan;
                }
                let descriptor = route_service
                    .canonical_descriptor(
                        tenant_id,
                        candidate.target_id,
                        candidate.effective_locale.as_str(),
                        Some(default_locale),
                    )
                    .await?;
                candidate.effective_locale = descriptor.locale;
                candidate.route = descriptor.path;
                visible.push(candidate);
            }
            match page.next_cursor {
                Some(next) if scan_after.as_deref() == Some(next.as_str()) => {
                    return Err(anyhow!("Forum category SEO keyset did not advance"));
                }
                Some(next) => scan_after = Some(next),
                None => break,
            }
        }
        let next_cursor = if has_more {
            visible
                .last()
                .map(|candidate| candidate.target_id.to_string())
        } else {
            None
        };
        Ok(SeoBulkSummaryPage {
            items: visible,
            next_cursor,
        })
    }

    async fn public_sitemap_requires_refresh(
        &self,
        runtime: &SeoTargetRuntimeContext,
        tenant_id: Uuid,
    ) -> AnyResult<bool> {
        // A saved sitemap can also predate a newly restricted category/topic
        // audience policy when anonymous reading remains enabled.
        public_discovery(runtime)
            .anonymous_reading_allowed(tenant_id)
            .await?;
        Ok(true)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let discovery = public_discovery(runtime);
        if !discovery.anonymous_reading_allowed(tenant_id).await? {
            return Ok(Vec::new());
        }
        let candidates = category_provider()
            .sitemap_candidates(runtime, request)
            .await?;
        let route_service = ForumCategoryRouteService::new(runtime.db.clone());
        let mut visible = Vec::with_capacity(candidates.len());
        for mut candidate in candidates {
            if discovery
                .get_public_category_with_locale_fallback(
                    tenant_id,
                    candidate.target_id,
                    candidate.locale.as_str(),
                    Some(default_locale),
                )
                .await?
                .is_none()
            {
                continue;
            }
            let descriptor = route_service
                .canonical_descriptor(
                    tenant_id,
                    candidate.target_id,
                    candidate.locale.as_str(),
                    Some(default_locale),
                )
                .await?;
            candidate.locale = descriptor.locale;
            candidate.route = descriptor.path;
            visible.push(candidate);
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

        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
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

        let Some(record) = topic_provider().load_target(runtime, request).await? else {
            return Ok(None);
        };
        Ok(Some(
            rewrite_topic_target(runtime, tenant_id, default_locale, record).await?,
        ))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let request_locale = request.locale;
        let channel_slug = request.channel_slug;

        let candidate = match parse_canonical_forum_route(request.route)? {
            Some(CanonicalForumRoute::Topic {
                locale,
                short_id,
                slug,
            }) => {
                let resolution = match ForumTopicRouteService::new(runtime.db.clone())
                    .resolve(tenant_id, locale.as_str(), short_id.as_str(), slug.as_str())
                    .await
                {
                    Ok(resolution) => resolution,
                    Err(error) if topic_route_absent(&error) => return Ok(None),
                    Err(error) => return Err(error.into()),
                };
                if resolution.disposition == ForumTopicRouteDisposition::Gone {
                    return Ok(None);
                }
                let canonical = resolution.canonical.ok_or_else(|| {
                    anyhow!("Forum topic SEO route resolved without a canonical target")
                })?;
                let visible = self
                    .load_target(
                        runtime,
                        SeoTargetLoadRequest {
                            tenant_id,
                            default_locale,
                            locale: canonical.locale.as_str(),
                            target_id: canonical.topic_id,
                            scope: SeoTargetLoadScope::PublicRoute,
                            channel_slug,
                        },
                    )
                    .await?;
                return Ok(visible.map(|record| SeoRouteMatchRecord {
                    target_kind: record.target_kind,
                    target_id: record.target_id,
                }));
            }
            Some(CanonicalForumRoute::Category { .. }) => return Ok(None),
            None => topic_provider().resolve_route(runtime, request).await?,
        };

        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let visible = self
            .load_target(
                runtime,
                SeoTargetLoadRequest {
                    tenant_id,
                    default_locale,
                    locale: request_locale,
                    target_id: candidate.target_id,
                    scope: SeoTargetLoadScope::PublicRoute,
                    channel_slug,
                },
            )
            .await?;
        Ok(visible.map(|record| SeoRouteMatchRecord {
            target_kind: record.target_kind,
            target_id: record.target_id,
        }))
    }

    async fn list_bulk_summaries_page(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkPageRequest<'_>,
    ) -> AnyResult<SeoBulkSummaryPage> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let discovery = public_discovery(runtime);
        if !discovery.anonymous_reading_allowed(tenant_id).await? {
            return Ok(SeoBulkSummaryPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        // Unlike the legacy mapper's raw topic list, this owner derives both the
        // items and the cursor from the audience-allowed sequence before paging.
        let page =
            ForumTopicAudienceListService::new(runtime.db.clone(), runtime.event_bus.clone())
                .with_settings_providers(crate::seo_targets::seo_settings_providers(runtime))
                .list_public_storefront_visible_with_locale_fallback(
                    tenant_id,
                    ListTopicsFilter {
                        search: None,
                        order: Default::default(),
                        solved_only: false,
                        category_id: None,
                        status: None,
                        locale: Some(request.locale.to_string()),
                        after: request.after.map(str::to_owned),
                        per_page: request.limit.clamp(1, MAX_FORUM_READ_LIMIT),
                    },
                    Some(default_locale),
                    None,
                )
                .await?;
        let has_more = page.next_cursor.is_some();
        let mut visible = Vec::with_capacity(page.items.len());
        let mut last_visible_cursor = None;
        for item in page.items {
            // Recheck before hydrating a public SEO summary; a policy may have
            // narrowed between the list scan and this route's load.
            let Some(record) = self
                .load_target(
                    runtime,
                    SeoTargetLoadRequest {
                        tenant_id,
                        default_locale,
                        locale: item.effective_locale.as_str(),
                        target_id: item.id,
                        scope: SeoTargetLoadScope::PublicRoute,
                        channel_slug: None,
                    },
                )
                .await?
            else {
                continue;
            };
            last_visible_cursor = item.sort_key.map(|key| key.encode());
            visible.push(SeoBulkSummaryRecord {
                target_kind: record.target_kind,
                target_id: record.target_id,
                effective_locale: record.effective_locale,
                label: record.title,
                route: record.canonical_route,
            });
        }
        Ok(SeoBulkSummaryPage {
            items: visible,
            next_cursor: if has_more { last_visible_cursor } else { None },
        })
    }

    async fn public_sitemap_requires_refresh(
        &self,
        runtime: &SeoTargetRuntimeContext,
        tenant_id: Uuid,
    ) -> AnyResult<bool> {
        // A saved sitemap can also predate a newly restricted category/topic
        // audience policy when anonymous reading remains enabled.
        public_discovery(runtime)
            .anonymous_reading_allowed(tenant_id)
            .await?;
        Ok(true)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let tenant_id = request.tenant_id;
        let default_locale = request.default_locale;
        let discovery = public_discovery(runtime);
        if !discovery.anonymous_reading_allowed(tenant_id).await? {
            return Ok(Vec::new());
        }
        let candidates = topic_provider()
            .sitemap_candidates(runtime, request)
            .await?;
        let route_service = ForumTopicRouteService::new(runtime.db.clone());
        let mut visible = Vec::with_capacity(candidates.len());
        for mut candidate in candidates {
            if discovery
                .get_public_topic_with_locale_fallback(
                    tenant_id,
                    candidate.target_id,
                    candidate.locale.as_str(),
                    Some(default_locale),
                    None,
                )
                .await?
                .is_none()
            {
                continue;
            }
            let descriptor = route_service
                .canonical_descriptor(tenant_id, candidate.target_id, candidate.locale.as_str())
                .await?;
            candidate.locale = descriptor.locale;
            candidate.route = descriptor.path;
            visible.push(candidate);
        }
        Ok(visible)
    }
}

async fn rewrite_category_target(
    runtime: &SeoTargetRuntimeContext,
    tenant_id: Uuid,
    default_locale: &str,
    mut record: SeoLoadedTargetRecord,
) -> AnyResult<SeoLoadedTargetRecord> {
    let service = ForumCategoryRouteService::new(runtime.db.clone());
    let canonical = service
        .canonical_descriptor(
            tenant_id,
            record.target_id,
            record.effective_locale.as_str(),
            Some(default_locale),
        )
        .await?;
    let locales = alternate_locales(&record)?;
    let mut alternates = BTreeMap::new();
    for locale in locales {
        let descriptor = service
            .canonical_descriptor(tenant_id, record.target_id, locale.as_str(), None)
            .await?;
        if descriptor.locale != locale {
            return Err(anyhow!(
                "Forum category SEO alternate resolved through an unexpected locale fallback"
            ));
        }
        alternates.insert(descriptor.locale, descriptor.path);
    }
    alternates
        .entry(canonical.locale.clone())
        .or_insert_with(|| canonical.path.clone());
    apply_canonical_routes(&mut record, canonical.locale, canonical.path, alternates);
    Ok(record)
}

async fn rewrite_topic_target(
    runtime: &SeoTargetRuntimeContext,
    tenant_id: Uuid,
    _default_locale: &str,
    mut record: SeoLoadedTargetRecord,
) -> AnyResult<SeoLoadedTargetRecord> {
    let service = ForumTopicRouteService::new(runtime.db.clone());
    let canonical = service
        .canonical_descriptor(
            tenant_id,
            record.target_id,
            record.effective_locale.as_str(),
        )
        .await?;
    let locales = alternate_locales(&record)?;
    let mut alternates = BTreeMap::new();
    for locale in locales {
        let descriptor = service
            .canonical_descriptor(tenant_id, record.target_id, locale.as_str())
            .await?;
        if descriptor.locale != locale {
            return Err(anyhow!(
                "Forum topic SEO alternate resolved through an unexpected locale fallback"
            ));
        }
        alternates.insert(descriptor.locale, descriptor.path);
    }
    alternates
        .entry(canonical.locale.clone())
        .or_insert_with(|| canonical.path.clone());
    apply_canonical_routes(&mut record, canonical.locale, canonical.path, alternates);
    Ok(record)
}

fn alternate_locales(record: &SeoLoadedTargetRecord) -> AnyResult<BTreeSet<String>> {
    let mut locales = record
        .alternates
        .iter()
        .map(|alternate| alternate.locale.clone())
        .collect::<BTreeSet<_>>();
    locales.insert(record.effective_locale.clone());
    if locales.len() > MAX_FORUM_SEO_ALTERNATE_ROUTES {
        return Err(anyhow!(
            "Forum SEO alternate route count exceeds the bounded public contract"
        ));
    }
    Ok(locales)
}

fn apply_canonical_routes(
    record: &mut SeoLoadedTargetRecord,
    effective_locale: String,
    canonical_route: String,
    alternates: BTreeMap<String, String>,
) {
    record.effective_locale = effective_locale.clone();
    record.canonical_route = canonical_route.clone();
    record.alternates = alternates
        .into_iter()
        .map(|(locale, route)| SeoTargetAlternateRoute { locale, route })
        .collect();
    record.template_fields.insert("locale", effective_locale);
    record.template_fields.insert("route", canonical_route);
}

fn parse_canonical_forum_route(route: &str) -> AnyResult<Option<CanonicalForumRoute>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    let segments = parsed
        .path_segments()
        .map(|items| items.filter(|item| !item.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();
    match segments.as_slice() {
        [locale, "forum", "c", slug] => Ok(Some(CanonicalForumRoute::Category {
            locale: (*locale).to_string(),
            slug: (*slug).to_string(),
        })),
        [locale, "forum", "t", short_id, slug] => Ok(Some(CanonicalForumRoute::Topic {
            locale: (*locale).to_string(),
            short_id: (*short_id).to_string(),
            slug: (*slug).to_string(),
        })),
        _ => Ok(None),
    }
}

fn category_route_absent(error: &ForumError) -> bool {
    matches!(
        error,
        ForumError::CategoryRouteNotFound
            | ForumError::CategoryNotFound(_)
            | ForumError::Validation(_)
    )
}

fn topic_route_absent(error: &ForumError) -> bool {
    matches!(
        error,
        ForumError::TopicRouteNotFound
            | ForumError::TopicNotFound(_)
            | ForumError::TopicDeleted
            | ForumError::Validation(_)
    )
}

fn public_discovery(runtime: &SeoTargetRuntimeContext) -> ForumPublicDiscoveryService {
    ForumPublicDiscoveryService::new(runtime.db.clone(), runtime.event_bus.clone())
        .with_settings_providers(crate::seo_targets::seo_settings_providers(runtime))
}

fn category_provider() -> crate::seo_targets::ForumCategorySeoTargetProvider {
    crate::seo_targets::ForumCategorySeoTargetProvider
}

fn topic_provider() -> crate::seo_targets::ForumTopicSeoTargetProvider {
    crate::seo_targets::ForumTopicSeoTargetProvider
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rustok_api::{
        PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
        StaticModuleSettingsSnapshot,
    };
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use sea_orm::Database;
    use std::sync::Arc;

    struct ForumReading(bool);

    #[async_trait]
    impl StaticModuleSettingsReader for ForumReading {
        async fn settings(
            &self,
            _tenant: Uuid,
            slug: &str,
        ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
            assert_eq!(slug, "forum");
            Ok(Some(StaticModuleSettingsSnapshot {
                enabled: true,
                settings: serde_json::json!({"allow_anonymous_reading": self.0}),
            }))
        }
    }

    #[tokio::test]
    async fn private_tenant_exposes_no_seo_bulk_cursor_or_sitemap_candidates() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let reader = SharedStaticModuleSettingsReader(Arc::new(ForumReading(false)));
        let runtime = SeoTargetRuntimeContext {
            event_bus: TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
            db,
            static_settings_reader: Some(reader),
        };
        let tenant = Uuid::new_v4();
        assert!(
            ForumCategorySeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await
                .unwrap()
        );
        assert!(
            ForumTopicSeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await
                .unwrap()
        );
        for page in [
            ForumCategorySeoTargetProvider
                .list_bulk_summaries_page(
                    &runtime,
                    SeoTargetBulkPageRequest {
                        tenant_id: tenant,
                        default_locale: "en",
                        locale: "en",
                        after: None,
                        limit: 10,
                    },
                )
                .await
                .unwrap(),
            ForumTopicSeoTargetProvider
                .list_bulk_summaries_page(
                    &runtime,
                    SeoTargetBulkPageRequest {
                        tenant_id: tenant,
                        default_locale: "en",
                        locale: "en",
                        after: None,
                        limit: 10,
                    },
                )
                .await
                .unwrap(),
        ] {
            assert!(page.items.is_empty());
            assert!(page.next_cursor.is_none());
        }
        for candidates in [
            ForumCategorySeoTargetProvider
                .sitemap_candidates(
                    &runtime,
                    SeoTargetSitemapRequest {
                        tenant_id: tenant,
                        default_locale: "en",
                    },
                )
                .await
                .unwrap(),
            ForumTopicSeoTargetProvider
                .sitemap_candidates(
                    &runtime,
                    SeoTargetSitemapRequest {
                        tenant_id: tenant,
                        default_locale: "en",
                    },
                )
                .await
                .unwrap(),
        ] {
            assert!(candidates.is_empty());
        }
    }

    #[tokio::test]
    async fn public_sitemaps_recheck_owner_policy_even_when_anonymous_reading_is_enabled() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let mut runtime = SeoTargetRuntimeContext {
            event_bus: TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
            db,
            static_settings_reader: Some(SharedStaticModuleSettingsReader(Arc::new(ForumReading(
                true,
            )))),
        };
        let tenant = Uuid::new_v4();
        for refresh in [
            ForumCategorySeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await,
            ForumTopicSeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await,
        ] {
            assert!(refresh.expect("stored URLs may predate a changed audience policy"));
        }
        runtime.static_settings_reader = None;
        assert!(
            ForumCategorySeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await
                .is_err()
        );
        assert!(
            ForumTopicSeoTargetProvider
                .public_sitemap_requires_refresh(&runtime, tenant)
                .await
                .is_err()
        );
    }

    #[test]
    fn recognizes_only_canonical_forum_category_and_topic_paths() {
        assert_eq!(
            parse_canonical_forum_route("/en/forum/c/general").unwrap(),
            Some(CanonicalForumRoute::Category {
                locale: "en".to_string(),
                slug: "general".to_string(),
            })
        );
        assert_eq!(
            parse_canonical_forum_route("/ru/forum/t/123456789abc/privet").unwrap(),
            Some(CanonicalForumRoute::Topic {
                locale: "ru".to_string(),
                short_id: "123456789abc".to_string(),
                slug: "privet".to_string(),
            })
        );
        assert!(
            parse_canonical_forum_route(
                "/en/modules/forum?category=00000000-0000-0000-0000-000000000000"
            )
            .unwrap()
            .is_none()
        );
        assert!(
            parse_canonical_forum_route("/en/forum/c/general/extra")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn canonical_route_application_is_sorted_and_updates_template_fields() {
        let mut record = SeoLoadedTargetRecord {
            target_kind: SeoTargetSlug::new("forum_category").unwrap(),
            target_id: Uuid::nil(),
            requested_locale: Some("en".to_string()),
            effective_locale: "en".to_string(),
            title: "General".to_string(),
            description: None,
            canonical_route: "/modules/forum?category=legacy".to_string(),
            alternates: Vec::new(),
            open_graph: Default::default(),
            structured_data: serde_json::json!({}),
            fallback_source: "forum_category".to_string(),
            template_fields: Default::default(),
        };
        apply_canonical_routes(
            &mut record,
            "en".to_string(),
            "/en/forum/c/general".to_string(),
            BTreeMap::from([
                ("ru".to_string(), "/ru/forum/c/obshchee".to_string()),
                ("en".to_string(), "/en/forum/c/general".to_string()),
            ]),
        );

        assert_eq!(record.canonical_route, "/en/forum/c/general");
        assert_eq!(
            record
                .alternates
                .iter()
                .map(|alternate| alternate.locale.as_str())
                .collect::<Vec<_>>(),
            vec!["en", "ru"]
        );
        assert_eq!(
            record
                .template_fields
                .values
                .get("route")
                .map(String::as_str),
            Some("/en/forum/c/general")
        );
    }
}

#[cfg(test)]
#[path = "seo_audience_targets_sqlite_tests.rs"]
mod sqlite_tests;
