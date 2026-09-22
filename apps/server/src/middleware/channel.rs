use axum::{
    body::Body,
    extract::State,
    http::{Extensions, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use moka::{Expiry, future::Cache};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

use crate::common::{RustokSettings, extract_effective_host, peer_ip_from_extensions};
use crate::context::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, TenantContextExt,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_api::{
    ChannelResolutionOutcome, ChannelResolutionStage, ChannelResolutionTraceStep,
    context::AuthContextExtension, request::ResolvedRequestLocale,
};
use rustok_channel::{
    ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionDecision, TargetSurface,
};

const CHANNEL_ID_HEADER: &str = "X-Channel-ID";
const CHANNEL_SLUG_HEADER: &str = "X-Channel-Slug";
const CHANNEL_CACHE_TTL: Duration = Duration::from_secs(60);
const CHANNEL_NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(10);
const CHANNEL_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const CHANNEL_CACHE_MAX_TENANT_VERSIONS: usize = 16 * 1024;

struct ChannelCacheVersionState {
    next_version: u64,
    default_version: u64,
    tenant_versions: HashMap<Uuid, u64>,
    exhausted: bool,
}

impl Default for ChannelCacheVersionState {
    fn default() -> Self {
        Self {
            next_version: 1,
            default_version: 1,
            tenant_versions: HashMap::new(),
            exhausted: false,
        }
    }
}

impl ChannelCacheVersionState {
    fn token(&self, tenant_id: Uuid) -> Option<u64> {
        if self.exhausted {
            return None;
        }
        Some(
            self.tenant_versions
                .get(&tenant_id)
                .copied()
                .unwrap_or(self.default_version),
        )
    }

    fn invalidate(&mut self, tenant_id: Uuid, maximum_tenants: usize) -> bool {
        if self.exhausted {
            return true;
        }

        let Some(next_version) = self.next_version.checked_add(1) else {
            self.exhausted = true;
            self.tenant_versions.clear();
            return true;
        };
        self.next_version = next_version;

        if !self.tenant_versions.contains_key(&tenant_id)
            && self.tenant_versions.len() >= maximum_tenants.max(1)
        {
            self.default_version = next_version;
            self.tenant_versions.clear();
            self.tenant_versions.insert(tenant_id, next_version);
            return true;
        }

        self.tenant_versions.insert(tenant_id, next_version);
        false
    }

    fn invalidate_all(&mut self) {
        if self.exhausted {
            self.tenant_versions.clear();
            return;
        }

        let Some(next_version) = self.next_version.checked_add(1) else {
            self.exhausted = true;
            self.tenant_versions.clear();
            return;
        };
        self.next_version = next_version;
        self.default_version = next_version;
        self.tenant_versions.clear();
    }
}

#[derive(Clone)]
struct ChannelResolutionCache {
    cache: Cache<ChannelCacheKey, CachedChannelResolution>,
    versions: Arc<Mutex<ChannelCacheVersionState>>,
    max_tenant_versions: usize,
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
    Found(Box<ChannelContext>),
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
                Self::Found(Box::new(ChannelContext {
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
                }))
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
        Self::with_max_tenant_versions(CHANNEL_CACHE_MAX_TENANT_VERSIONS)
    }

    fn with_max_tenant_versions(max_tenant_versions: usize) -> Self {
        Self {
            cache: Cache::builder()
                .expire_after(ChannelCacheExpiry)
                .weigher(channel_cache_entry_weight)
                .max_capacity(CHANNEL_CACHE_MAX_WEIGHT_BYTES)
                .build(),
            versions: Arc::new(Mutex::new(ChannelCacheVersionState::default())),
            max_tenant_versions: max_tenant_versions.max(1),
        }
    }

    fn ttl_for(value: &CachedChannelResolution) -> Duration {
        match value {
            CachedChannelResolution::Found(_) => CHANNEL_CACHE_TTL,
            CachedChannelResolution::Missing => CHANNEL_NEGATIVE_CACHE_TTL,
        }
    }

    fn tenant_version(&self, tenant_id: Uuid) -> Option<u64> {
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .token(tenant_id)
    }

    async fn invalidate_tenant(&self, tenant_id: Uuid) {
        let invalidate_all = self
            .versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invalidate(tenant_id, self.max_tenant_versions);
        if invalidate_all {
            self.cache.invalidate_all();
            self.cache.run_pending_tasks().await;
        }
    }

    async fn invalidate_all(&self) {
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invalidate_all();
        self.cache.invalidate_all();
        self.cache.run_pending_tasks().await;
    }

    #[cfg(test)]
    fn tracked_tenant_versions(&self) -> usize {
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tenant_versions
            .len()
    }

    #[cfg(test)]
    fn exhaust_versions(&self) {
        let mut versions = self
            .versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        versions.next_version = u64::MAX;
    }
}

fn channel_cache_entry_weight(key: &ChannelCacheKey, value: &CachedChannelResolution) -> u32 {
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
    let candidate = Arc::new(ChannelResolutionCache::new());
    let _ = ctx.shared_insert_if_absent(candidate.clone());
    ctx.shared_get::<Arc<ChannelResolutionCache>>()
        .unwrap_or(candidate)
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

    let cached = if let Some(version) = cache.tenant_version(facts.tenant_id) {
        let cache_key = channel_cache_key_from_facts(&facts, version);
        cache
            .cache
            .try_get_with(cache_key, async move {
                resolver
                    .resolve(&facts)
                    .await
                    .map(CachedChannelResolution::from_decision)
                    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
            })
            .await
            .map_err(|error| *error)?
    } else {
        resolver
            .resolve(&facts)
            .await
            .map(CachedChannelResolution::from_decision)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    };

    if let CachedChannelResolution::Found(context) = cached {
        req.extensions_mut()
            .insert(ChannelContextExtension(*context));
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
    format!("sha256-{}", hex::encode(Sha256::digest(value.as_bytes())))
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

pub async fn invalidate_all_channel_cache(ctx: &ServerRuntimeContext) {
    channel_cache(ctx).invalidate_all().await;
}

#[cfg(test)]
mod version_registry_tests {
    use super::ChannelResolutionCache;
    use uuid::Uuid;

    #[tokio::test]
    async fn tenant_version_registry_rotates_without_reusing_stale_tokens() {
        let cache = ChannelResolutionCache::with_max_tenant_versions(2);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        let initial_first = cache.tenant_version(first);

        cache.invalidate_tenant(first).await;
        let invalidated_first = cache.tenant_version(first);
        cache.invalidate_tenant(second).await;
        cache.invalidate_tenant(third).await;

        assert_ne!(initial_first, invalidated_first);
        assert_ne!(initial_first, cache.tenant_version(first));
        assert!(cache.tracked_tenant_versions() <= 2);
        assert!(cache.tenant_version(third).is_some());
    }

    #[tokio::test]
    async fn namespace_invalidation_rotates_default_and_tracked_tokens() {
        let cache = ChannelResolutionCache::with_max_tenant_versions(2);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        cache.invalidate_tenant(first).await;
        let first_before = cache.tenant_version(first);
        let second_before = cache.tenant_version(second);

        cache.invalidate_all().await;

        assert_ne!(first_before, cache.tenant_version(first));
        assert_ne!(second_before, cache.tenant_version(second));
        assert_eq!(cache.tracked_tenant_versions(), 0);
    }

    #[tokio::test]
    async fn repeated_tenant_invalidation_does_not_grow_the_registry() {
        let cache = ChannelResolutionCache::with_max_tenant_versions(2);
        let tenant = Uuid::new_v4();
        for _ in 0..32 {
            cache.invalidate_tenant(tenant).await;
        }
        assert_eq!(cache.tracked_tenant_versions(), 1);
    }

    #[tokio::test]
    async fn version_exhaustion_disables_cache_instead_of_reusing_a_token() {
        let cache = ChannelResolutionCache::with_max_tenant_versions(2);
        let tenant = Uuid::new_v4();
        cache.exhaust_versions();

        cache.invalidate_tenant(tenant).await;

        assert!(cache.tenant_version(tenant).is_none());
        assert_eq!(cache.tracked_tenant_versions(), 0);
    }
}

#[cfg(test)]
#[path = "channel_tests.rs"]
mod tests;
