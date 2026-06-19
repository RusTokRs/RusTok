import fs from 'node:fs';

function read(path) { return fs.readFileSync(path, 'utf8'); }
function json(path) { return JSON.parse(read(path)); }
function fail(message) { console.error(`[verify-blog-fba] ${message}`); process.exit(1); }
function hasAll(text, snippets, label) { for (const s of snippets) if (!text.includes(s)) fail(`${label} missing ${s}`); }
function sameSet(actual, expected, label) {
  const a = [...actual].sort().join('|');
  const e = [...expected].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

const registryPath = 'crates/rustok-blog/contracts/blog-fba-registry.json';
const evidencePath = 'crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json';
const providerPath = 'crates/rustok-comments/contracts/comments-fba-registry.json';
const registry = json(registryPath);
const evidence = json(evidencePath);
const provider = json(providerPath);

if (registry.schema_version !== 1) fail('registry schema_version drift');
if (registry.module !== 'blog' || registry.role !== 'consumer' || registry.status !== 'in_progress') fail('registry identity/status drift');
if (registry.consumer_profile !== 'blog_post_comments') fail('consumer profile drift');
const dependency = registry.provider_dependencies?.[0];
if (!dependency) fail('missing comments provider dependency');
if (dependency.module !== 'comments' || dependency.registry !== providerPath) fail('provider dependency identity drift');
if (dependency.contract_version !== provider.contract_version || dependency.port !== 'CommentsThreadPort') fail('provider contract/port drift');
if (provider.module !== 'comments' || provider.role !== 'provider' || provider.status !== 'in_progress') fail('comments provider status drift');
sameSet(dependency.operations, provider.ports?.[0]?.operations ?? [], 'consumer/provider operations');
sameSet(dependency.fallback_profiles, provider.consumers?.find(c => c.module === 'blog')?.fallback_profiles ?? [], 'consumer/provider fallback profiles');
sameSet(dependency.degraded_modes, provider.consumers?.find(c => c.module === 'blog')?.degraded_modes ?? [], 'consumer/provider degraded modes');
if (dependency.context !== 'rustok_api::ports::PortContext' || dependency.error !== 'rustok_api::ports::PortError') fail('consumer context/error drift');

const manifest = read('crates/rustok-blog/rustok-module.toml');
hasAll(manifest, ['[fba.consumer]', 'registry = "contracts/blog-fba-registry.json"', 'profile = "blog_post_comments"', 'comments.thread.v1'], 'manifest');

if (evidence.generated_from !== registryPath || evidence.status !== registry.contract_tests.status) fail('evidence header drift');
sameSet(evidence.cases.map(c => c.operation), registry.contract_tests.cases.map(c => c.operation), 'evidence/registry cases');
sameSet(evidence.fallback_smoke.profiles, registry.contract_tests.fallback_smoke.profiles, 'fallback profiles');
sameSet(evidence.fallback_smoke.degraded_modes, registry.contract_tests.fallback_smoke.degraded_modes, 'degraded modes');

const plan = read('crates/rustok-blog/docs/implementation-plan.md');
hasAll(plan, ['- FBA status: `in_progress`', 'blog-fba-registry.json', 'CommentsThreadPort', 'blog-comments-consumer-static-matrix.json'], 'local plan');
const central = read('docs/modules/registry.md');
hasAll(central, ['| `blog` |', 'crates/rustok-blog/contracts/blog-fba-registry.json', '`in_progress` | `in_progress`'], 'central registry');
const unified = read('docs/research/fluid-backend-architecture-unified-plan.md');
hasAll(unified, ['`blog`', 'CommentsThreadPort', 'blog-fba-registry.json'], 'unified plan');

console.log('[verify-blog-fba] blog FBA comments consumer metadata and static evidence are consistent');
