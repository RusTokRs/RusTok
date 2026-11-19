import { readFileSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '../../');

function source(relative) {
  const fullPath = resolve(repoRoot, relative);
  let content = readFileSync(fullPath, 'utf8');
  const parent = dirname(fullPath);
  for (const line of content.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.startsWith('include!("') && trimmed.endsWith('");')) {
      const incFile = trimmed.slice('include!("'.length, -'");'.length);
      const incPath = resolve(parent, incFile);
      if (existsSync(incPath)) {
        content += '\n' + readFileSync(incPath, 'utf8');
      }
    } else if (trimmed.startsWith('#[path = "') && trimmed.endsWith('"]')) {
      const modFile = trimmed.slice('#[path = "'.length, -'"]'.length);
      const modPath = resolve(parent, modFile);
      if (existsSync(modPath)) {
        content += '\n' + readFileSync(modPath, 'utf8');
      }
    }
  }
  return content;
}

function assert(condition, message) {
  if (!condition) {
    console.error(`FAIL: ${message}`);
    process.exit(1);
  }
}

console.log('Verifying cache_architecture_guard invariants...');

// 1. variable_payload_caches_use_weighted_capacity
for (const rel of [
  'apps/server/src/services/field_definition_cache.rs',
  'apps/server/src/services/rbac_runtime.rs',
  'apps/server/src/middleware/locale.rs',
  'apps/server/src/middleware/channel.rs',
  'crates/modules/rustok-seo/src/services/mod.rs',
]) {
  const s = source(rel);
  assert(s.includes('.weigher('), `variable-size cache must use byte-weighted capacity: ${rel}`);
}

// 2. database_backed_hot_misses_are_coalesced
for (const rel of [
  'apps/server/src/middleware/locale.rs',
  'apps/server/src/middleware/channel.rs',
  'crates/modules/rustok-seo/src/services/redirects.rs',
]) {
  const s = source(rel);
  assert(s.includes('.try_get_with('), `database-backed cache miss must be coalesced: ${rel}`);
}

// 3. channel_cache_bounds_request_keys_and_negative_results
{
  const channel = source('apps/server/src/middleware/channel.rs');
  assert(channel.includes('bounded_cache_component'), 'request-controlled channel selectors must not be stored verbatim in cache keys');
  assert(channel.includes('Sha256::digest'), 'channel selector cache components must remain cryptographically bounded');
  assert(channel.includes('CHANNEL_NEGATIVE_CACHE_TTL'), 'missing channel resolutions must keep an independent short negative TTL');
  assert(channel.includes('ChannelCacheExpiry'), 'positive and negative channel resolutions must retain separate expirations');
}

// 4. redis_rate_limit_operations_are_bounded_and_redacted
{
  const rate_limit = source('apps/server/src/middleware/rate_limit.rs');
  assert(rate_limit.includes('RATE_LIMIT_REDIS_OPERATION_TIMEOUT'), 'Redis rate-limit connection, Lua and health operations must retain a deadline');
  assert(rate_limit.includes('redis_with_timeout('), 'Redis rate-limit operations must use the shared timeout wrapper');
  assert(rate_limit.includes('redis_rate_limit_key'), 'Redis rate-limit identities must use a bounded canonical key helper');
  assert(rate_limit.includes('Sha256::digest(identity.as_bytes())'), 'Redis rate-limit keys must not contain raw IP, tenant or OAuth identity');
  assert(rate_limit.includes('bounded_redis_window_seconds'), 'Redis EXPIRE arguments must not overflow when converting to i64');
}

// 5. weighted_backend_uses_cache_service_owned_redis_client
{
  const weighted = source('crates/modules/rustok-cache/src/weighted.rs');
  assert(weighted.includes('SharedClientRedisCacheBackend::new'), 'weighted backend must reuse the CacheService-owned Redis client');
  assert(!weighted.includes('redis_url()'), 'weighted backend must not reopen Redis from a URL');
  assert(!weighted.includes('RedisCacheBackend::with_circuit_breaker'), 'weighted backend must not use the legacy URL constructor');
}

