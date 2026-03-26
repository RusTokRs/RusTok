use axum::{
    body::Body,
    extract::State,
    http::{header::HOST, Request},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::context::{ChannelContext, ChannelContextExtension, TenantContextExt};
use rustok_channel::{ChannelDetailResponse, ChannelService};

const CHANNEL_ID_HEADER: &str = "X-Channel-ID";
const CHANNEL_SLUG_HEADER: &str = "X-Channel-Slug";

pub async fn resolve(
    State(ctx): State<AppContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let Some(tenant) = req.extensions().tenant_context().cloned() else {
        return Ok(next.run(req).await);
    };

    let service = ChannelService::new(ctx.db.clone());
    let selected = if let Some(channel_id) = channel_id_from_header(req.headers()) {
        match service.get_channel(channel_id).await {
            Ok(channel) if channel.tenant_id == tenant.id => Some(ChannelDetailResponse {
                channel,
                targets: Vec::new(),
                module_bindings: Vec::new(),
                oauth_apps: Vec::new(),
            }),
            _ => None,
        }
    } else if let Some(slug) = channel_slug_from_header(req.headers())
        .or_else(|| channel_slug_from_query(req.uri().query()))
    {
        match service.get_channel_by_slug(tenant.id, &slug).await {
            Ok(Some(channel)) => Some(ChannelDetailResponse {
                channel,
                targets: Vec::new(),
                module_bindings: Vec::new(),
                oauth_apps: Vec::new(),
            }),
            _ => None,
        }
    } else if let Some(host) = extract_host(req.headers()) {
        let normalized = host
            .split(':')
            .next()
            .unwrap_or(host)
            .trim()
            .to_ascii_lowercase();
        service
            .get_channel_by_target_value(tenant.id, &normalized)
            .await
            .ok()
            .flatten()
    } else {
        None
    }
    .or(service.get_default_channel(tenant.id).await.ok().flatten());

    if let Some(detail) = selected {
        let selected_target = detail
            .targets
            .iter()
            .find(|target| target.is_primary)
            .or_else(|| detail.targets.first());
        req.extensions_mut()
            .insert(ChannelContextExtension(ChannelContext {
                id: detail.channel.id,
                tenant_id: detail.channel.tenant_id,
                slug: detail.channel.slug,
                name: detail.channel.name,
                is_active: detail.channel.is_active,
                status: detail.channel.status,
                target_type: selected_target.map(|target| target.target_type.clone()),
                target_value: selected_target.map(|target| target.value.clone()),
                settings: detail.channel.settings,
            }));
    }

    Ok(next.run(req).await)
}

fn channel_id_from_header(headers: &axum::http::HeaderMap) -> Option<Uuid> {
    headers
        .get(CHANNEL_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn channel_slug_from_header(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(CHANNEL_SLUG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn channel_slug_from_query(query: Option<&str>) -> Option<String> {
    query.and_then(|query| {
        query.split('&').find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            (key == "channel" && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
    })
}

fn extract_host(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("x-forwarded-host")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .or_else(|| headers.get(HOST).and_then(|value| value.to_str().ok()))
}

#[cfg(test)]
mod tests {
    use super::{channel_id_from_header, channel_slug_from_header, channel_slug_from_query};
    use axum::http::HeaderMap;
    use uuid::Uuid;

    #[test]
    fn parses_channel_id_header() {
        let mut headers = HeaderMap::new();
        let channel_id = Uuid::new_v4();
        headers.insert(
            "X-Channel-ID",
            channel_id.to_string().parse().expect("header"),
        );

        assert_eq!(channel_id_from_header(&headers), Some(channel_id));
    }

    #[test]
    fn parses_channel_slug_from_header_and_query() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Channel-Slug", "mobile-app".parse().expect("header"));

        assert_eq!(
            channel_slug_from_header(&headers).as_deref(),
            Some("mobile-app")
        );
        assert_eq!(
            channel_slug_from_query(Some("locale=ru&channel=web-store")).as_deref(),
            Some("web-store")
        );
    }
}
