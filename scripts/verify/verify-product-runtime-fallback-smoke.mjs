#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const json = (path) => JSON.parse(read(path));
const fail = (message) => {
  console.error(`[verify-product-runtime-fallback-smoke] ${message}`);
  process.exit(1);
};
const sameSet = (actual, expected) =>
  Array.isArray(actual) &&
  Array.isArray(expected) &&
  actual.length === expected.length &&
  expected.every((item) => actual.includes(item));

function functionBody(source, name) {
  const signature = new RegExp(`async\\s+fn\\s+${name}\\s*\\(`, 'g');
  let match;
  while ((match = signature.exec(source)) !== null) {
    const semicolon = source.indexOf(';', match.index);
    const open = source.indexOf('{', match.index);
    if (open < 0 || (semicolon >= 0 && semicolon < open)) continue;
    let depth = 0;
    for (let index = open; index < source.length; index += 1) {
      if (source[index] === '{') depth += 1;
      if (source[index] === '}' && --depth === 0) {
        return source.slice(open + 1, index);
      }
    }
  }
  return null;
}

function assertOrdered(body, markers, operation) {
  let previous = -1;
  for (const marker of markers) {
    const index = body.indexOf(marker);
    if (index < 0) fail(`${operation} source marker missing: ${marker}`);
    if (index <= previous) fail(`${operation} source marker order drift: ${marker}`);
    previous = index;
  }
}

const registryPath = 'crates/rustok-product/contracts/product-fba-registry.json';
const smokePath = 'crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json';
const registry = json(registryPath);
const smoke = json(smokePath);
const contractSmoke = json('crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json');
const ports = read('crates/rustok-product/src/ports.rs');
const plan = read('crates/rustok-product/docs/implementation-plan.md');
const central = read('docs/modules/registry.md');

if (registry.status !== 'boundary_ready') fail('product registry must be boundary_ready for fallback smoke evidence');
if (smoke.schema_version !== 1 || smoke.module !== 'product') fail('runtime smoke identity drift');
if (smoke.status !== 'no_compile_executable_runtime_fallback_smoke') fail('runtime smoke status drift');
if (smoke.generated_from !== registryPath) fail('runtime smoke source drift');
if (smoke.runner !== 'scripts/verify/verify-product-runtime-fallback-smoke.mjs') fail('runtime smoke runner drift');
if (smoke.contract_version !== registry.contract_version) fail('runtime smoke contract version drift');
if (registry.evidence?.runtime_fallback_smoke !== smokePath) fail('registry runtime fallback smoke path drift');
if (registry.evidence?.runtime_fallback_smoke_runner !== smoke.runner) fail('registry runtime fallback runner drift');
if (!sameSet(smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('runtime smoke profile drift');
if (!sameSet(smoke.profiles, contractSmoke.fallback_profiles)) fail('runtime fallback profiles must mirror contract smoke');
if (registry.contract_tests.fallback_smoke.status !== 'planned_runtime_pending') {
  fail('fallback smoke must remain planned_runtime_pending until live execution evidence lands');
}

for (const profile of registry.contract_tests.fallback_smoke.profiles) {
  if (!smoke.smoke_cases.some((entry) => entry.profile === profile && entry.execution_status === 'no_compile_executable_locked')) {
    fail(`runtime smoke missing executable no-compile profile ${profile}`);
  }
}

for (const marker of [
  'trait ProductCatalogReadPort',
  'impl ProductCatalogReadPort for crate::CatalogService',
  'const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48',
  'validate_published_products_request',
  'parse_port_tenant_id',
  'product_error_to_port_error',
  'PortError::validation',
  'PortError::unavailable',
  'PortErrorKind::NotFound',
]) {
  if (!ports.includes(marker)) fail(`runtime smoke source missing ${marker}`);
}

const readBody = functionBody(ports, 'read_product_projection');
if (!readBody) fail('read_product_projection implementation body missing');
assertOrdered(
  readBody,
  [
    'context.require_policy(PortCallPolicy::read())?',
    'parse_port_tenant_id(&context)?',
    'request.locale.as_deref().unwrap_or(context.locale.as_str())',
    'self.get_product_with_locale_fallback(',
    'request.fallback_locale.as_deref()',
    '.map_err(product_error_to_port_error)',
  ],
  'read_product_projection',
);

const listBody = functionBody(ports, 'list_published_products');
if (!listBody) fail('list_published_products implementation body missing');
assertOrdered(
  listBody,
  [
    'context.require_policy(PortCallPolicy::read())?',
    'validate_published_products_request(&request)?',
    'parse_port_tenant_id(&context)?',
    'request.locale.as_deref().unwrap_or(context.locale.as_str())',
    'self.list_published_products_with_locale_fallback(',
    'request.public_channel_slug.as_deref()',
    '.map_err(product_error_to_port_error)',
  ],
  'list_published_products',
);

for (const consumer of registry.consumers) {
  if (!consumer.fallback_profiles?.every((profile) => smoke.profiles.includes(profile))) {
    fail(`${consumer.module} consumer fallback profile missing from runtime smoke`);
  }
}

if (!plan.includes('- FBA status: `boundary_ready`')) fail('local plan FBA status drift');
if (!plan.includes(smokePath)) fail('local plan lacks runtime fallback smoke evidence');
if (!central.includes('| `product` | admin + storefront | `in_progress` | `boundary_ready`')) {
  fail('central readiness board product status drift');
}
if (!central.includes(smokePath)) fail('central readiness board lacks runtime fallback smoke evidence');

console.log('[verify-product-runtime-fallback-smoke] Product no-compile runtime fallback smoke is executable and source-locked');
