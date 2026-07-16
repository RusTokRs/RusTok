use uuid::Uuid;

use std::time::Duration;

use rustok_api::{PortActor, PortContext, TenantContext};
use rustok_media::MediaAssetReadPort;
use rustok_seo_targets::{
    SeoLoadedTargetRecord, SeoTargetLoadRequest, SeoTargetLoadScope, SeoTargetOpenGraphRecord,
    SeoTargetRuntimeContext, SeoTargetSlug,
};

use crate::dto::{SeoAlternateLink, SeoImageAsset, SeoOpenGraph};
use crate::{SeoError, SeoResult};

use super::routing::locale_prefixed_path;
use super::{SeoService, TargetState};

impl SeoService {
    pub(super) async fn load_target_state(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: &str,
    ) -> SeoResult<Option<TargetState>> {
        self.load_target_state_with_scope(
            tenant,
            target_kind,
            target_id,
            locale,
            SeoTargetLoadScope::Authoring,
            None,
        )
        .await
    }

    pub(super) async fn load_route_target_state(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<TargetState>> {
        self.load_target_state_with_scope(
            tenant,
            target_kind,
            target_id,
            locale,
            SeoTargetLoadScope::PublicRoute,
            channel_slug,
        )
        .await
    }

    async fn load_target_state_with_scope(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: &str,
        scope: SeoTargetLoadScope,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<TargetState>> {
        let Some(provider) = self.registry.get(&target_kind) else {
            return Ok(None);
        };
        let record = provider
            .load_target(
                &self.target_runtime(),
                SeoTargetLoadRequest {
                    tenant_id: tenant.id,
                    default_locale: tenant.default_locale.as_str(),
                    locale,
                    target_id,
                    scope,
                    channel_slug,
                },
            )
            .await
            .map_err(|error| {
                SeoError::validation(format!(
                    "SEO target provider `{}` failed to load target: {error}",
                    target_kind.as_str()
                ))
            })?;

        Ok(match record {
            Some(record) => Some(
                map_loaded_target_record(
                    record,
                    tenant.id,
                    locale,
                    self.media_asset_read_port.as_deref(),
                )
                .await,
            ),
            None => None,
        })
    }

    pub(super) fn target_runtime(&self) -> SeoTargetRuntimeContext {
        SeoTargetRuntimeContext {
            db: self.db.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}

async fn map_loaded_target_record(
    record: SeoLoadedTargetRecord,
    tenant_id: Uuid,
    locale: &str,
    media_port: Option<&dyn MediaAssetReadPort>,
) -> TargetState {
    TargetState {
        target_kind: record.target_kind,
        target_id: record.target_id,
        requested_locale: record.requested_locale,
        effective_locale: record.effective_locale,
        title: record.title,
        description: record.description,
        canonical_path: record.canonical_route,
        alternates: record
            .alternates
            .into_iter()
            .map(|item| SeoAlternateLink {
                locale: item.locale.clone(),
                href: locale_prefixed_path(item.locale.as_str(), item.route.as_str()),
                x_default: false,
            })
            .collect(),
        open_graph: map_open_graph(record.open_graph, tenant_id, locale, media_port).await,
        structured_data: record.structured_data,
        fallback_source: record.fallback_source,
        template_fields: record.template_fields.values,
    }
}

async fn map_open_graph(
    record: SeoTargetOpenGraphRecord,
    tenant_id: Uuid,
    locale: &str,
    media_port: Option<&dyn MediaAssetReadPort>,
) -> SeoOpenGraph {
    SeoOpenGraph {
        title: record.title,
        description: record.description,
        kind: record.kind,
        site_name: record.site_name,
        url: record.url,
        locale: record.locale,
        images: {
            let mut images = Vec::with_capacity(record.images.len());
            for image in record.images {
                let descriptor = match (media_port, image.media_asset_id) {
                    (Some(port), Some(media_id)) => port
                        .get_image_descriptor(
                            PortContext::new(
                                tenant_id.to_string(),
                                PortActor::service("rustok-seo.image-descriptor"),
                                locale,
                                format!("seo-media-image:{media_id}"),
                            )
                            .with_deadline(Duration::from_secs(2)),
                            media_id,
                            image.alt.clone(),
                        )
                        .await
                        .ok()
                        .flatten(),
                    _ => None,
                };
                images.push(SeoImageAsset {
                    url: descriptor
                        .as_ref()
                        .map(|value| value.url.clone())
                        .unwrap_or(image.url),
                    alt: descriptor
                        .as_ref()
                        .and_then(|value| value.alt.clone())
                        .or(image.alt),
                    width: descriptor
                        .as_ref()
                        .and_then(|value| value.width)
                        .or(image.width),
                    height: descriptor
                        .as_ref()
                        .and_then(|value| value.height)
                        .or(image.height),
                    mime_type: descriptor
                        .and_then(|value| value.mime_type)
                        .or(image.mime_type),
                });
            }
            images
        },
    }
}