// 6. default_backend_factory_uses_the_service_owned_redis_client
{
  const service = source('crates/modules/rustok-cache/src/service.rs');
  assert(service.includes('self.backend_shared_client(prefix, ttl, max_capacity)'), 'the default backend factory must delegate to the service-owned client path');
  assert(service.includes('self.backend_shared_client_with_options(prefix, ttl, max_capacity, options)'), 'the per-call backend factory must delegate to the service-owned client path');
  assert(!service.includes('async fn raw_backend('), 'the default factory must not retain a second URL-based Redis construction path');
  assert(!service.includes('RedisCacheBackend::with_circuit_breaker'), 'the default factory must not reopen Redis from the stored URL');
}

// 7. generic_invalidation_and_loader_inputs_are_bounded
{
  const service = source('crates/modules/rustok-cache/src/service.rs');
  const policy = source('crates/modules/rustok-cache/src/policy.rs');
  for (const required of [
    'MAX_CACHE_INVALIDATION_CHANNEL_BYTES',
    'MAX_CACHE_INVALIDATION_KEY_BYTES',
    'MAX_CACHE_LOAD_KEY_BYTES',
    'DEFAULT_MAX_IN_FLIGHT_CACHE_LOADS',
    'ChannelTooLong',
    'KeyTooLong',
    'cache load coordinator saturated',
  ]) {
    assert(service.includes(required), `generic cache resource contract must retain ${required}`);
  }
  assert(service.includes('load_or_fill_rejects_empty_and_oversized_keys_before_loader'), 'load-key bounds must retain regression coverage');
  assert(service.includes('unique_in_flight_loads_are_bounded_without_breaking_same_key_coalescing'), 'unique-flight capacity must retain regression coverage');
  assert(policy.includes('zero_ttl_policy_is_rejected_before_cache_or_loader_work'), 'load policies must not report a successful fill when zero TTL deletes the value');
}

// 8. shared_fallback_health_does_not_mask_primary_degradation
{
  const fallback = source('crates/modules/rustok-cache/src/fallback.rs');
  const weighted = source('crates/modules/rustok-cache/src/weighted.rs');
  const shared = source('crates/modules/rustok-cache/src/shared_backend.rs');
  assert(fallback.includes('self.primary.health().await'), 'fallback health must report the shared primary state');
  assert(weighted.includes('DegradationAwareFallbackBackend::new'), 'weighted Redis backends must use degradation-aware fallback');
  assert(shared.includes('DegradationAwareFallbackBackend::new'), 'entry-count shared Redis backends must use degradation-aware fallback');
}

// 9. stale_refresh_is_bounded_deduplicated_and_atomic
{
  const core_context = source('crates/libs/rustok-core/src/context.rs');
  const core_atomic = source('crates/libs/rustok-core/src/cache_atomic.rs');
  const refresh = source('crates/modules/rustok-cache/src/refresh.rs');
  const observability = source('crates/modules/rustok-cache/src/observability.rs');
  const shared = source('crates/modules/rustok-cache/src/shared_backend.rs');
  const fallback = source('crates/modules/rustok-cache/src/fallback.rs');
  const weighted = source('crates/modules/rustok-cache/src/weighted.rs');
  const service = source('crates/modules/rustok-cache/src/service.rs');
  const atomic_cas = source('crates/modules/rustok-cache/tests/atomic_cas.rs');

  for (const required of [
    'MAX_CACHE_REFRESH_KEY_BYTES',
    'CacheRefreshSchedule::InvalidKey',
    'validate_refresh_request(&key, expected_schema_version, max_encoded_bytes)?',
    'coordinator_rejects_invalid_keys_without_running_refresh',
    'swr_rejects_invalid_key_before_backend_or_loader_work',
    'invalid_swr_configuration_does_not_delete_a_valid_entry',
    'CacheRefreshTaskCompletionGuard',
    'dropping_unpolled_refresh_future_releases_lease_and_counts_failure',
    'system_clock_rejects_refresh_that_expires_while_loader_runs',
  ]) {
    assert(refresh.includes(required), `stale refresh resource contract must retain ${required}`);
  }
  assert(observability.includes('rustok_cache_refresh_rejected_total'), 'rejected stale refresh work must remain observable without key labels');
  assert(core_context.includes('pub enum CacheCompareAndSetOutcome'), 'the backend contract must expose explicit CAS applied/mismatch outcomes');
  assert(core_context.includes('atomic cache compare-and-set is not supported by this backend'), 'unsupported CAS backends must fail closed instead of emulating GET plus SET');
  assert(core_atomic.includes('IN_MEMORY_WRITE_LOCK_STRIPES'), 'in-memory CAS and ordinary writes must share bounded striped locks');
  assert(core_atomic.includes('.and_compute_with(move |current|'), 'local CAS must couple comparison and mutation in one Moka entry operation');
  assert(shared.includes('SHARED_REDIS_COMPARE_AND_SET_SCRIPT') && shared.includes('current ~= ARGV[1]') && shared.includes('PSETEX'), 'service-owned Redis CAS must use one binary-safe conditional Lua operation');
  for (const [name, s] of [
    ['fallback', fallback],
    ['weighted', weighted],
    ['service instrumentation', service],
    ['shared instrumentation', shared],
  ]) {
    assert(s.includes('compare_and_set'), `${name} must delegate the atomic CAS contract`);
  }
  assert(fallback.includes('compare_and_set_fails_closed_when_shared_primary_is_unavailable'), 'distributed fallback CAS must retain fail-closed outage coverage');
  assert(refresh.includes('.compare_and_set(&key, &observed_bytes, bytes, ttl)'), 'background refresh must publish through backend-level atomic CAS');
  assert(!refresh.includes('backend.get(&key).await?.as_deref() != Some(observed_bytes.as_slice())'), 'background refresh must not use a racy prewrite GET check');
  assert(refresh.includes('concurrent_replacement_wins_over_slow_stale_refresh'), 'SWR must retain regression coverage for superseded refresh writes');
  assert(atomic_cas.includes('real_redis_compare_and_set_is_binary_safe_and_conditionally_deletes'), 'atomic Redis CAS must retain live integration coverage');
}

