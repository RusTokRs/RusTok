use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, StatusCode},
};
use rustok_core::{extract_locale_tag_from_header, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};
use uuid::Uuid;

use crate::context::{ChannelContextExtension, ChannelResolutionSource, TenantContextExtension};

const ADMIN_LOCALE_COOKIE: &str = "rustok-admin-locale";
const MEDUSA_LOCALE_HEADER: &str = "x-medusa-locale";

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub channel_resolution_source: Option<ChannelResolutionSource>,
    pub locale: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedRequestLocale {
    pub requested_locale: Option<String>,
    pub effective_locale: String,
}

impl RequestContext {
    pub fn require_user(&self) -> Result<Uuid, (StatusCode, &'static str)> {
        self.user_id
            .ok_or((StatusCode::UNAUTHORIZED, "Authentication required"))
    }
}

impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let tenant_context = parts
            .extensions
            .get::<TenantContextExtension>()
            .map(|ext| &ext.0);

        let tenant_id = tenant_context
            .map(|tenant| tenant.id)
            .or_else(|| {
                parts
                    .headers
                    .get("X-Tenant-ID")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| Uuid::parse_str(value).ok())
            })
            .ok_or((StatusCode::BAD_REQUEST, "X-Tenant-ID header required"))?;

        let user_id = parts
            .headers
            .get("X-User-ID")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok());

        let locale = parts
            .extensions
            .get::<ResolvedRequestLocale>()
            .map(|resolved| resolved.effective_locale.clone())
            .unwrap_or_else(|| {
                resolve_request_locale(
                    parts,
                    tenant_context.map(|tenant| tenant.default_locale.as_str()),
                )
                .effective_locale
            });
        let channel_context = parts
            .extensions
            .get::<ChannelContextExtension>()
            .map(|ext| &ext.0);

        Ok(RequestContext {
            tenant_id,
            user_id,
            channel_id: channel_context.map(|channel| channel.id),
            channel_slug: channel_context.map(|channel| channel.slug.clone()),
            channel_resolution_source: channel_context
                .map(|channel| channel.resolution_source.clone()),
            locale,
        })
    }
}

pub fn resolve_request_locale(
    parts: &Parts,
    tenant_default_locale: Option<&str>,
) -> ResolvedRequestLocale {
    let requested_locale = extract_requested_locale(parts);
    let effective_locale = requested_locale
        .clone()
        .or_else(|| tenant_default_locale.and_then(normalize_locale_tag))
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

    ResolvedRequestLocale {
        requested_locale,
        effective_locale,
    }
}

pub fn extract_requested_locale(parts: &Parts) -> Option<String> {
    extract_locale_from_query(parts)
        .or_else(|| extract_locale_from_medusa_header(&parts.headers))
        .or_else(|| extract_locale_from_cookie(&parts.headers))
        .or_else(|| extract_locale_from_accept_language(&parts.headers))
}

fn extract_locale_from_query(parts: &Parts) -> Option<String> {
    parts.uri.query().and_then(|query| {
        query.split('&').find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            if key != "locale" {
                return None;
            }

            normalize_locale_tag(value)
        })
    })
}

fn extract_locale_from_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| {
            raw.split(';').find_map(|entry| {
                let (name, value) = entry.trim().split_once('=')?;
                if name != ADMIN_LOCALE_COOKIE {
                    return None;
                }

                normalize_locale_tag(value)
            })
        })
}

fn extract_locale_from_medusa_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get(MEDUSA_LOCALE_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_locale_tag)
}

fn extract_locale_from_accept_language(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| extract_locale_tag_from_header(Some(value)))
}

#[cfg(test)]
mod tests {
    use axum::http::Request;
    use tokio::runtime::Runtime;
    use uuid::Uuid;

    use crate::context::{ChannelContext, ChannelContextExtension, ChannelResolutionSource};
    use crate::context::{TenantContext, TenantContextExtension};

