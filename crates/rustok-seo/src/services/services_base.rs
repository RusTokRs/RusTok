use std::sync::Arc;

use chrono::{DateTime, Utc};
use rustok_content::{ContentProjection, ContentReadPort, ContentReadRequest, ContentSelector};
use rustok_core::ModuleRuntimeExtensions;
use rustok_events::TransactionalEventBus;
use rustok_media::ports::{MediaAssetReadPort, SeoMediaAssetReadProvider};
use rustok_tenant::PortContext;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    entity::tenant_module,
    error::{SeoError, SeoResult},
    model::{
        SeoAlternateLink, SeoMeta, SeoMetaInput, SeoOpenGraph, SeoResolveRequest,
        SeoResolveResponse, SeoRobots, SeoSettings, SeoSitemapEntry, SeoSitemapSettings,
        SeoStructuredDataKind,
    },
    registry::{
        built_in_target_registry, seo_target_registry_from_extensions, SeoTargetRegistry,
    },
    settings::SeoModuleSettings,
    MODULE_SLUG,
};

#[derive(Debug, Clone)]
struct ResolvedSeoTarget {
    tenant_id: Uuid,
    target_type: String,
    target_id: Uuid,
    route: String,
    locale: String,
    published_at: Option<DateTime<Utc>>,
    title: String,
    description: Option<String>,
    keywords: Vec<String>,
    canonical_path: Option<String>,
    image_asset_id: Option<Uuid>,
    noindex: bool,
    nofollow: bool,
    alternates: Vec<SeoAlternateLink>,
    open_graph: SeoOpenGraph,
    structured_data: serde_json::Value,
    fallback_source: String,
    template_fields: std::collections::BTreeMap<String, String>,
}