// 10. invalidation_recovery_is_two_phase_and_monotonic
{
  const invalidation = source('crates/modules/rustok-cache/src/invalidation.rs');
  const bounded = source('crates/modules/rustok-cache/src/bounded_invalidation.rs');
  assert(invalidation.includes('UnverifiedFirst'), 'unseeded invalidation consumers must not trust the first observed generation');
  assert(bounded.includes('pub fn acknowledge_recovery('), 'gap recovery must be acknowledged only after clear/rebuild succeeds');
  assert(invalidation.includes('OffsetRegressed'), 'durable invalidation offsets must never move backwards');
  assert(invalidation.includes('gap_does_not_advance_until_recovery_is_acknowledged'), 'gap tracking must retain regression coverage for failed recovery');
}

// 11. generation_fallback_is_trusted_monotonic_and_bounded
{
  const generation = source('crates/modules/rustok-cache/src/generation.rs');
  const backend_generation = source('crates/modules/rustok-cache/src/backend_generation.rs');
  assert(generation.includes('NoLocalSnapshot'), 'Redis generation failure without a trusted local snapshot must fail closed');
  assert(generation.includes('GenerationRegressed'), 'shared generation loss must not lower a locally observed generation');
  assert(generation.includes('DEFAULT_MAX_LOCAL_GENERATION_SNAPSHOTS'), 'trusted generation snapshots must have a default process memory bound');
  assert(generation.includes('LocalSnapshotCapacityExceeded'), 'new generation namespaces must fail closed after snapshot capacity is reached');
  assert(generation.includes('trusted_local_snapshots_are_bounded_without_evicting_existing_namespaces'), 'generation capacity must retain regression coverage without evicting trusted state');
  for (const required of [
    'MAX_GENERATION_OPERATION_ATTEMPTS',
    'snapshot_is_current',
    'get_discards_old_namespace_result_when_generation_changes_midflight',
    'set_retries_current_namespace_when_generation_changes_midflight',
  ]) {
    assert(backend_generation.includes(required), `generation-aware cache operations must retain ${required}`);
  }
}

// 12. typed_loading_invalidates_raced_incompatible_values
{
  const typed = source('crates/modules/rustok-cache/src/typed.rs');
  assert(typed.includes('incompatible_value_racing_after_initial_probe_is_invalidated'), 'typed loading must retain race regression coverage');
  assert(typed.includes('backend.invalidate(&key).await?'), 'typed validation failures must propagate shared invalidation failures');
}

