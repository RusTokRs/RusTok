use std::collections::HashMap;

use rustok_api::TenantContext;
use rustok_content::resolve_by_locale_with_fallback;
use rustok_seo_targets::{SeoTargetBulkListRequest, SeoTargetSlug};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use uuid::Uuid;

use crate::dto::{SeoBulkItem, SeoBulkListInput, SeoBulkPage, SeoBulkSource, SeoModuleSettings};
use crate::entities::{self as seo_meta, meta_translation};
use crate::{SeoError, SeoResult};

use super::robots::first_open_graph_image_url;
use super::templates::render_generated_record;
use super::{LoadedMeta, SeoService, TargetState, trimmed_option};

const MAX_BULK_PAGE_SIZE: i32 = 100;
const BULK_META_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub(super) struct BulkReadFilter {
    pub target_kind: SeoTargetSlug,
    pub locale: String,
    pub query: Option<String>,
    pub source: SeoBulkSource,
}

#[derive(Debug, Clone)]
pub(super) struct BulkReadProjection {
    pub effective_locale: String,
    pub source: SeoBulkSource,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub canonical_url: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub structured_data: Option<Value>,
    pub noindex: bool,
    pub nofollow: bool,
}

#[derive(Debug, Clone)]
pub(super) struct BulkReadRow {
    pub target_id: Uuid,
    pub label: String,
    pub route: String,
    pub projection: BulkReadProjection,
}

impl SeoService {
    pub(super) async fn collect_bulk_read_rows(
        &self,
        tenant: &TenantContext,
        filter: &BulkReadFilter,
    ) -> SeoResult<Vec<BulkReadRow>> {
        if !self.is_enabled(tenant.id).await? {
            return Ok(Vec::new());
        }

        let Some(provider) = self.registry.get(&filter.target_kind) else {
            return Ok(Vec::new());
        };
        let summaries = provider
            .list_bulk_summaries(
                &self.target_runtime(),
                SeoTargetBulkListRequest {
                    tenant_id: tenant.id,
                    default_locale: tenant.default_locale.as_str(),
                    locale: filter.locale.as_str(),
                },
            )
            .await
            .map_err(|error| {
                SeoError::validation(format!(
                    "SEO target provider `{}` failed to collect bulk summaries: {error}",
                    filter.target_kind.as_str()
                ))
            })?;
        let target_ids = summaries
            .iter()
            .map(|summary| summary.target_id)
            .collect::<Vec<_>>();
        let mut explicit_by_target = self
            .load_bulk_explicit_meta_batches(
                tenant.id,
                filter.target_kind.as_str(),
                target_ids.as_slice(),
            )
            .await?;
        let settings = self.load_settings(tenant.id).await?;
        let mut rows = Vec::with_capacity(summaries.len());

        for summary in summaries {
            let state = self
                .load_target_state(
                    tenant,
                    filter.target_kind.clone(),
                    summary.target_id,
                    filter.locale.as_str(),
                )
                .await?;
            let explicit = explicit_by_target.remove(&summary.target_id);
            let projection = resolve_bulk_read_projection(
                tenant,
                filter.target_kind.clone(),
                summary.target_id,
                filter.locale.as_str(),
                explicit,
                state,
                &settings,
            )
            .ok_or(SeoError::NotFound)?;

            if filter.source != SeoBulkSource::Any && filter.source != projection.source {
                continue;
            }
            if let Some(query) = filter.query.as_deref() {
                let haystacks = [
                    summary.label.to_ascii_lowercase(),
                    summary.route.to_ascii_lowercase(),
                    summary.target_id.to_string().to_ascii_lowercase(),
                ];
                if !haystacks.iter().any(|value| value.contains(query)) {
                    continue;
                }
            }
            rows.push(BulkReadRow {
                target_id: summary.target_id,
                label: summary.label,
                route: summary.route,
                projection,
            });
        }

        Ok(rows)
    }