impl SeoService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        registry: Arc<SeoTargetRegistry>,
    ) -> Self {
        Self {
            db,
            event_bus,
            registry,
            media_asset_read_port: None,
        }
    }

    pub fn with_media_asset_read_port(mut self, port: Arc<dyn MediaAssetReadPort>) -> Self {
        self.media_asset_read_port = Some(port);
        self
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        extensions: &ModuleRuntimeExtensions,
    ) -> SeoResult<Self> {
        let registry = seo_target_registry_from_extensions(extensions)
            .ok_or_else(|| SeoError::configuration("SEO target registry is not initialized"))?;
        let service = Self::new(db, event_bus, registry);
        if let Some(provider) = extensions.get::<SeoMediaAssetReadProvider>() {
            Ok(service.with_media_asset_read_port(provider.port()))
        } else {
            Ok(service)
        }
    }

    #[cfg(test)]
    pub(crate) fn with_builtin_registry(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> Self {
        Self::new(db, event_bus, built_in_target_registry())
    }

    #[cfg(test)]
    pub(crate) fn new_memory(db: DatabaseConnection) -> Self {
        Self::with_builtin_registry(
            db,
            TransactionalEventBus::new(Arc::new(rustok_events::MemoryTransport::new())),
        )
    }

    pub async fn is_enabled(&self, tenant_id: Uuid) -> SeoResult<bool> {
        tenant_module::Entity::is_enabled(&self.db, tenant_id, MODULE_SLUG)
            .await
            .map_err(SeoError::from)
    }

    pub async fn load_settings(&self, tenant_id: Uuid) -> SeoResult<SeoModuleSettings> {
        let Some(module) = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(MODULE_SLUG))
            .one(&self.db)
            .await?
        else {
            return Ok(SeoModuleSettings::default());
        };

        SeoModuleSettings::from_json(module.settings)
    }

    pub async fn resolve(&self, request: SeoResolveRequest) -> SeoResult<SeoResolveResponse> {
        let settings = self.load_settings(request.tenant_id).await?;
        let target = self.resolve_target(&request, &settings).await?;
        let meta = self
            .load_target_meta(request.tenant_id, &request.target_type, request.target_id)
            .await?;
        self.build_response(target, meta, settings).await
    }

    pub async fn save_meta(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Uuid,
        input: SeoMetaInput,
    ) -> SeoResult<SeoMeta> {
        crate::persistence::save_meta(&self.db, tenant_id, target_type, target_id, input).await
    }

    pub async fn sitemap_entries(
        &self,
        tenant_id: Uuid,
        settings: &SeoSitemapSettings,
    ) -> SeoResult<Vec<SeoSitemapEntry>> {
        let mut entries = Vec::new();
        for target in self.registry.targets() {
            let mut target_entries = target.sitemap_entries(tenant_id, settings).await?;
            entries.append(&mut target_entries);
        }
        entries.sort_by(|left, right| left.location.cmp(&right.location));
        entries.dedup_by(|left, right| left.location == right.location);
        Ok(entries)
    }

    async fn resolve_target(
        &self,
        request: &SeoResolveRequest,
        settings: &SeoModuleSettings,
    ) -> SeoResult<ResolvedSeoTarget> {
        let target = self
            .registry
            .target(&request.target_type)
            .ok_or_else(|| SeoError::not_found("SEO target resolver is not registered"))?;
        let projection = target
            .resolve(
                PortContext::system(request.tenant_id.to_string()),
                ContentReadRequest {
                    selector: ContentSelector::Id(request.target_id),
                    include_unpublished: request.include_unpublished,
                },
            )
            .await?;
        self.target_from_projection(request, settings, projection).await
    }

    async fn target_from_projection(
        &self,
        request: &SeoResolveRequest,
        settings: &SeoModuleSettings,
        projection: ContentProjection,
    ) -> SeoResult<ResolvedSeoTarget> {
        let route = projection
            .route
            .clone()
            .unwrap_or_else(|| format!("/{}/{}", request.target_type, request.target_id));
        let image_asset_id = projection.image_asset_id;
        let image_url = if let (Some(port), Some(asset_id)) =
            (self.media_asset_read_port.as_ref(), image_asset_id)
        {
            port.read_asset_url(
                PortContext::system(request.tenant_id.to_string()),
                asset_id,
            )
            .await?
        } else {
            None
        };
        let mut open_graph = SeoOpenGraph::default();
        open_graph.image = image_url;
        Ok(ResolvedSeoTarget {
            tenant_id: request.tenant_id,
            target_type: request.target_type.clone(),
            target_id: request.target_id,
            route,
            locale: projection.locale,
            published_at: projection.published_at,
            title: projection.title,
            description: projection.description,
            keywords: projection.keywords,
            canonical_path: projection.canonical_path,
            image_asset_id,
            noindex: projection.noindex,
            nofollow: projection.nofollow,
            alternates: projection.alternates,
            open_graph,
            structured_data: projection.structured_data,
            fallback_source: projection.source,
            template_fields: projection.template_fields,
        })
    }

    async fn load_target_meta(
        &self,
        tenant_id: Uuid,
        target_type: &str,
        target_id: Uuid,
    ) -> SeoResult<Option<SeoMeta>> {
        crate::persistence::find_meta(&self.db, tenant_id, target_type, target_id).await
    }

    async fn build_response(
        &self,
        target: ResolvedSeoTarget,
        meta: Option<SeoMeta>,
        settings: SeoModuleSettings,
    ) -> SeoResult<SeoResolveResponse> {
        let title = meta
            .as_ref()
            .and_then(|value| value.title.clone())
            .unwrap_or_else(|| target.title.clone());
        let description = meta
            .as_ref()
            .and_then(|value| value.description.clone())
            .or_else(|| target.description.clone());
        let keywords = meta
            .as_ref()
            .map(|value| value.keywords.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| target.keywords.clone());
        let canonical_path = meta
            .as_ref()
            .and_then(|value| value.canonical_path.clone())
            .or_else(|| target.canonical_path.clone())
            .unwrap_or_else(|| target.route.clone());
        let canonical_url = settings.canonical_url(&canonical_path);
        let robots = SeoRobots {
            noindex: meta.as_ref().map(|value| value.noindex).unwrap_or(target.noindex),
            nofollow: meta
                .as_ref()
                .map(|value| value.nofollow)
                .unwrap_or(target.nofollow),
        };
        let structured_data = self.structured_data(&target, &settings);
        Ok(SeoResolveResponse {
            title,
            description,
            keywords,
            canonical_url,
            robots,
            alternates: target.alternates,
            open_graph: target.open_graph,
            structured_data,
            fallback_source: target.fallback_source,
        })
    }

    fn structured_data(
        &self,
        target: &ResolvedSeoTarget,
        settings: &SeoModuleSettings,
    ) -> serde_json::Value {
        if target.structured_data != serde_json::Value::Null {
            return target.structured_data.clone();
        }
        serde_json::json!({
            "@context": "https://schema.org",
            "@type": SeoStructuredDataKind::WebPage.as_str(),
            "url": settings.canonical_url(&target.route),
            "name": target.title,
        })
    }
}