// 13. distributed_lease_deadline_is_usable_after_confirmation
{
  const lease = source('crates/modules/rustok-cache/src/lease.rs');
  assert(lease.includes('OperationTimeoutNotLessThanTtl'), 'lease operation timeout must remain strictly below lease TTL');
  assert(lease.includes('ExpiredBeforeConfirmation'), 'a lease confirmed after its deadline must not be returned as acquired');
  assert(lease.includes('MAX_LEASE_CACHE_KEY_BYTES'), 'lease source keys must be bounded before hashing');
}

// 14. cache_values_and_keys_have_bounded_versioned_contracts
{
  const key = source('crates/modules/rustok-cache/src/key.rs');
  const envelope = source('crates/modules/rustok-cache/src/envelope.rs');
  for (const required of [
    'MAX_CACHE_KEY_BYTES',
    'MAX_CACHE_IDENTITY_BYTES',
    'MAX_CACHE_KEY_INPUT_BYTES',
    'MAX_CACHE_KEY_DYNAMIC_COMPONENTS',
    'IdentityTooLong',
    'TooManyDynamicComponents',
  ]) {
    assert(key.includes(required), `canonical cache key contract must retain ${required}`);
  }
  assert(envelope.includes('DEFAULT_MAX_CACHE_ENVELOPE_BYTES'), 'typed cache envelopes must retain an explicit maximum encoded size');
  assert(envelope.includes('CACHE_ENVELOPE_FORMAT_VERSION'), 'typed cache envelopes must remain wire-format versioned');
  assert(envelope.includes('ser_flavors::Size::default()'), 'cache envelopes must be measured before output allocation');
  assert(envelope.includes('BoundedEnvelopeWriter'), 'cache envelope output must remain physically bounded during serialization');
  assert(!envelope.includes('postcard::to_stdvec(self)'), 'cache envelope limits must not be checked only after allocating the complete output');
}

// 15. cache_contracts_have_single_owners
{
  const service = source('crates/modules/rustok-cache/src/service.rs');
  const shared = source('crates/modules/rustok-cache/src/shared_backend.rs');
  const weighted = source('crates/modules/rustok-cache/src/weighted.rs');
  const instrumentation = source('crates/modules/rustok-cache/src/instrumentation.rs');
  const invalidation = source('crates/modules/rustok-cache/src/invalidation.rs');
  const generation = source('crates/modules/rustok-cache/src/generation.rs');

  assert(instrumentation.includes('pub(crate) struct InstrumentedCacheBackend'), 'the cache instrumentation wrapper must have exactly one implementation');
  for (const [name, s] of [
    ['service', service],
    ['shared_backend', shared],
    ['weighted', weighted],
  ]) {
    assert(!s.includes('struct Instrumented'), `${name} must not reintroduce a second instrumentation wrapper`);
    assert(s.includes('crate::instrumentation::InstrumentedCacheBackend'), `${name} must reuse the shared instrumentation wrapper`);
  }
  assert(instrumentation.includes('hits: self.hits.load(Ordering::Relaxed),'), 'observed hits must not add inner counters, which double counted every hit');
  assert(invalidation.includes('CacheInvalidationMessage::try_new'), 'payload construction must be validated by the transport constructor');
  assert(!invalidation.includes('pub struct CacheInvalidationGapTracker'), 'the unbounded gap tracker must stay deleted; BoundedCacheInvalidationGapTracker is canonical');
  assert(generation.includes('GENERATION_CATCH_UP_SCRIPT'), 'shared generation loss must be repaired with an advance-only script');
  assert(generation.includes('SharedBumpAppliedLocalLag'), 'a bump after a successful INCR must be distinguishable from a failed bump');
}

// 16. cache_publish_reports_shared_transport_failures
{
  const service = source('crates/modules/rustok-cache/src/service.rs');
  const durable = source('crates/modules/rustok-cache/src/durable_invalidation_service.rs');
  assert(service.includes('pub async fn publish(') && service.includes('Result<CacheInvalidationOutcome, CacheInvalidationError>'), 'cache invalidation publication must return a Result');
  assert(service.includes('CacheInvalidationError::SharedTransport'), 'a configured-but-failing Redis publish must be reported, not swallowed');
  assert(durable.includes('DurableCacheInvalidationError::Transport'), 'durable publication must propagate the shared transport failure');
}

