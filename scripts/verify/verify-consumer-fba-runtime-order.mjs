import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const defaultRoot = process.env.CONSUMER_FBA_ROOT || process.cwd();

export class ConsumerFbaRuntimeOrderError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ConsumerFbaRuntimeOrderError';
  }
}

const fail = (message) => { throw new ConsumerFbaRuntimeOrderError(message); };
const sameSet = (actual, expected) =>
  Array.isArray(actual) && Array.isArray(expected) &&
  actual.length === expected.length && expected.every((item) => actual.includes(item));

function functionBody(source, name) {
  const signature = new RegExp(`(?:pub\\s+)?(?:async\\s+)?fn\\s+${name}\\s*\\(`, 'g');
  for (let match = signature.exec(source); match; match = signature.exec(source)) {
    const open = source.indexOf('{', match.index);
    const semicolon = source.indexOf(';', match.index);
    if (open < 0 || (semicolon >= 0 && semicolon < open)) continue;
    let depth = 0;
    for (let index = open; index < source.length; index += 1) {
      if (source[index] === '{') depth += 1;
      if (source[index] === '}' && --depth === 0) return source.slice(open + 1, index);
    }
  }
  return null;
}

function assertOrdered(body, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = body.indexOf(marker, previous + 1);
    if (index < 0) fail(`${label} source marker missing: ${marker}`);
    previous = index;
  }
}

function verifyAssertions({ read }, assertions, label) {
  for (const assertion of assertions ?? []) {
    const source = read(assertion.path);
    for (const marker of assertion.markers ?? []) {
      if (!source.includes(marker)) fail(`${label} ${assertion.name} missing ${marker}`);
    }
  }
}

function verifyManifestDegradedModes(read, manifestPath, modes, label) {
  const manifest = read(manifestPath);
  for (const mode of modes) {
    if (!manifest.includes(`"${mode}"`)) fail(`${label} manifest missing degraded mode ${mode}`);
  }
}

function verifyBlog({ read, json }) {
  const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
  const providerPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
  const smokePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json';
  const registry = json(registryPath);
  const provider = json(providerPath);
  const smoke = json(smokePath);
  const dependency = registry.provider_dependencies?.[0];

  if (registry.module !== 'blog' || registry.role !== 'consumer' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('blog registry identity/status drift');
  if (smoke.generated_from !== registryPath || smoke.status !== 'executable_no_compile') fail('blog smoke identity drift');
  if (registry.evidence.consumer_runtime_order_smoke !== smokePath || registry.evidence.consumer_runtime_order_smoke_runner !== smoke.runner) {
    fail('blog runtime-order evidence registry drift');
  }
  if (!dependency || dependency.registry !== providerPath || dependency.port !== 'CommentsThreadPort') fail('blog provider dependency drift');
  if (provider.module !== 'comments' || provider.role !== 'provider') fail('comments provider identity drift');
  if (!sameSet(smoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('blog fallback profile drift');
  if (!sameSet(smoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail('blog degraded mode drift');
  if (!sameSet(dependency.operations, provider.ports?.[0]?.operations ?? [])) fail('blog provider operation set drift');
  verifyManifestDegradedModes(read, 'crates/rustok-blog/rustok-module.toml', smoke.fallback_smoke.degraded_modes, 'blog');

  const service = read(smoke.source_contract.consumer_service);
  for (const entry of smoke.runtime_order) {
    if (!registry.contract_tests.cases.some((testCase) => testCase.operation === entry.operation)) {
      fail(`blog runtime order operation ${entry.operation} is not declared in registry cases`);
    }
    const body = functionBody(service, entry.function);
    if (!body) fail(`blog runtime order function missing: ${entry.function}`);
    assertOrdered(body, entry.markers, `blog ${entry.operation}`);
  }
  verifyAssertions({ read }, smoke.source_assertions, 'blog');
}

function verifySeo({ read, json }) {
  const registryPath = 'crates/rustok-seo/contracts/seo-fba-registry.json';
  const providerPath = 'crates/rustok-media/contracts/media-fba-registry.json';
  const providerFallbackPath = 'crates/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json';
  const smokePath = 'crates/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json';
  const registry = json(registryPath);
  const provider = json(providerPath);
  const providerFallback = json(providerFallbackPath);
  const smoke = json(smokePath);
  const dependency = registry.provider_dependencies?.[0];

  if (registry.module !== 'seo' || registry.role !== 'consumer' || !['in_progress', 'boundary_ready'].includes(registry.status)) fail('seo registry identity/status drift');
  if (smoke.generated_from !== registryPath || smoke.status !== 'executable_no_compile') fail('seo smoke identity drift');
  if (registry.evidence.consumer_runtime_order_smoke !== smokePath || registry.evidence.consumer_runtime_order_smoke_runner !== smoke.runner) {
    fail('seo runtime-order evidence registry drift');
  }
  if (!dependency || dependency.registry !== providerPath || dependency.port !== 'MediaAssetReadPort') fail('seo provider dependency drift');
  if (provider.module !== 'media' || provider.role !== 'provider') fail('media provider identity drift');
  if (!sameSet(smoke.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles)) fail('seo fallback profile drift');
  if (!sameSet(smoke.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes)) fail('seo degraded mode drift');
  verifyManifestDegradedModes(read, 'crates/rustok-seo/rustok-module.toml', smoke.fallback_smoke.degraded_modes, 'seo');
  if (smoke.fallback_smoke.provider_source !== providerFallbackPath || registry.contract_tests.fallback_smoke.provider_source !== providerFallbackPath) {
    fail('seo provider fallback source drift');
  }
  const providerFallbackModes = (providerFallback.degraded_modes ?? []).map((mode) => mode.name);
  for (const mode of smoke.fallback_smoke.degraded_modes) {
    if (!providerFallbackModes.includes(mode)) fail(`seo provider fallback smoke lacks ${mode}`);
  }
  const providerOperations = provider.ports?.[0]?.operations ?? [];
  for (const operation of dependency.operations) {
    if (!providerOperations.includes(operation)) fail(`seo provider operation ${operation} missing`);
  }

  for (const entry of smoke.runtime_order) {
    const source = read(entry.path);
    const body = functionBody(source, entry.function);
    if (!body) fail(`seo runtime order function missing: ${entry.function}`);
    assertOrdered(body, entry.markers, `seo ${entry.operation}`);
  }
  verifyAssertions({ read }, smoke.source_assertions, 'seo');
}

export function verifyConsumerFbaRuntimeOrder({ root = defaultRoot } = {}) {
  const read = (repoPath) => fs.readFileSync(path.join(root, repoPath), 'utf8');
  const json = (repoPath) => JSON.parse(read(repoPath));
  verifyBlog({ read, json });
  verifySeo({ read, json });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    verifyConsumerFbaRuntimeOrder();
    console.log('consumer FBA runtime order verified: blog, seo');
  } catch (error) {
    if (error instanceof ConsumerFbaRuntimeOrderError) {
      console.error(`consumer FBA runtime order failed: ${error.message}`);
      process.exit(1);
    }
    throw error;
  }
}
