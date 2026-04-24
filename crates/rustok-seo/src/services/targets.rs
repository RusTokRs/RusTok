use uuid::Uuid;

use rustok_api::TenantContext;
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

        Ok(record.map(map_loaded_target_record))
    }

    pub(super) fn target_runtime(&self) -> SeoTargetRuntimeContext {
        SeoTargetRuntimeContext {
            db: self.db.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}

fn map_loaded_target_record(record: SeoLoadedTargetRecord) -> TargetState {
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
        open_graph: map_open_graph(record.open_graph),
        structured_data: record.structured_data,
        fallback_source: record.fallback_source,
        template_fields: record.template_fields.values,
    }
}

fn map_open_graph(record: SeoTargetOpenGraphRecord) -> SeoOpenGraph {
    SeoOpenGraph {
        title: record.title,
        description: record.description,
        kind: record.kind,
        site_name: record.site_name,
        url: record.url,
        locale: record.locale,
        images: record
            .images
            .into_iter()
            .map(|image| SeoImageAsset {
                url: image.url,
                alt: image.alt,
                width: image.width,
                height: image.height,
                mime_type: image.mime_type,
            })
            .collect(),
    }
}