// 17. cache_admin_authority_has_one_rule_for_both_transports
{
  const system = source('apps/server/src/graphql/system.rs');
  const transport = source('crates/modules/rustok-cache/admin/src/transport.rs');
  const api = source('crates/libs/rustok-api/src/context/host_authority.rs');
  const host = source('apps/server/src/host_authority.rs');

  assert(api.includes('pub fn authorizes_host_or_permission('), 'the cache admin authority rule must be owned by the shared host contract');
  assert(host.includes('pub use rustok_api::{current_host_authority, with_host_authority_scope};') && !host.includes('task_local!'), 'the host must not define a second host-authority task-local scope');
  assert(system.includes('fn require_cache_read_authority(') && system.includes('rustok_api::authorizes_host_or_permission('), 'cacheHealth must evaluate the shared authority rule');
  const cache_health = system.split('async fn cache_health(')[1];
  assert(cache_health && !cache_health.split('\n').slice(0, 6).some(line => line.includes('require_host_read()')), 'cacheHealth must not be limited to host-global authority while the admin UI uses it');
  const normalized = transport.split(/\s+/).join(' ');
  assert(normalized.includes('rustok_api::authorizes_host_or_permission(Some(auth), required, host_authority)'), 'the native cache admin transport must reuse the shared authority rule');
  assert(!transport.includes('fn ensure_cache_admin_permission('), 'the native transport must not keep a second, hardcoded permission check');
  for (const [required, level, operation] of [
    ['SETTINGS_READ', 'Read', 'cache health and cache settings reads'],
    ['SETTINGS_MANAGE', 'Manage', 'cache settings writes'],
  ]) {
    const expected = `native_scope( &rustok_api::Permission::${required}, rustok_api::HostAuthority::${level}, )`;
    assert(normalized.includes(expected), `${operation} must require the ${level} level`);
  }
}

// 18. cache_readiness_has_one_predicate_for_every_consumer
{
  const service = source('crates/modules/rustok-cache/src/service.rs');
  const redis_status = source('crates/modules/rustok-cache/src/redis_status.rs');
  const health = source('apps/server/src/controllers/health.rs');

  assert(redis_status.includes('&& self.generation_recovery_pending == 0'), 'RedisCacheStatus::is_healthy must require completed recovery');
  const report = service.split('impl CacheHealthReport {')[1];
  assert(report.includes('pub fn is_connected(') && report.includes('pub fn is_healthy('), 'CacheHealthReport must keep the transport and readiness views separate');
  assert(report.includes('self.redis_recovery_pending == 0') && report.includes('self.redis_recovery_error.is_none()'), 'CacheHealthReport::is_healthy must require completed recovery, like RedisCacheStatus');
  assert(health.includes('report.is_connected()') && health.includes('cache recovery is incomplete'), 'readiness must report incomplete recovery instead of a transport outage');
}

// 19. cache_ttl_policy_cannot_be_built_without_its_constructor
{
  const policy = source('crates/modules/rustok-cache/src/policy.rs');
  assert(policy.includes('pub struct CacheTtlPolicy'), 'CacheTtlPolicy must stay an opaque struct');
  assert(!policy.includes('pub enum CacheTtlPolicy'), 'an exposed CacheTtlPolicy enum re-opens zero-TTL construction');
  assert(policy.includes('enum CacheTtlPolicyKind'), 'the private representation must be named CacheTtlPolicyKind');
  const kind = policy.split('enum CacheTtlPolicyKind {')[1];
  assert(kind && !kind.split('}\n')[0].includes('pub '), 'the policy representation must not expose its variants');
  for (const constructor of ['pub fn fixed(', 'pub fn deterministic_jitter(']) {
    const body = policy.split(constructor)[1];
    assert(body && body.split('pub fn ')[0].includes('CachePolicyError::ZeroTtl'), `${constructor} must reject a zero TTL`);
  }
}

