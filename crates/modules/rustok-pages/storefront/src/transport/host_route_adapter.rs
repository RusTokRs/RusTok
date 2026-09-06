use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorefrontPageRouteDisposition {
    Canonical,
    Redirect,
    Gone,
    NotFound,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontPageRouteDecision {
    pub disposition: StorefrontPageRouteDisposition,
    pub canonical_path: Option<String>,
    pub canonical_slug: Option<String>,
    pub canonical_page_id: Option<String>,
    pub canonical_locale: Option<String>,
    pub channel_id: Option<String>,
    pub route_generation: Option<u64>,
    pub page_generation: Option<u64>,
    pub artifact_generation: Option<u64>,
}

#[cfg(feature = "ssr")]
impl StorefrontPageRouteDecision {
    fn terminal(disposition: StorefrontPageRouteDisposition) -> Self {
        Self {
            disposition,
            canonical_path: None,
            canonical_slug: None,
            canonical_page_id: None,
            canonical_locale: None,
            channel_id: None,
            route_generation: None,
            page_generation: None,
            artifact_generation: None,
        }
    }
}

pub async fn resolve_storefront_page_route(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPageRouteDecision, ApiError> {
    storefront_page_route_native(configured_tenant_slug(), page_slug, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "pages/route-decision")]
async fn storefront_page_route_native(
    tenant_slug: Option<String>,
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPageRouteDecision, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{HostRuntimeContext, PLATFORM_FALLBACK_LOCALE, build_locale_candidates};
        use rustok_channel::ChannelService;
        use rustok_content::entities::node::ContentStatus;
        use rustok_core::SecurityContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_pages::{
            PAGE_ROUTE_NOT_FOUND, PAGE_ROUTE_RESOLUTION_CONFLICT, PageRouteDisposition,
            PageRouteService, PageService, PagesCacheReadRuntime, PagesError,
        };
        use rustok_tenant::TenantService;

        const MODULE_SLUG: &str = "pages";

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "pages/route-decision requires TransactionalEventBus in host runtime context",
                )
            })?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .ok();

        let (tenant_id, fallback_locale) = if let Some(tenant) = tenant_context.as_ref() {
            (
                tenant.id,
                normalize_tenant_fallback_locale(tenant.default_locale.as_str()),
            )
        } else {
            let slug = tenant_slug
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ServerFnError::new(
                        "pages/route-decision requires tenant context or tenant slug",
                    )
                })?;
            let tenant = TenantService::new(runtime_ctx.db_clone())
                .get_tenant_by_slug(slug)
                .await
                .map_err(ServerFnError::new)?;
            (
                tenant.id,
                normalize_tenant_fallback_locale(tenant.default_locale.as_str()),
            )
        };

        let channel_id = request_context
            .as_ref()
            .and_then(|context| context.channel_id);
        if let Some(channel_id) = channel_id {
            let enabled = ChannelService::new(runtime_ctx.db_clone())
                .is_module_enabled(channel_id, MODULE_SLUG)
                .await
                .map_err(ServerFnError::new)?;
            if !enabled {
                return Ok(StorefrontPageRouteDecision::terminal(
                    StorefrontPageRouteDisposition::NotFound,
                ));
            }
        }

        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.as_str()))
            .unwrap_or(fallback_locale.as_str());
        let locale_candidates = build_locale_candidates(
            [
                Some(requested_locale),
                Some(fallback_locale.as_str()),
                Some(PLATFORM_FALLBACK_LOCALE),
            ],
            true,
        );
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_channel_slug(ctx.channel_slug.as_deref()));
        let route_service = PageRouteService::new(runtime_ctx.db_clone());

        for candidate_locale in locale_candidates {
            let resolution = match route_service
                .resolve(tenant_id, candidate_locale.as_str(), page_slug.as_str())
                .await
            {
                Ok(resolution) => resolution,
                Err(error) if page_error_code(&error) == Some(PAGE_ROUTE_NOT_FOUND) => continue,
                Err(error) if page_error_code(&error) == Some(PAGE_ROUTE_RESOLUTION_CONFLICT) => {
                    return Ok(StorefrontPageRouteDecision::terminal(
                        StorefrontPageRouteDisposition::Conflict,
                    ));
                }
                Err(error) => return Err(ServerFnError::new(error.to_string())),
            };

            if resolution.disposition == PageRouteDisposition::Gone {
                return Ok(StorefrontPageRouteDecision::terminal(
                    StorefrontPageRouteDisposition::Gone,
                ));
            }

            let canonical = resolution.canonical.ok_or_else(|| {
                ServerFnError::new(
                    "Pages canonical or redirect route decision is missing its canonical target",
                )
            })?;
            let page = match PageService::new(runtime_ctx.db_clone(), event_bus.clone())
                .get_with_locale_fallback(
                    tenant_id,
                    SecurityContext::public_read(),
                    canonical.page_id,
                    canonical.locale.as_str(),
                    None,
                )
                .await
            {
                Ok(page) => page,
                Err(PagesError::PageNotFound(_)) | Err(PagesError::Forbidden(_)) => {
                    return Ok(StorefrontPageRouteDecision::terminal(
                        StorefrontPageRouteDisposition::NotFound,
                    ));
                }
                Err(error) => return Err(ServerFnError::new(error.to_string())),
            };
            if page.status != ContentStatus::Published
                || !is_visible_for_public_channel(
                    page.channel_slugs.as_slice(),
                    public_channel_slug.as_deref(),
                )
            {
                return Ok(StorefrontPageRouteDecision::terminal(
                    StorefrontPageRouteDisposition::NotFound,
                ));
            }

            let disposition = if resolution.disposition == PageRouteDisposition::Redirect
                || candidate_locale != requested_locale
            {
                StorefrontPageRouteDisposition::Redirect
            } else {
                StorefrontPageRouteDisposition::Canonical
            };
            let generations =
                if let Some(cache_runtime) = runtime_ctx.shared_get::<PagesCacheReadRuntime>() {
                    match cache_runtime.generation_snapshot(tenant_id).await {
                        Ok(generations) => Some(generations),
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                %tenant_id,
                                "Pages host route generation read failed; composition ETag disabled"
                            );
                            None
                        }
                    }
                } else {
                    None
                };
            return Ok(StorefrontPageRouteDecision {
                disposition,
                canonical_path: Some(encoded_page_route_path(
                    canonical.locale.as_str(),
                    canonical.slug.as_str(),
                )),
                canonical_slug: Some(canonical.slug),
                canonical_page_id: Some(canonical.page_id.to_string()),
                canonical_locale: Some(canonical.locale),
                channel_id: channel_id.map(|value| value.to_string()),
                route_generation: generations.map(|value| value.route),
                page_generation: generations.map(|value| value.page),
                artifact_generation: generations.map(|value| value.artifact),
            });
        }

        Ok(StorefrontPageRouteDecision::terminal(
            StorefrontPageRouteDisposition::NotFound,
        ))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_slug, page_slug, locale);
        Err(ServerFnError::new(
            "pages/route-decision requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn page_error_code(error: &rustok_pages::PagesError) -> Option<&str> {
    match error {
        rustok_pages::PagesError::Rich(error) => error.error_code.as_deref(),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
fn normalize_tenant_fallback_locale(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        rustok_api::PLATFORM_FALLBACK_LOCALE.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(feature = "ssr")]
fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
fn is_visible_for_public_channel(
    channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(public_channel_slug) = public_channel_slug else {
        return false;
    };
    channel_slugs
        .iter()
        .any(|slug| slug.eq_ignore_ascii_case(public_channel_slug))
}

#[cfg(feature = "ssr")]
fn encoded_page_route_path(locale: &str, slug: &str) -> String {
    format!(
        "/{locale}/modules/pages?slug={}",
        form_urlencode_component(slug)
    )
}

#[cfg(feature = "ssr")]
fn form_urlencode_component(value: &str) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail"),
        }
    }
    encoded
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::{encoded_page_route_path, form_urlencode_component};

    #[test]
    fn host_route_location_percent_encodes_unicode_and_reserved_bytes() {
        assert_eq!(
            encoded_page_route_path("pt-BR", "sobre nós/ç"),
            "/pt-BR/modules/pages?slug=sobre+n%C3%B3s%2F%C3%A7"
        );
        assert_eq!(form_urlencode_component("a?b=c"), "a%3Fb%3Dc");
    }
}
