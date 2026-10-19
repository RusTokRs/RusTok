#!/usr/bin/env node
// Source invariants only: deliberately does not execute tests, compile Rust, or claim runtime safety.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const configured = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configured ? pathToFileURL(`${path.resolve(configured)}${path.sep}`) : new URL('../../', import.meta.url);
const read = (file) => readFileSync(new URL(file, root), 'utf8');
const failures = [];
const need = (ok, label) => { if (!ok) failures.push(label); };
const compact = (text) => text.replace(/\s/g, '');
function body(source, name) {
  const match = new RegExp(`fn\\s+${name}\\s*\\(`).exec(source);
  if (!match) { failures.push(`missing function ${name}`); return ''; }
  const start = source.indexOf('{', match.index);
  let depth = 0;
  for (let i = start; i < source.length; i++) {
    if (source[i] === '{') depth++;
    if (source[i] === '}' && --depth === 0) return source.slice(start, i + 1);
  }
  failures.push(`unterminated function ${name}`); return '';
}
const ports = read('crates/modules/rustok-pricing/src/ports.rs');
const publicSource = read('crates/modules/rustok-pricing/src/public_error.rs');
const native = read('crates/modules/rustok-product/storefront/src/transport/native_server_adapter.rs');
const mutations = read('crates/modules/rustok-commerce/src/graphql/mutations/pricing.rs');
const shim = read('crates/modules/rustok-commerce/src/graphql/safe_query/source/rustok_pricing_shim.rs');
const owner = body(ports, 'pricing_error_policy');
const core = body(ports, 'pricing_core_error_policy');
const rich = body(ports, 'pricing_rich_error_policy');
const publicPolicy = body(publicSource, 'public_error');
for (const [source, variants, prefix] of [
  [owner, ['Database', 'ProductNotFound', 'VariantNotFound', 'DuplicateHandle', 'DuplicateSku', 'InvalidPrice', 'Validation', 'InsufficientInventory', 'InvalidOptionCombination', 'ShippingProfileNotFound', 'DuplicateShippingProfileSlug', 'NoVariants', 'CannotDeletePublished', 'Core', 'Rich'], 'CommerceError'],
  [core, ['Database', 'Cache', 'External', 'Auth', 'Forbidden', 'InvalidIdFormat', 'Validation', 'NotFound', 'Serialization', 'Scripting', 'Internal'], 'CoreError'],
  [rich, ['Validation', 'BusinessLogic', 'Unauthenticated', 'Forbidden', 'NotFound', 'Conflict', 'RateLimited', 'Database', 'ExternalService', 'Timeout', 'Internal'], 'ErrorKind'],
  [publicPolicy, ['Validation', 'NotFound', 'Conflict', 'Forbidden', 'Unavailable', 'Timeout', 'InvariantViolation'], 'PortErrorKind'],
]) {
  for (const variant of variants) need(source.includes(`${prefix}::${variant}`), `missing ${prefix}::${variant}`);
  need(!/\b_\s*=>/.test(source), `${prefix}: wildcard masks future variants`);
}
function outcomes(source, cases, label) {
  for (const [variant, kind, code, retryable] of cases) {
    const tail = source.slice(source.indexOf(variant));
    const arrow = tail.indexOf('=>');
    const match = tail.slice(arrow).match(/PricingErrorPolicy::new\(\s*PortErrorKind::(\w+),\s*"([^"]+)",\s*"[^"]+",\s*(true|false),?\s*\)/);
    need(arrow >= 0 && match?.[1] === kind && match?.[2] === `pricing.${code}` &&
      match?.[3] === String(retryable), `${label}: wrong outcome for ${variant}`);
  }
}
outcomes(owner, [
  ['CommerceError::Database', 'Unavailable', 'database_unavailable', true],
  ['CommerceError::ProductNotFound', 'NotFound', 'product_not_found', false],
  ['CommerceError::VariantNotFound', 'NotFound', 'variant_not_found', false],
  ['CommerceError::DuplicateHandle', 'Conflict', 'duplicate_handle', false],
  ['CommerceError::DuplicateSku', 'Conflict', 'duplicate_sku', false],
  ['CommerceError::InvalidPrice', 'Validation', 'validation', false],
  ['CommerceError::Validation', 'Validation', 'validation', false],
  ['CommerceError::InsufficientInventory', 'Conflict', 'insufficient_inventory', false],
  ['CommerceError::InvalidOptionCombination', 'Validation', 'invalid_option_combination', false],
  ['CommerceError::ShippingProfileNotFound', 'NotFound', 'shipping_profile_not_found', false],
  ['CommerceError::DuplicateShippingProfileSlug', 'Conflict', 'duplicate_shipping_profile_slug', false],
  ['CommerceError::NoVariants', 'Validation', 'no_variants', false],
  ['CommerceError::CannotDeletePublished', 'Conflict', 'cannot_delete_published', false],
], 'Commerce');
outcomes(core, [
  ['CoreError::Database', 'Unavailable', 'database_unavailable', true],
  ['CoreError::Cache', 'Unavailable', 'dependency_unavailable', true],
  ['CoreError::External', 'Unavailable', 'dependency_unavailable', true],
  ['CoreError::Auth', 'Forbidden', 'access_denied', false],
  ['CoreError::Forbidden', 'Forbidden', 'access_denied', false],
  ['CoreError::InvalidIdFormat', 'Validation', 'validation', false],
  ['CoreError::Validation', 'Validation', 'validation', false],
  ['CoreError::NotFound', 'NotFound', 'resource_not_found', false],
  ['CoreError::Serialization', 'InvariantViolation', 'core_error', false],
  ['CoreError::Scripting', 'InvariantViolation', 'core_error', false],
  ['CoreError::Internal', 'InvariantViolation', 'core_error', false],
], 'Core');
outcomes(rich, [
  ['ErrorKind::Validation', 'Validation', 'validation', false],
  ['ErrorKind::BusinessLogic', 'Validation', 'validation', false],
  ['ErrorKind::Unauthenticated', 'Forbidden', 'access_denied', false],
  ['ErrorKind::Forbidden', 'Forbidden', 'access_denied', false],
  ['ErrorKind::NotFound', 'NotFound', 'resource_not_found', false],
  ['ErrorKind::Conflict', 'Conflict', 'state_conflict', false],
  ['ErrorKind::RateLimited', 'Unavailable', 'rate_limited', true],
  ['ErrorKind::Database', 'Unavailable', 'database_unavailable', true],
  ['ErrorKind::ExternalService', 'Unavailable', 'dependency_unavailable', true],
  ['ErrorKind::Timeout', 'Timeout', 'timeout', true],
  ['ErrorKind::Internal', 'InvariantViolation', 'rich_error', false],
], 'Rich');
need(ports.includes("code: &'static str,") && ports.includes("message: &'static str,"), 'owner policy must constrain code/copy to static values');
need(owner.includes('CommerceError::Core(error) => pricing_core_error_policy(error)'), 'owner must delegate Core');
need(owner.includes('CommerceError::Rich(error) => pricing_rich_error_policy(&error.kind)'), 'owner must classify Rich only by kind');
const servicePublic = body(publicSource, 'map_pricing_service_error');
need(servicePublic.includes('crate::ports::pricing_error_policy(error)'), 'direct service consumers must share the port policy');
need(servicePublic.includes('crate::ports::log_pricing_service_failure('), 'bounded service diagnostic required');
need(compact(body(publicSource, 'map_pricing_port_error')).includes('public_error(&error.kind,error.retryable)'), 'port payload is not public authority');
need(compact(publicPolicy).includes('owner_retryable||matches!(kind,PortErrorKind::Unavailable|PortErrorKind::Timeout)'), 'do not erase retryability');
need(publicPolicy.includes('Uuid::new_v4()'), 'fresh opaque reference required');
for (const code of ['REQUEST_INVALID', 'RESOURCE_NOT_FOUND', 'STATE_CONFLICT', 'ACCESS_DENIED', 'TEMPORARILY_UNAVAILABLE', 'OPERATION_FAILED']) need(publicPolicy.includes(`"PRICING_${code}"`), `missing public code ${code}`);
const nativeMapper = body(native, 'map_pricing_service_error');
const mutationMapper = body(mutations, 'pricing_port_graphql_error');
need(nativeMapper.includes('rustok_pricing::map_pricing_service_error('), 'native must use Pricing owner');
need(mutationMapper.includes('rustok_pricing::map_pricing_port_error(&error)'), 'GraphQL mutations must use Pricing owner');
for (const [label, source] of [['native', nativeMapper], ['mutation', mutationMapper], ['public presentation', publicSource.split('#[cfg(test)]')[0]], ['service diagnostic', body(ports, 'log_pricing_service_failure')]]) {
  for (const forbidden of ['error = ?error', 'error.to_string()', 'error.message.clone()', 'error.user_message', 'error.status_code', 'error.error_code', 'error.trace_id', 'error.source', 'error.fields']) need(!source.includes(forbidden), `${label}: payload/metadata escape via ${forbidden}`);
}
need(!nativeMapper.includes('CommerceError::') && !mutationMapper.includes('PortErrorKind::'), 'no transport-local owner classifier');
need(mutationMapper.includes('async_graphql::Error::new(public.message)'), 'GraphQL public message must be static');
need(mutationMapper.includes('extensions.set("correlation_id", public.correlation_id.to_string())'), 'GraphQL public reference missing');
need(shim.includes('let retryable = retryable || self.error.retryable;'), 'query facade must preserve owner retryability');
const implementations = ports.slice(ports.indexOf('impl PricingReadPort for'), ports.indexOf('struct PricingPortContextFacts'));
need((implementations.match(/log_pricing_port_entry\(&context, owner_operation\)/g) ?? []).length === 10, 'ten owner operation entry logs must be bounded');
need(!implementations.includes('tracing::debug!('), 'operation-local payload debug log');
for (const name of ['legacy_domain_outcomes_keep_owner_classification_and_codes', 'nested_core_errors_preserve_all_eleven_classifications', 'rich_kind_is_authority_not_status_or_untrusted_metadata', 'arbitrary_port_payload_is_private_and_owner_retryability_is_not_lost']) need(publicSource.includes(`fn ${name}(`), `unrun scenario missing: ${name}`);
need(ports.includes('fn canonical_port_uses_nested_owner_policy_without_payload_or_retry_loss('), 'unrun canonical-port scenario missing');
if (failures.length) { console.error(failures.join('\n')); process.exit(1); }
console.log('Pricing public-error source guard PASS (owner Core/Rich policy, bounded consumers and retryability; no tests or runtime execution)');