// 20. every_cache_backend_wrapper_forwards_state_reconciliation
for (const [file, wrapper] of [
  ['crates/modules/rustok-cache/src/backend_generation.rs', 'GenerationAwareCacheBackend'],
  ['crates/modules/rustok-cache/src/backend_generation_recovery.rs', 'GenerationRecoveryHealthBackend'],
  ['crates/modules/rustok-cache/src/cas_observability.rs', 'ObservedCacheBackend'],
  ['crates/modules/rustok-cache/src/fallback.rs', 'DegradationAwareFallbackBackend'],
  ['crates/modules/rustok-cache/src/instrumentation.rs', 'InstrumentedCacheBackend'],
]) {
  const text = source(file);
  const impl_block = text.split(`impl CacheBackend for ${wrapper} {`)[1];
  assert(impl_block, `${wrapper} must implement CacheBackend`);
  const body = impl_block.split('\n}\n')[0];
  assert(body.includes('reconcile_local_state'), `${wrapper} must forward reconcile_local_state instead of ignoring the recovery contract`);
}

console.log('Verifying tenant_cache_architecture_guard invariants...');

// Tenant Guard 1: tenant_cache_uses_weighted_shared_client_backends
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('.backend_weighted('), 'tenant cache must use backend_weighted');
  assert(!tenant.includes('.backend(\n'), 'tenant cache must not use plain unweighted backend');
  assert(tenant.includes('TENANT_CACHE_MAX_WEIGHT_BYTES'), 'tenant cache must have max weight bytes bound');
  assert(tenant.includes('TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES'), 'tenant negative cache must have max weight bytes bound');
}

// Tenant Guard 2: tenant_keys_are_canonical_and_schema_versioned
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('CanonicalCacheKeyBuilder::new'), 'tenant cache keys must use CanonicalCacheKeyBuilder');
  assert(tenant.includes('TENANT_CACHE_VERSION: &str = "v2"'), 'tenant cache version must be v2');
  assert(tenant.includes('TENANT_CONTEXT_SCHEMA_VERSION'), 'tenant context schema version must exist');
  assert(tenant.includes('TENANT_NEGATIVE_SCHEMA_VERSION'), 'tenant negative schema version must exist');
}

// Tenant Guard 3: tenant_context_cache_payload_keeps_settings_typed_and_versioned
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('TENANT_CONTEXT_SCHEMA_VERSION: u32 = 3'), 'tenant context schema version must be 3');
  assert(tenant.includes('settings: serde_json::Value'), 'tenant context must keep settings as serde_json::Value');
  assert(tenant.includes('impl From<TenantContext> for CachedTenantContext'), 'must implement From<TenantContext> for CachedTenantContext');
  assert(tenant.includes('impl From<CachedTenantContext> for TenantContext'), 'must implement From<CachedTenantContext> for TenantContext');
  assert(!tenant.includes('impl TryFrom<TenantContext> for CachedTenantContext'), 'must not use TryFrom for CachedTenantContext');
  assert(!tenant.includes('impl TryFrom<CachedTenantContext> for TenantContext'), 'must not use TryFrom for TenantContext');
  assert(!tenant.includes('settings_json: String'), 'must not use string settings_json');
  assert(!tenant.includes('serde_json::from_str(&context.settings_json)'), 'must not parse string settings_json');
}

// Tenant Guard 4: tenant_cache_initialization_uses_canonical_shared_service_without_check_insert_race
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('shared_insert_if_absent(cache_service.clone())'), 'must insert cache service if absent');
  assert(tenant.includes('let canonical_cache_service = ctx.shared_get::<CacheService>()'), 'must get canonical cache service');
  assert(tenant.includes('TenantCacheInfrastructure::new(&canonical_cache_service)'), 'must construct infrastructure from canonical cache service');
  assert(tenant.includes('shared_insert_if_absent(Arc::new(infrastructure))'), 'must insert infrastructure if absent');
}

// Tenant Guard 5: tenant_payloads_use_typed_envelopes_and_explicit_negative_policy
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('load_enveloped_or_fill('), 'must use load_enveloped_or_fill');
  assert(tenant.includes('CacheEnvelope::new'), 'must use CacheEnvelope::new');
  assert(tenant.includes('NegativeCachePolicy::deterministic_jittered'), 'must use deterministic_jittered negative policy');
  assert(tenant.includes('get_negative::<CachedTenantMiss>'), 'must use get_negative');
  assert(tenant.includes('store_negative('), 'must use store_negative');
  assert(!tenant.includes('serde_json::from_slice::<TenantContext>'), 'must not use serde_json::from_slice');
  assert(!tenant.includes('serde_json::to_vec(&context)'), 'must not use serde_json::to_vec');
}

