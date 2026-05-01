use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_commerce_foundation::dto::{ProductResponse, ProductTranslationResponse};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_seo_targets::{
    builtin_slug, schema, SeoBulkSummaryRecord, SeoLoadedTargetRecord, SeoRouteMatchRecord,
    SeoSitemapCandidateRecord, SeoTargetAlternateRoute, SeoTargetBulkListRequest,
    SeoTargetCapabilities, SeoTargetImageRecord, SeoTargetLoadRequest, SeoTargetLoadScope,
    SeoTargetOpenGraphRecord, SeoTargetProvider, SeoTargetRouteResolveRequest,
    SeoTargetRuntimeContext, SeoTargetSitemapRequest, SeoTargetSlug, SeoTemplateFieldMap,
};
use url::Url;

use crate::{CatalogService, StorefrontProductListItem};

const BULK_FETCH_SIZE: u64 = 48;

#[derive(Clone, Default)]
pub struct ProductSeoTargetProvider;

#[async_trait]
impl SeoTargetProvider for ProductSeoTargetProvider {
    fn slug(&self) -> SeoTargetSlug {
        SeoTargetSlug::new(builtin_slug::PRODUCT).expect("builtin SEO target slug must stay valid")
    }

    fn display_name(&self) -> &'static str {
        "Product"
    }

    fn owner_module_slug(&self) -> &'static str {
        "product"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, true, true)
    }

    async fn load_target(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        let service = CatalogService::new(runtime.db.clone(), runtime.event_bus.clone());
        let product = service
            .get_product_with_locale_fallback(
                request.tenant_id,
                request.target_id,
                request.locale,
                Some(request.default_locale),
            )
            .await
            .ok();
        let Some(product) = product else {
            return Ok(None);
        };

        if matches!(request.scope, SeoTargetLoadScope::PublicRoute)
            && (product.status != ProductStatus::Active || product.published_at.is_none())
        {
            return Ok(None);
        }

        Ok(Some(map_product_response(
            product,
            request.locale,
            request.default_locale,
        )))
    }

    async fn resolve_route(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetRouteResolveRequest<'_>,
    ) -> AnyResult<Option<SeoRouteMatchRecord>> {
        let Some(handle) = parse_product_route(request.route)? else {
            return Ok(None);
        };
        let service = CatalogService::new(runtime.db.clone(), runtime.event_bus.clone());
        let product = service
            .get_published_product_by_handle_with_locale_fallback(
                request.tenant_id,
                handle.as_str(),
                request.locale,
                Some(request.default_locale),
                request.channel_slug,
            )
            .await?;

        Ok(product.map(|product| SeoRouteMatchRecord {
            target_kind: self.slug(),
            target_id: product.id,
        }))
    }

    async fn list_bulk_summaries(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetBulkListRequest<'_>,
    ) -> AnyResult<Vec<SeoBulkSummaryRecord>> {
        let service = CatalogService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page = 1_u64;
        let mut summaries = Vec::new();

        loop {
            let list = service
                .list_published_products_with_locale_fallback(
                    request.tenant_id,
                    request.locale,
                    Some(request.default_locale),
                    None,
                    page,
                    BULK_FETCH_SIZE,
                )
                .await?;
            if list.items.is_empty() {
                break;
            }
            for item in list.items {
                if let Some(summary) = load_product_summary(
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
            if !list.has_next {
                break;
            }
            page += 1;
        }

        Ok(summaries)
    }

    async fn sitemap_candidates(
        &self,
        runtime: &SeoTargetRuntimeContext,
        request: SeoTargetSitemapRequest<'_>,
    ) -> AnyResult<Vec<SeoSitemapCandidateRecord>> {
        let service = CatalogService::new(runtime.db.clone(), runtime.event_bus.clone());
        let mut page = 1_u64;
        let mut candidates = Vec::new();

        loop {
            let list = service
                .list_published_products_with_locale_fallback(
                    request.tenant_id,
                    request.default_locale,
                    Some(request.default_locale),
                    None,
                    page,
                    BULK_FETCH_SIZE,
                )
                .await?;
            if list.items.is_empty() {
                break;
            }
            for item in list.items {
                if let Some(candidate) = load_product_sitemap_candidate(
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
            if !list.has_next {
                break;
            }
            page += 1;
        }

        Ok(candidates)
    }
}

async fn load_product_summary(
    service: &CatalogService,
    tenant_id: uuid::Uuid,
    locale: &str,
    default_locale: &str,
    item: StorefrontProductListItem,
) -> AnyResult<Option<SeoBulkSummaryRecord>> {
    let product = service
        .get_product_with_locale_fallback(tenant_id, item.id, locale, Some(default_locale))
        .await
        .ok();
    let Some(product) = product else {
        return Ok(None);
    };
    let mapped = map_product_response(product, locale, default_locale);
    Ok(Some(SeoBulkSummaryRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        effective_locale: mapped.effective_locale,
        label: mapped.title,
        route: mapped.canonical_route,
    }))
}

async fn load_product_sitemap_candidate(
    service: &CatalogService,
    tenant_id: uuid::Uuid,
    default_locale: &str,
    item: StorefrontProductListItem,
) -> AnyResult<Option<SeoSitemapCandidateRecord>> {
    let product = service
        .get_product_with_locale_fallback(tenant_id, item.id, default_locale, Some(default_locale))
        .await
        .ok();
    let Some(product) = product else {
        return Ok(None);
    };
    let mapped = map_product_response(product, default_locale, default_locale);
    Ok(Some(SeoSitemapCandidateRecord {
        target_kind: mapped.target_kind,
        target_id: mapped.target_id,
        locale: mapped.effective_locale,
        route: mapped.canonical_route,
    }))
}

fn map_product_response(
    product: ProductResponse,
    requested_locale: &str,
    default_locale: &str,
) -> SeoLoadedTargetRecord {
    let (translation, effective_locale) = resolve_product_translation(
        product.translations.as_slice(),
        requested_locale,
        Some(default_locale),
    );
    let translation = translation
        .cloned()
        .unwrap_or_else(|| ProductTranslationResponse {
            locale: default_locale.to_string(),
            title: "Untitled product".to_string(),
            handle: String::new(),
            description: None,
            meta_title: None,
            meta_description: None,
        });
    let title = translation
        .meta_title
        .clone()
        .unwrap_or_else(|| translation.title.clone());
    let description = translation
        .meta_description
        .clone()
        .or_else(|| translation.description.clone())
        .or_else(|| summarize_text(translation.title.as_str()));
    let image = product.images.first().cloned();
    let canonical_route = format!("/modules/product?handle={}", translation.handle);
    let mut template_fields = SeoTemplateFieldMap::default();
    template_fields.insert("title", title.clone());
    template_fields.insert("description", description.clone().unwrap_or_default());
    template_fields.insert("handle", translation.handle.clone());
    template_fields.insert("locale", effective_locale.clone());
    template_fields.insert("route", canonical_route.clone());

    SeoLoadedTargetRecord {
        target_kind: SeoTargetSlug::new(builtin_slug::PRODUCT)
            .expect("builtin SEO target slug must stay valid"),
        target_id: product.id,
        requested_locale: Some(requested_locale.to_string()),
        effective_locale: effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_route,
        alternates: product
            .translations
            .iter()
            .map(|item| SeoTargetAlternateRoute {
                locale: item.locale.clone(),
                route: format!("/modules/product?handle={}", item.handle),
            })
            .collect(),
        open_graph: SeoTargetOpenGraphRecord {
            title: Some(title.clone()),
            description: description.clone(),
            kind: Some("product".to_string()),
            site_name: None,
            url: None,
            locale: None,
            images: image
                .map(|image| {
                    vec![SeoTargetImageRecord {
                        url: image.url,
                        alt: image.alt_text,
                        width: None,
                        height: None,
                        mime_type: None,
                    }]
                })
                .unwrap_or_default(),
        },
        structured_data: schema::product(
            translation.title.as_str(),
            translation.description.as_deref(),
            product.images.first().map(|image| image.url.as_str()),
            effective_locale.as_str(),
        ),
        fallback_source: "product".to_string(),
        template_fields,
    }
}

fn resolve_product_translation<'a>(
    items: &'a [ProductTranslationResponse],
    requested: &str,
    fallback: Option<&str>,
) -> (Option<&'a ProductTranslationResponse>, String) {
    let candidates =
        rustok_core::build_locale_candidates([Some(requested), fallback, Some("en")], true);
    for candidate in candidates {
        if let Some(item) = items.iter().find(|item| {
            rustok_core::normalize_locale_tag(item.locale.as_str())
                .is_some_and(|locale| locale == candidate)
        }) {
            return (Some(item), candidate);
        }
    }

    (
        items.first(),
        items
            .first()
            .and_then(|item| rustok_core::normalize_locale_tag(item.locale.as_str()))
            .unwrap_or_else(|| requested.to_string()),
    )
}

fn parse_product_route(route: &str) -> AnyResult<Option<String>> {
    let parsed = Url::parse(format!("https://rustok.local{route}").as_str())?;
    if !matches_module_path(&parsed, "product") {
        return Ok(None);
    }
    Ok(parsed
        .query_pairs()
        .find(|(key, _)| key == "handle")
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty()))
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