    use super::*;

    #[test]
    fn normalizes_accept_language_header() {
        let request = Request::builder()
            .header("X-Tenant-ID", Uuid::nil().to_string())
            .header("Accept-Language", "ru-ru,ru;q=0.9,en;q=0.8")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "ru-RU");
    }

    #[test]
    fn falls_back_to_tenant_default_locale() {
        let request = Request::builder().body(()).expect("request");
        let (mut parts, _) = request.into_parts();
        parts
            .extensions
            .insert(TenantContextExtension(TenantContext {
                id: Uuid::nil(),
                name: "Test".to_string(),
                slug: "test".to_string(),
                domain: None,
                settings: serde_json::json!({}),
                default_locale: "ru".to_string(),
                is_active: true,
            }));

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "ru");
        assert_eq!(context.tenant_id, Uuid::nil());
        assert_eq!(context.channel_id, None);
        assert_eq!(context.channel_resolution_source, None);
    }

    #[test]
    fn includes_channel_context_when_middleware_resolves_channel() {
        let request = Request::builder().body(()).expect("request");
        let (mut parts, _) = request.into_parts();
        parts
            .extensions
            .insert(TenantContextExtension(TenantContext {
                id: Uuid::nil(),
                name: "Test".to_string(),
                slug: "test".to_string(),
                domain: None,
                settings: serde_json::json!({}),
                default_locale: "en".to_string(),
                is_active: true,
            }));
        let channel_id = Uuid::new_v4();
        parts
            .extensions
            .insert(ChannelContextExtension(ChannelContext {
                id: channel_id,
                tenant_id: Uuid::nil(),
                slug: "web".to_string(),
                name: "Web".to_string(),
                is_active: true,
                status: "experimental".to_string(),
                target_type: Some("web_domain".to_string()),
                target_value: Some("example.test".to_string()),
                settings: serde_json::json!({}),
                resolution_source: ChannelResolutionSource::Host,
            }));

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.channel_id, Some(channel_id));
        assert_eq!(context.channel_slug.as_deref(), Some("web"));
        assert_eq!(
            context.channel_resolution_source,
            Some(ChannelResolutionSource::Host)
        );
    }

    #[test]
    fn prefers_query_locale_over_cookie_and_headers() {
        let request = Request::builder()
            .uri("/api/blog/posts?locale=ru")
            .header("X-Tenant-ID", Uuid::nil().to_string())
            .header("Cookie", "rustok-admin-locale=en")
            .header("Accept-Language", "de-DE,de;q=0.9")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "ru");
    }

    #[test]
    fn falls_back_to_admin_locale_cookie_before_accept_language() {
        let request = Request::builder()
            .header("X-Tenant-ID", Uuid::nil().to_string())
            .header("Cookie", "rustok-admin-locale=ru")
            .header("Accept-Language", "en-US,en;q=0.9")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "ru");
    }

    #[test]
    fn prefers_highest_quality_accept_language_value() {
        let request = Request::builder()
            .header("X-Tenant-ID", Uuid::nil().to_string())
            .header("Accept-Language", "en-US;q=0.5,ru-RU;q=0.9,de;q=0.8")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "ru-RU");
    }

    #[test]
    fn prefers_medusa_locale_header_over_cookie_and_accept_language() {
        let request = Request::builder()
            .header("X-Tenant-ID", Uuid::nil().to_string())
            .header("x-medusa-locale", "de-DE")
            .header("Cookie", "rustok-admin-locale=ru")
            .header("Accept-Language", "en-US,en;q=0.9")
            .body(())
            .expect("request");
        let (mut parts, _) = request.into_parts();

        let runtime = Runtime::new().expect("tokio runtime");
        let context = runtime
            .block_on(RequestContext::from_request_parts(&mut parts, &()))
            .expect("request context");

        assert_eq!(context.locale, "de-DE");
    }
}