// Tenant Guard 6: tenant_loader_and_direct_redis_metrics_are_bounded
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  assert(tenant.includes('TENANT_CACHE_LOADER_TIMEOUT'), 'must have loader timeout');
  assert(tenant.includes('CacheTtlPolicy::deterministic_jitter'), 'must use deterministic jitter');
  assert(tenant.includes('tenant_cache_redis_timeout('), 'must have redis timeout wrapper');
  assert(tenant.includes('redis::cmd("INCR")'), 'must bound INCR');
  assert(tenant.includes('redis::cmd("GET")'), 'must bound GET');
}

// Tenant Guard 7: tenant_invalidation_payload_uses_versioned_cache_invalidation
{
  const generation = source('apps/server/src/services/tenant_cache_generation.rs');
  assert(generation.includes('VersionedCacheInvalidation::from_message(&message)'), 'must use VersionedCacheInvalidation');
}

// Tenant Guard 8: weighted_factories_apply_generation_before_instrumentation
{
  const weighted = source('crates/modules/rustok-cache/src/weighted.rs');
  const wrap = weighted.indexOf('self.wrap_generation_aware_backend(prefix, backend).await');
  const instrument = weighted.indexOf('InstrumentedWeightedCacheBackend::new(prefix, backend)');
  assert(wrap !== -1, 'weighted backend must be generation-aware');
  assert(instrument !== -1, 'weighted backend instrumentation must remain present');
  assert(wrap < instrument, 'generation mapping must occur before metrics instrumentation');
}

// Tenant Guard 9: tenant_generation_matches_and_aliases_both_physical_backend_prefixes
{
  const tenant = source('apps/server/src/middleware/tenant.rs');
  const generation = source('apps/server/src/services/tenant_cache_generation.rs');
  const backend_generation = source('crates/modules/rustok-cache/src/backend_generation.rs');

  assert(tenant.includes('tenant-cache:{}:data'), 'must alias data prefix');
  assert(tenant.includes('tenant-cache:{}:negative'), 'must alias negative prefix');
  assert(generation.includes('TENANT_CACHE_DATA_BACKEND_PREFIX: &str = "tenant-cache:v2:data"'), 'must match data backend prefix');
  assert(generation.includes('TENANT_CACHE_NEGATIVE_BACKEND_PREFIX: &str = "tenant-cache:v2:negative"'), 'must match negative backend prefix');
  assert(generation.includes('bind_cache_backend_generation_aliases('), 'must call bind_cache_backend_generation_aliases');
  assert(generation.includes('bind_tenant_backend_generations()?'), 'must bind tenant backend generations');
  assert(backend_generation.includes('pub fn bind_cache_backend_generation_aliases'), 'must export bind_cache_backend_generation_aliases');
  assert(backend_generation.includes('AliasAlreadyBound'), 'must handle AliasAlreadyBound');
}

// Tenant Guard 10: outbox_keeps_transactional_transport_and_rotates_relay_target
{
  const factory = source('apps/server/src/services/event_transport_factory.rs');
  assert(factory.includes('transport: outbox_transport'), 'outbox must keep transactional transport');
  assert(
    factory.includes('TenantCacheGenerationTransport::new(gated, cache.clone())') ||
    factory.includes('tenant_generation_transport(ctx, &cache, relay_target)'),
    'must rotate relay target'
  );
  assert(factory.includes('start_tenant_cache_generation_listener(ctx, cache.clone()).await?'), 'must start tenant generation listener');
}

// Tenant Guard 11: tenant_generation_closes_subscribe_gap_and_rotates_before_delivery
{
  const generation = source('apps/server/src/services/tenant_cache_generation.rs');
  assert(generation.includes('consume_subscription_with_ready('), 'must use consume_subscription_with_ready');
  assert(generation.includes('redis_ready_recovery'), 'must use redis_ready_recovery');

  const bump = generation.indexOf('.bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)');
  const invalidation = generation.indexOf('.publish_durable(&record)');
  const downstream = generation.indexOf('self.inner.publish(envelope).await');
  assert(bump !== -1 && invalidation !== -1 && downstream !== -1, 'tenant events must bump, publish invalidation, and deliver downstream');
  assert(bump < invalidation && invalidation < downstream, 'bump < invalidation < downstream must hold');
}

