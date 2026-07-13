use axum::{
    body::Body,
    extract::State,
    http::{Extensions, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use moka::{future::Cache, Expiry};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::common::{extract_effective_host, peer_ip_from_extensions, RustokSettings};
use crate::context::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, TenantContextExt,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::{
    context::AuthContextExtension, request::ResolvedRequestLocale, ChannelResolutionOutcome,
    ChannelResolutionStage, ChannelResolutionTraceStep,
};
use rustok_channel::{
    ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionDecision, TargetSurface,
};

const CHANNEL_ID_HEADER: &str = "X-Channel-ID";
const CHANNEL_SLUG_HEADER: &str = "X-Channel-Slug";
const CHANNEL_CACHE_TTL: Duration = Duration::from_secs(60);
const CHANNEL_NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(10);
const CHANNEL_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
struct ChannelResolutionCache {
    cache: Cache<ChannelCacheKey, CachedChannelResolution>,
    tenant_versions: Arc<RwLock<HashMap<Uuid, u64>>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ChannelCacheKey {
    tenant_id: Uuid,
    version: u64,
    header_channel_id: Option<Uuid>,
    header_channel_slug: Option<String>,
    query_channel_slug: Option<String>,
    host: Option<String>,
    oauth_app_id: Option<Uuid>,
    locale: Option<String>,
}

#[derive(Clone)]
enum CachedChannelResolution {
    Found(ChannelContext),
    Missing,
}

impl CachedChannelResolution {
    fn from_decision(decision: ResolutionDecision) -> Self {
        resolved_detail_source_and_trace(decision)
            .map(|(detail, source, trace)| {
                let selected_target = detail
                    .targets
                    .iter()
                    .find(|target| target.is_primary)
                    .or_else(|| detail.targets.first());
                Self::Found(ChannelContext {
                    id: detail.channel.id,
                    tenant_id: detail.channel.tenant_id,
                    slug: detail.channel.slug,
                    name: detail.channel.name,
                    is_active: detail.channel.is_active,
                    status: detail.channel.status,
                    target_type: selected_target.map(|target| target.target_type.clone()),
                    target_value: selected_target.map(|target| target.value.clone()),
                    settings: detail.channel.settings,
                    resolution_source: source,
                    resolution_trace: trace,
                })
            })
            .unwrap_or(Self::Missing)
    }
}

struct ChannelCacheExpiry;

impl Expiry<ChannelCacheKey, CachedChannelResolution> for ChannelCacheExpiry {
    fn expire_after_create(
        &self,
        _key: &ChannelCacheKey,
        value: &CachedChannelResolution,
        _created_at: Instant,
    ) -> Option<Duration> {
        Some(ChannelResolutionCache::ttl_for(value))
    }

    fn expire_after_update(
        &self,
        _key: &ChannelCacheKey,
        value: &CachedChannelResolution,
        _updated_at: Instant,
        _duration_until_expiry: Option<Duration>,
    ) -> Option<Duration> {
        Some(ChannelResolutionCache::ttl_for(value))
    }
}

impl ChannelResolutionCache {
    fn new() -> Self {
        Self {
            cache: Cache::builder()
                .expire_after(ChannelCacheExpiry)
                .weigher(channel_cache_entry_weight)
                .max_capacity(CHANNEL_CACHE_MAX_WEIGHT_BYTES)
                .build(),
            tenant_versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn ttl_for(value: &CachedChannelResolution) -> Duration {
        match value {
            CachedChannelResolution::Found(_) => CHANNEL_CACHE_TTL,
            CachedChannelResolution::Missing => CHANNEL_NEGATIVE_CACHE_TTL,
        }
    }

    async fn tenant_version(&self, tenant_id: Uuid) -> u64 {
        self.tenant_versions
            .read()
            .await
            .get(&tenant_id)
            .copied()
            .unwrap_or(0)
    }

    async fn invalidate_tenant(&self, tenant_id: Uuid) {
        let mut versions = self.tenant_versions.write().await;
        let next_version = versions
            .get(&tenant_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        versions.insert(tenant_id, next_version);
    }
}

fn channel_cache_entry_weight(
    key: &ChannelCacheKey,
    value: &CachedChannelResolution,
) -> u32 {
    let key_strings = [
        key.header_channel_slug.as_ref(),
        key.query_channel_slug.as_ref(),
        key.host.as_ref(),
        key.locale.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(String::len)
    .sum::<usize>();
    let value_weight = match value {
        CachedChannelResolution::Found(context) => serde_json::to_vec(context)
            .map(|encoded| encoded.len())
            .unwrap_or(std::mem::size_of::<ChannelContext>()),
        CachedChannelResolution::Missing => 1,
    };
    std::mem::size_of::<ChannelCacheKey>()
        .saturating_add(key_strings)
        .saturating_add(value_weight)
        .clamp(1, u32::MAX as usize) as u32
}

fn channel_cache(ctx: &ServerRuntimeContext) -> Arc<ChannelResolutionCache> {
    if let Some(cache) = ctx.shared_get::<Arc<ChannelResolutionCache>>() {
        return cache;
    }

    let cache = Arc::new(ChannelResolutionCache::new());
    ctx.shared_insert(cache.clone());
    cache
}

pub async fn resolve(
    State(ctx): State<ServerRuntimeContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let Some(tenant) = req.extensions().tenant_context().cloned() else {
        return Ok(next.run(req).await);
    };

    let settings = ctx.settings();
    let resolver = ChannelResolver::new(ctx.db_clone());
    let facts = build_request_facts(
        tenant.id,
        req.headers(),
        req.uri().query(),
        peer_ip_from_extensions(req.extensions()),
        settings,
        req.extensions(),
    );
    let cache = channel_cache(&ctx);
    let cache_key =
        channel_cache_key_from_facts(&facts, cache.tenant_version(facts.tenant_id).await);

    let cached = cache
        .cache
        .try_get_with(cache_key, async move {
            resolver
                .resolve(&facts)
                .await
                .map(CachedChannelResolution::from_decision)
                .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        })
        .await
        .map_err(|error| *error)?;

    if let CachedChannelResolution::Found(context) = cached {
        req.extensions_mut()
            .insert(ChannelContextExtension(context));
    }

    Ok(next.run(req).await)
}

fn build_request_facts(
    tenant_id: Uuid,
    headers: &HeaderMap,
    query: Option<&str>,
    peer_ip: Option<std::net::IpAddr>,
    settings: &RustokSettings,
    extensions: &Extensions,
) -> RequestFacts {
    RequestFacts {
        tenant_id,
        surface: TargetSurface::Http,
        header_channel_id: channel_id_from_header(headers),
        header_channel_slug: channel_slug_from_header(headers),
        query_channel_slug: channel_slug_from_query(query),
        host: extract_effective_host(headers, peer_ip, &settings.runtime.request_trust),
        oauth_app_id: extensions
            .get::<AuthContextExtension>()
            .and_then(|auth| auth.0.client_id),
        locale: extensions
            .get::<ResolvedRequestLocale>()
            .map(|resolved| resolved.effective_locale.clone()),
    }
}

fn channel_cache_key_from_facts(facts: &RequestFacts, version: u64) -> ChannelCacheKey {
    ChannelCacheKey {
        tenant_id: facts.tenant_id,
        version,
        header_channel_id: facts.header_channel_id,
        header_channel_slug: facts
            .header_channel_slug
            .as_deref()
            .map(bounded_cache_component),
        query_channel_slug: facts
            .query_channel_slug
            .as_deref()
            .map(bounded_cache_component),
        host: facts.host.as_deref().map(bounded_cache_component),
        oauth_app_id: facts.oauth_app_id,
        locale: facts.locale.as_deref().map(bounded_cache_component),
    }
}

fn bounded_cache_component(value: &str) -> String {
    format!(
        "sha256-{}",
        hex::encode(Sha256::digest(value.as_bytes()))
    )
}

fn resolved_detail_source_and_trace(
    decision: ResolutionDecision,
) -> Option<(
    rustok_channel::ChannelDetailResponse,
    ChannelResolutionSource,
    Vec<ChannelResolutionTraceStep>,
)> {
    let detail = decision.detail?;
    let source = match decision.source? {
        ChannelResolutionOrigin::HeaderId => ChannelResolutionSource::HeaderId,
        ChannelResolutionOrigin::HeaderSlug => ChannelResolutionSource::HeaderSlug,
        ChannelResolutionOrigin::Query => ChannelResolutionSource::Query,
        ChannelResolutionOrigin::Host => ChannelResolutionSource::Host,
        ChannelResolutionOrigin::Policy => ChannelResolutionSource::Policy,
        ChannelResolutionOrigin::Default => ChannelResolutionSource::Default,
    };

    Some((
        detail,
        source,
        decision.trace.into_iter().map(map_trace_step).collect(),
    ))
}

fn map_trace_step(step: rustok_channel::ResolutionTraceStep) -> ChannelResolutionTraceStep {
    ChannelResolutionTraceStep {
        stage: match step.stage {
            rustok_channel::ResolutionStage::HeaderId => ChannelResolutionStage::HeaderId,
            rustok_channel::ResolutionStage::HeaderSlug => ChannelResolutionStage::HeaderSlug,
            rustok_channel::ResolutionStage::Query => ChannelResolutionStage::Query,
            rustok_channel::ResolutionStage::Host => ChannelResolutionStage::Host,
            rustok_channel::ResolutionStage::Policy => ChannelResolutionStage::Policy,
            rustok_channel::ResolutionStage::Default => ChannelResolutionStage::Default,
        },
        outcome: match step.outcome {
            rustok_channel::ResolutionOutcome::Matched => ChannelResolutionOutcome::Matched,
            rustok_channel::ResolutionOutcome::Miss => ChannelResolutionOutcome::Miss,
            rustok_channel::ResolutionOutcome::Rejected => ChannelResolutionOutcome::Rejected,
        },
        detail: step.detail,
    }
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

pub async fn invalidate_tenant_channel_cache(ctx: &ServerRuntimeContext, tenant_id: Uuid) {
    channel_cache(ctx).invalidate_tenant(tenant_id).await;
}

#[cfg(test)]
#[path = "channel_tests.rs"]
mod tests;