    pub(super) async fn list_bulk_items_batched(
        &self,
        tenant: &TenantContext,
        input: SeoBulkListInput,
    ) -> SeoResult<SeoBulkPage> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, MAX_BULK_PAGE_SIZE);
        let filter = BulkReadFilter {
            target_kind: input.target_kind,
            locale: super::normalize_effective_locale(
                input.locale.as_str(),
                tenant.default_locale.as_str(),
            )?,
            query: input
                .query
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty()),
            source: input.source.unwrap_or(SeoBulkSource::Any),
        };
        let rows = self.collect_bulk_read_rows(tenant, &filter).await?;
        let total = rows.len() as i32;
        let offset = ((page - 1) * per_page) as usize;
        let items = rows
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .map(|row| SeoBulkItem {
                target_kind: filter.target_kind.clone(),
                target_id: row.target_id,
                locale: filter.locale.clone(),
                effective_locale: row.projection.effective_locale,
                label: row.label,
                route: row.route,
                source: row.projection.source,
                title: row.projection.title,
                description: row.projection.description,
                canonical_url: row.projection.canonical_url,
                noindex: row.projection.noindex,
                nofollow: row.projection.nofollow,
            })
            .collect();

        Ok(SeoBulkPage {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn load_bulk_explicit_meta_batches(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, LoadedMeta>> {
        let mut loaded = HashMap::new();
        for target_ids in target_ids.chunks(BULK_META_BATCH_SIZE) {
            let metas = seo_meta::Entity::find()
                .filter(seo_meta::Column::TenantId.eq(tenant_id))
                .filter(seo_meta::Column::TargetType.eq(target_kind))
                .filter(seo_meta::Column::TargetId.is_in(target_ids.iter().copied()))
                .all(&self.db)
                .await?;
            if metas.is_empty() {
                continue;
            }

            let meta_ids = metas.iter().map(|meta| meta.id).collect::<Vec<_>>();
            let translations = meta_translation::Entity::find()
                .filter(meta_translation::Column::MetaId.is_in(meta_ids))
                .order_by_asc(meta_translation::Column::MetaId)
                .order_by_asc(meta_translation::Column::Locale)
                .all(&self.db)
                .await?;
            let mut translations_by_meta = HashMap::<Uuid, Vec<meta_translation::Model>>::new();
            for translation in translations {
                translations_by_meta
                    .entry(translation.meta_id)
                    .or_default()
                    .push(translation);
            }

            for meta in metas {
                loaded.insert(
                    meta.target_id,
                    LoadedMeta {
                        translations: translations_by_meta.remove(&meta.id).unwrap_or_default(),
                        meta,
                    },
                );
            }
        }
        Ok(loaded)
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_bulk_read_projection(
    tenant: &TenantContext,
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    requested_locale: &str,
    explicit: Option<LoadedMeta>,
    state: Option<TargetState>,
    settings: &SeoModuleSettings,
) -> Option<BulkReadProjection> {
    match (explicit, state) {
        (Some(explicit), Some(state)) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                state.effective_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.title.clone()))
                    .or(Some(state.title)),
                description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.description.clone()))
                    .or(state.description),
                keywords: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.keywords.clone())),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_title.clone())),
                og_description: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_description.clone())),
                og_image: translation
                    .as_ref()
                    .and_then(|item| trimmed_option(item.og_image.clone())),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (Some(explicit), None) => {
            let resolved = resolve_by_locale_with_fallback(
                explicit.translations.as_slice(),
                requested_locale,
                Some(tenant.default_locale.as_str()),
                |item| item.locale.as_str(),
            );
            let translation = resolved.item.cloned();
            Some(BulkReadProjection {
                effective_locale: resolved.effective_locale,
                source: SeoBulkSource::Explicit,
                title: translation.as_ref().and_then(|item| item.title.clone()),
                description: translation
                    .as_ref()
                    .and_then(|item| item.description.clone()),
                keywords: translation.as_ref().and_then(|item| item.keywords.clone()),
                canonical_url: explicit.meta.canonical_url,
                og_title: translation.as_ref().and_then(|item| item.og_title.clone()),
                og_description: translation
                    .as_ref()
                    .and_then(|item| item.og_description.clone()),
                og_image: translation.as_ref().and_then(|item| item.og_image.clone()),
                structured_data: explicit.meta.structured_data,
                noindex: explicit.meta.no_index,
                nofollow: explicit.meta.no_follow,
            })
        }
        (None, Some(state)) => {
            debug_assert_eq!(state.target_kind, target_kind);
            debug_assert_eq!(state.target_id, target_id);
            let generated = render_generated_record(
                &state,
                &settings.template_defaults,
                settings.template_overrides.get(state.target_kind.as_str()),
            );
            let generated_source = generated.title.is_some()
                || generated.description.is_some()
                || generated.canonical_url.is_some()
                || generated.keywords.is_some()
                || generated.og_title.is_some()
                || generated.og_description.is_some()
                || generated.robots.is_some()
                || generated.twitter_title.is_some()
                || generated.twitter_description.is_some();
            Some(BulkReadProjection {
                effective_locale: state.effective_locale,
                source: if generated_source {
                    SeoBulkSource::Generated
                } else {
                    SeoBulkSource::Fallback
                },
                title: generated.title.or(Some(state.title)),
                description: generated.description.or(state.description),
                keywords: generated.keywords,
                canonical_url: generated.canonical_url,
                og_title: generated.og_title.or(state.open_graph.title.clone()),
                og_description: generated
                    .og_description
                    .or(state.open_graph.description.clone()),
                og_image: first_open_graph_image_url(&state.open_graph),
                structured_data: Some(state.structured_data),
                noindex: false,
                nofollow: false,
            })
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dto::{SeoAlternateLink, SeoOpenGraph};

    fn tenant() -> TenantContext {
        TenantContext {
            id: Uuid::new_v4(),
            name: "Tenant".to_string(),
            slug: "tenant".to_string(),
            domain: None,
            settings: json!({}),
            default_locale: "en-US".to_string(),
            is_active: true,
        }
    }

    fn state(target_kind: SeoTargetSlug, target_id: Uuid) -> TargetState {
        TargetState {
            target_kind,
            target_id,
            requested_locale: Some("en-US".to_string()),
            effective_locale: "en-US".to_string(),
            title: "Fallback title".to_string(),
            description: Some("Fallback description".to_string()),
            canonical_path: "/fallback".to_string(),
            alternates: vec![SeoAlternateLink {
                locale: "en-US".to_string(),
                href: "/en-US/fallback".to_string(),
                x_default: false,
            }],
            open_graph: SeoOpenGraph::default(),
            structured_data: json!({"@type": "WebPage"}),
            fallback_source: "target".to_string(),
            template_fields: Default::default(),
        }
    }

    #[test]
    fn fallback_projection_preserves_full_export_fields() {
        let target_kind = SeoTargetSlug::new("page").expect("valid target kind");
        let target_id = Uuid::new_v4();
        let projection = resolve_bulk_read_projection(
            &tenant(),
            target_kind.clone(),
            target_id,
            "en-US",
            None,
            Some(state(target_kind, target_id)),
            &SeoModuleSettings::default(),
        )
        .expect("projection");

        assert_eq!(projection.source, SeoBulkSource::Fallback);
        assert_eq!(projection.title.as_deref(), Some("Fallback title"));
        assert_eq!(
            projection.structured_data,
            Some(json!({"@type": "WebPage"}))
        );
    }
}