// Tenant Guard 12: tenant_generation_dedupe_is_bounded_serialized_two_phase_and_retry_safe
{
  const generation = source('apps/server/src/services/tenant_cache_generation.rs');
  const dedupe = source('crates/modules/rustok-cache/src/event_dedupe.rs');

  for (const required of [
    'DEFAULT_MAX_CACHE_EVENT_DEDUPE_ENTRIES',
    'DEFAULT_CACHE_EVENT_DEDUPE_TTL',
    'CACHE_EVENT_DEDUPE_LOCK_STRIPES',
    'capacity_eviction_total',
    'probe_does_not_precommit_failed_work',
    'same_event_serialization_closes_the_concurrent_probe_race',
  ]) {
    assert(dedupe.includes(required), `event dedupe must retain ${required}`);
  }

  const serialize = generation.indexOf('successful_rotations.serialize_event(envelope.id)');
  const probe = generation.indexOf('successful_rotations.is_duplicate(envelope.id)');
  const bump = generation.indexOf('.bump_cache_backend_generation(TENANT_CACHE_BACKEND_PREFIX)');
  const publish = generation.indexOf('.publish_durable(&record)');
  const commit = generation.indexOf('successful_rotations.observe(envelope.id)');
  const downstream = generation.indexOf('self.inner.publish(envelope).await');

  assert(serialize < probe && probe < bump && bump < publish && publish < commit && commit < downstream, 'dedupe ordering must hold');
  assert(generation.includes('retry_delivers_downstream_without_rotating_generation_twice'), 'must retain retry regression test marker');
}

// Tenant Guard 13: tenant_generation_health_is_context_scoped_and_component_aware
{
  const generation = source('apps/server/src/services/tenant_cache_generation.rs');
  const status = source('apps/server/src/services/tenant_cache_generation_status.rs');

  for (const required of [
    'GENERATION_RECONCILE_INTERVAL',
    'MissedTickBehavior::Skip',
    'periodic_reconciliation',
    'state: Arc<TenantCacheGenerationListenerState>',
    'tenant_cache_generation_listener_snapshot',
    '.mark_subscriber_ready_after_recovery()',
    '.mark_subscriber_activity_healthy()',
    '.mark_reconciliation_healthy()',
    '.mark_subscriber_degraded(',
    '.mark_reconciliation_degraded(',
    '.mark_local_degraded(',
  ]) {
    assert(generation.includes(required), `tenant generation runtime must retain ${required}`);
  }

  for (const required of [
    'subscriber_ready && reconciliation_healthy',
    'MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES',
    'redis_health_requires_subscriber_and_reconciliation',
    'subscriber_activity_does_not_hide_reconciliation_failure',
    'reconciliation_success_does_not_hide_subscriber_failure',
    'independent_runtime_states_do_not_overwrite_each_other',
    'record_tenant_generation_listener_metrics',
  ]) {
    assert(status.includes(required), `tenant generation status must retain ${required}`);
  }
  assert(!status.includes('OnceLock'), 'tenant generation listener state must be owned by ServerRuntimeContext');
}

// Tenant Guard 14: tenant_generation_metrics_are_label_free_and_registered_once
{
  const observability = source('crates/modules/rustok-cache/src/tenant_generation_observability.rs');
  for (const required of [
    'rustok_cache_tenant_generation_listener_status',
    'rustok_cache_tenant_generation_local_ready',
    'rustok_cache_tenant_generation_subscriber_ready',
    'rustok_cache_tenant_generation_reconciliation_healthy',
    'register_runtime_collector',
    'metrics_are_label_free_and_component_specific',
  ]) {
    assert(observability.includes(required), `tenant generation observability must retain ${required}`);
  }
  assert(observability.includes("assert!(!payload.contains('{'))"), 'tenant generation lifecycle metrics must remain label-free');
}

console.log('ALL 34 cache & tenant cache architectural invariants PASSED!');
