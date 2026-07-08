#!/usr/bin/env node
// No-compile API surface contract guardrail for the platform verification plan.
// It verifies that GraphQL/REST optional module wiring stays manifest-driven and
// that the documented API plan points at this source-level evidence.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const failures = [];
const passes = [];

function read(rel) {
  return fs.readFileSync(path.join(root, rel), 'utf8');
}

function exists(rel) {
  return fs.existsSync(path.join(root, rel));
}

function pass(message) {
  passes.push(message);
}

function fail(message) {
  failures.push(message);
}

function walk(rel, predicate) {
  const result = [];
  const visit = (currentRel) => {
    for (const entry of fs.readdirSync(path.join(root, currentRel), { withFileTypes: true })) {
      const childRel = path.join(currentRel, entry.name);
      if (entry.isDirectory()) visit(childRel);
      else if (entry.isFile() && predicate(childRel)) result.push(childRel);
    }
  };
  visit(rel);
  return result;
}

function requireContains(rel, needle, message) {
  const content = read(rel);
  if (content.includes(needle)) pass(message);
  else fail(`${message} (${rel} missing ${JSON.stringify(needle)})`);
}

function requireNotContains(rel, needle, message) {
  const content = read(rel);
  if (!content.includes(needle)) pass(message);
  else fail(`${message} (${rel} unexpectedly contains ${JSON.stringify(needle)})`);
}

function moduleEntries() {
  const raw = read('modules.toml');
  const entries = [];
  const moduleRe = /^([A-Za-z0-9_]+)\s*=\s*\{([^\n]+)\}/gm;
  let match;
  while ((match = moduleRe.exec(raw))) {
    const slug = match[1];
    const body = match[2];
    const crateMatch = body.match(/crate\s*=\s*"([^"]+)"/);
    const pathMatch = body.match(/path\s*=\s*"([^"]+)"/);
    const required = /required\s*=\s*true/.test(body);
    if (crateMatch && pathMatch) {
      entries.push({ slug, crateName: crateMatch[1], modulePath: pathMatch[1], required });
    }
  }
  return entries;
}

function tableContains(rel, slug) {
  return read(rel).includes(`| \`${slug}\``) || read(rel).includes(`\`${slug}\``);
}

function inspectPackageManifest(entry) {
  const manifestRel = `${entry.modulePath}/rustok-module.toml`;
  if (!exists(manifestRel)) return null;
  const raw = read(manifestRel);
  return {
    rel: manifestRel,
    raw,
    hasGraphql: raw.includes('[provides.graphql]'),
    hasHttp: raw.includes('[provides.http]'),
    hasEntryType: /\[crate\][\s\S]*entry_type\s*=\s*"[^"]+"/.test(raw),
    slugMatches: new RegExp(`\[module\][\\s\\S]*slug\\s*=\\s*"${entry.slug}"`).test(raw),
  };
}

function isTestRel(rel) {
  return rel.includes(`${path.sep}tests${path.sep}`) || rel.endsWith(`${path.sep}tests.rs`);
}

function isInsideTestModule(source, index) {
  const before = source.slice(0, index);
  const lastCfgTest = before.lastIndexOf('#[cfg(test)]');
  if (lastCfgTest !== -1 && /#\[cfg\(test\)\]\s*mod\s+\w+/m.test(source.slice(lastCfgTest, index))) {
    return true;
  }
  const testModuleMatches = [...before.matchAll(/(?:^|\n)\s*(?:#\[cfg\(test\)\]\s*)?mod\s+\w*tests\b/g)];
  const lastTestModule =
    testModuleMatches.length > 0 ? testModuleMatches[testModuleMatches.length - 1].index ?? -1 : -1;
  if (lastTestModule === -1) return false;
  const lastNonTestModule = before.lastIndexOf('\nmod ');
  return lastTestModule >= lastNonTestModule;
}

function shouldForbidSystemAuthority(rel) {
  return (
    rel.endsWith(`${path.sep}graphql${path.sep}query.rs`) ||
    rel.includes(`${path.sep}storefront${path.sep}`) ||
    rel.includes(`${path.sep}controllers${path.sep}`) ||
    rel.endsWith('ports.rs') ||
    rel.endsWith('services.rs')
  );
}

// GraphQL composition root must use generated optional modules instead of a hardcoded list.
requireContains('apps/server/src/graphql/schema.rs', 'schema_codegen::OptionalModuleQuery', 'GraphQL Query includes generated optional module root');
requireContains('apps/server/src/graphql/schema.rs', 'schema_codegen::OptionalModuleMutation', 'GraphQL Mutation includes generated optional module root');
requireContains('apps/server/src/graphql/schema.rs', 'include!(concat!(env!("OUT_DIR"), "/graphql_schema_codegen.rs"))', 'GraphQL schema uses build-time generated code');

// Build script must read both the platform module manifest and package-local transport declarations.
requireContains('apps/server/build.rs', 'RUSTOK_MODULES_MANIFEST', 'Build script supports explicit module manifest path');
requireContains('apps/server/build.rs', 'apply_module_package_manifest', 'Build script imports module-local rustok-module transport declarations');
requireContains('apps/server/build.rs', 'render_graphql_codegen', 'Build script renders optional GraphQL composition');
requireContains('apps/server/build.rs', 'render_routes_codegen', 'Build script renders optional REST route mounting');
requireContains('apps/server/build.rs', 'provides.graphql', 'Build script schema understands [provides.graphql]');
requireContains('apps/server/build.rs', 'provides.http', 'Build script schema understands [provides.http]');

// API plan/docs must expose this guardrail as no-compile evidence.
requireContains('docs/verification/platform-api-surfaces-verification-plan.md', 'verify-api-surface-contract.mjs', 'API verification plan lists no-compile source guardrail');
requireContains('scripts/verify/README.md', 'verify-api-surface-contract.mjs', 'Verification README documents API surface guardrail');

// Neutral Port* contracts belong to rustok-api without pulling runtime crates
// into its default dependency graph.
requireContains('crates/rustok-api/src/ports.rs', 'pub struct PortContext', 'rustok-api owns PortContext');
requireContains('crates/rustok-api/src/ports.rs', 'pub struct PortError', 'rustok-api owns PortError');
requireNotContains('crates/rustok-api/src/ports.rs', 'rustok_core', 'rustok-api Port contracts do not re-export core');
requireContains('crates/rustok-api/src/runtime.rs', 'pub struct HostRuntimeContext', 'rustok-api owns neutral host runtime context');
requireContains('crates/rustok-api/src/runtime.rs', 'pub fn db_clone(&self) -> DatabaseConnection', 'HostRuntimeContext exposes DB access without Loco');
requireContains('crates/rustok-api/src/runtime.rs', 'pub fn with_shared_value<T>', 'HostRuntimeContext accepts host-provided typed handles without Loco');
requireContains('crates/rustok-api/src/runtime.rs', 'pub fn shared_get<T>(&self) -> Option<T>', 'HostRuntimeContext exposes typed shared handles without Loco');
requireContains('apps/server/src/services/app_router.rs', 'HostRuntimeContext::new(ctx.db.clone())', 'server function context provides neutral runtime context');
requireContains('apps/server/src/services/app_router.rs', 'with_shared_value(storage)', 'server function context provides storage through neutral host runtime context');
requireContains('apps/server/src/services/server_runtime_context.rs', 'pub struct ServerRuntimeContext', 'server owns neutral runtime context for server services');
requireContains('apps/server/src/services/server_runtime_context.rs', 'pub fn db(&self) -> &DatabaseConnection', 'ServerRuntimeContext exposes DB access without service-level Loco dependency');
requireContains('apps/server/src/services/server_runtime_context.rs', 'pub fn shared_get<T>(&self) -> Option<T>', 'ServerRuntimeContext exposes typed shared-store access behind server boundary');
requireContains('apps/server/src/services/server_runtime_context.rs', 'impl FromRef<AppContext> for ServerRuntimeContext', 'Axum can extract the neutral server runtime from the current host state');
requireNotContains('apps/server/src/services/settings_service.rs', 'loco_rs', 'settings service does not depend on Loco runtime context');
requireContains('apps/server/src/services/settings_service.rs', 'ServerRuntimeContext', 'settings service consumes server runtime context');
for (const rel of [
  'apps/server/src/services/build_event_hub.rs',
  'apps/server/src/services/field_definition_cache.rs',
  'apps/server/src/services/marketplace_catalog.rs',
]) {
  requireNotContains(rel, 'loco_rs', `${rel} does not depend on Loco runtime context`);
  requireContains(rel, 'ServerRuntimeContext', `${rel} consumes server runtime context`);
}
requireNotContains('apps/server/src/services/event_bus.rs', 'use loco_rs::app::AppContext', 'event bus service does not consume Loco AppContext');
requireNotContains('apps/server/src/services/event_bus.rs', 'rustok_outbox::loco', 'server event bus does not re-export the outbox Loco adapter');
requireContains('apps/server/src/services/event_bus.rs', 'ServerRuntimeContext', 'event bus service consumes server runtime context');
requireContains('apps/server/src/services/event_bus.rs', 'pub fn transactional_event_bus_from_context(ctx: &ServerRuntimeContext)', 'transactional event bus is built from the server runtime context');
requireNotContains('apps/server/src/services/runtime_guardrails.rs', 'loco_rs', 'runtime guardrails service does not depend on Loco runtime context');
requireContains('apps/server/src/services/runtime_guardrails.rs', 'ServerRuntimeContext', 'runtime guardrails service consumes server runtime context');
requireNotContains('apps/server/src/services/rbac_consistency.rs', 'loco_rs', 'RBAC consistency service does not depend on Loco runtime context');
requireContains('apps/server/src/services/rbac_consistency.rs', 'ServerRuntimeContext', 'RBAC consistency service consumes server runtime context');
requireNotContains('apps/server/src/services/release_backend.rs', 'loco_rs', 'release backend service does not depend on Loco runtime context');
requireContains('apps/server/src/services/release_backend.rs', 'ServerRuntimeContext', 'release backend service consumes server runtime context');
requireNotContains('apps/server/src/services/build_executor.rs', 'loco_rs', 'build executor service does not depend on Loco runtime context');
requireContains('apps/server/src/services/build_executor.rs', 'ServerRuntimeContext', 'build executor service consumes server runtime context');
requireNotContains('apps/server/src/services/event_transport_factory.rs', 'loco_rs', 'event transport factory does not depend on Loco runtime context');
requireContains('apps/server/src/services/event_transport_factory.rs', 'ServerRuntimeContext', 'event transport factory consumes server runtime context');
requireContains('apps/server/src/services/module_event_dispatcher.rs', 'ctx: &ServerRuntimeContext', 'module event dispatcher spawn consumes server runtime context');
requireNotContains('apps/server/src/services/module_event_dispatcher.rs', 'loco_rs::app::AppContext', 'module runtime extension assembly does not consume Loco AppContext');
requireNotContains('apps/server/src/services/email.rs', 'AppContext', 'email service factory does not depend on Loco AppContext');
requireContains('apps/server/src/services/email.rs', 'ServerRuntimeContext', 'email service factory consumes server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'pub fn module_runtime_extensions_from_ctx', 'module runtime extensions helper is owned by app runtime');
requireContains('apps/server/src/services/app_runtime.rs', 'ctx: &ServerRuntimeContext', 'app runtime helpers consume server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'init_storage(ctx: &ServerRuntimeContext)', 'storage bootstrap helper consumes server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'init_marketplace_catalog(ctx: &ServerRuntimeContext)', 'marketplace catalog bootstrap helper consumes server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'fn init_alloy_runtime(ctx: &ServerRuntimeContext)', 'Alloy bootstrap helper consumes server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'alloy::build_alloy_runtime', 'server registers Alloy runtime through host-neutral construction');
requireNotContains('crates/alloy/src/runtime.rs', 'loco_rs', 'Alloy runtime core does not consume Loco AppContext');
requireNotContains('crates/alloy/src/runtime.rs', 'AppContext', 'Alloy runtime core exposes host-neutral construction only');
requireNotContains('crates/alloy/src/graphql/mod.rs', 'loco_rs', 'Alloy GraphQL resolvers do not consume Loco AppContext');
requireContains('crates/alloy/src/graphql/mod.rs', 'SharedAlloyRuntime', 'Alloy GraphQL resolvers consume schema-owned runtime data');
requireContains('apps/server/src/graphql/schema.rs', 'alloy::SharedAlloyRuntime', 'GraphQL schema receives Alloy runtime as schema-owned data');
requireContains('crates/alloy/src/controllers/mod.rs', 'pub struct AlloyHttpRuntime', 'Alloy HTTP controllers use a narrow runtime state');
requireContains('crates/alloy/src/controllers/mod.rs', 'State(runtime): State<AlloyHttpRuntime>', 'Alloy HTTP handlers consume narrow runtime state');
requireNotContains('crates/alloy/src/controllers/mod.rs', 'State(ctx): State<AppContext>', 'Alloy HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/alloy/src/controllers/mod.rs', 'ctx.shared_store', 'Alloy HTTP handlers do not use Loco shared store as service locator');
requireNotContains('crates/rustok-ai/Cargo.toml', 'loco-rs', 'AI capability crate does not depend on Loco');
requireNotContains('crates/rustok-ai/src/graphql/mutation.rs', 'loco_rs', 'AI GraphQL mutations do not consume Loco AppContext');
requireNotContains('crates/rustok-ai/src/service.rs', 'AppContext', 'AI management service does not consume Loco AppContext');
requireNotContains('crates/rustok-ai/src/direct.rs', 'rustok_outbox::loco', 'AI direct execution does not consume outbox Loco adapter');
requireContains('crates/rustok-ai/src/service/types.rs', 'pub struct AiHostRuntime', 'AI owns a host-neutral runtime contract');
requireContains('apps/server/src/graphql/schema.rs', 'rustok_ai::AiHostRuntime', 'GraphQL schema receives AI runtime as schema-owned data');
requireContains('apps/server/src/services/app_runtime.rs', 'fn init_rate_limit_layers(\n    ctx: &ServerRuntimeContext', 'rate-limit bootstrap consumes server runtime context');
requireContains('apps/server/src/services/app_runtime.rs', 'fn build_namespaced_rate_limiter(\n    ctx: &ServerRuntimeContext', 'rate-limit shared handles are inserted through server runtime context');
requireContains('apps/server/src/services/graphql_schema.rs', 'storage_from_ctx(ctx: &ServerRuntimeContext)', 'GraphQL schema storage helper consumes server runtime context');
requireContains('apps/server/src/services/graphql_schema.rs', 'ctx.shared_get::<SharedGraphqlSchema>()', 'GraphQL schema cache uses server runtime context shared store');
requireNotContains('apps/server/src/services/graphql_schema.rs', 'loco_rs', 'GraphQL schema service does not depend on Loco');
requireContains('apps/server/src/services/graphql_schema.rs', 'init_graphql_schema(ctx: &ServerRuntimeContext)', 'GraphQL schema service consumes the server runtime context');
requireNotContains('crates/rustok-content-orchestration/Cargo.toml', 'loco-rs', 'content orchestration crate does not depend on Loco');
requireNotContains('crates/rustok-content-orchestration/src/lib.rs', 'AppContext', 'content orchestration runtime helpers do not consume Loco AppContext');
requireNotContains('crates/rustok-content-orchestration/src/graphql.rs', 'loco_rs', 'content orchestration GraphQL resolvers do not consume Loco AppContext');
requireContains('crates/rustok-content-orchestration/src/lib.rs', 'build_content_orchestration_service', 'content orchestration exposes host-neutral service construction');
requireContains('apps/server/src/services/app_runtime.rs', 'build_content_orchestration_service', 'server registers content orchestration through host-neutral construction');
requireContains('apps/server/src/graphql/schema.rs', 'SharedContentOrchestrationService', 'GraphQL schema receives content orchestration as schema-owned data');
requireNotContains('crates/rustok-commerce/src/storefront_checkout_runtime.rs', 'loco_rs', 'commerce storefront checkout runtime does not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/storefront_checkout_runtime.rs', 'rustok_outbox::loco', 'commerce storefront checkout runtime does not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/storefront_checkout_runtime.rs', 'pub struct StorefrontCheckoutRuntime', 'commerce storefront checkout exposes a host-neutral runtime contract');
requireContains('crates/rustok-commerce/src/controllers/mod.rs', 'pub struct CommerceHttpRuntime', 'commerce HTTP controllers expose a narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/products.rs', 'AppContext', 'commerce product HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/products.rs', 'rustok_outbox::loco', 'commerce product HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/products.rs', 'State(runtime): State<crate::controllers::CommerceHttpRuntime>', 'commerce product HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/products.rs', 'AppContext', 'commerce admin product HTTP wrapper does not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/products.rs', 'rustok_outbox::loco', 'commerce admin product HTTP wrapper does not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/products.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin product HTTP wrapper consumes narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/store/products.rs', 'AppContext', 'commerce storefront product/catalog HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/store/products.rs', 'rustok_outbox::loco', 'commerce storefront product/catalog HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/store/products.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce storefront product/catalog HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/store/orders.rs', 'AppContext', 'commerce storefront order HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/store/orders.rs', 'rustok_outbox::loco', 'commerce storefront order HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/store/orders.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce storefront order HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/store/carts.rs', 'AppContext', 'commerce storefront cart HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/store/carts.rs', 'rustok_outbox::loco', 'commerce storefront cart HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/store/carts.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce storefront cart HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/store/checkout.rs', 'AppContext', 'commerce storefront checkout HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/store/checkout.rs', 'rustok_outbox::loco', 'commerce storefront checkout HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/store/checkout.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce storefront checkout HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/fulfillments.rs', 'AppContext', 'commerce admin fulfillment HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/fulfillments.rs', 'rustok_outbox::loco', 'commerce admin fulfillment HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/fulfillments.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin fulfillment HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/shipping.rs', 'AppContext', 'commerce admin shipping HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/shipping.rs', 'rustok_outbox::loco', 'commerce admin shipping HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/shipping.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin shipping HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/payments.rs', 'AppContext', 'commerce admin payment HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/payments.rs', 'rustok_outbox::loco', 'commerce admin payment HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/payments.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin payment HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/orders.rs', 'AppContext', 'commerce admin order HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/orders.rs', 'rustok_outbox::loco', 'commerce admin order HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/orders.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin order HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/changes.rs', 'AppContext', 'commerce admin order-change HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/changes.rs', 'rustok_outbox::loco', 'commerce admin order-change HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/changes.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin order-change HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-commerce/src/controllers/admin/returns.rs', 'AppContext', 'commerce admin return HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-commerce/src/controllers/admin/returns.rs', 'rustok_outbox::loco', 'commerce admin return HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-commerce/src/controllers/admin/returns.rs', 'State(runtime): State<CommerceHttpRuntime>', 'commerce admin return HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-blog/src/controllers/posts.rs', 'AppContext', 'blog post HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-blog/src/controllers/comments.rs', 'AppContext', 'blog comment HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-blog/src/controllers/posts.rs', 'rustok_outbox::loco', 'blog post HTTP handlers do not consume outbox Loco adapter');
requireNotContains('crates/rustok-blog/src/controllers/comments.rs', 'rustok_outbox::loco', 'blog comment HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-blog/src/controllers/mod.rs', 'pub struct BlogHttpRuntime', 'blog HTTP controllers use a narrow runtime state');
requireNotContains('crates/rustok-pages/src/controllers/mod.rs', 'rustok_outbox::loco', 'pages HTTP handlers do not consume outbox Loco adapter');
requireContains('crates/rustok-pages/src/controllers/mod.rs', 'pub struct PagesHttpRuntime', 'pages HTTP controllers use a narrow runtime state');
requireContains('crates/rustok-pages/src/controllers/mod.rs', 'State(runtime): State<PagesHttpRuntime>', 'pages HTTP handlers consume narrow runtime state');
for (const rel of [
  'crates/rustok-forum/src/controllers/categories.rs',
  'crates/rustok-forum/src/controllers/topics.rs',
  'crates/rustok-forum/src/controllers/replies.rs',
  'crates/rustok-forum/src/controllers/users.rs',
  'crates/rustok-forum/src/controllers/widgets.rs',
]) {
  requireNotContains(rel, 'AppContext', `${rel} handlers do not consume Loco AppContext`);
  requireNotContains(rel, 'rustok_outbox::loco', `${rel} handlers do not consume outbox Loco adapter`);
  requireContains(rel, 'ForumHttpRuntime', `${rel} handlers consume narrow forum runtime state`);
}
requireNotContains('crates/rustok-forum/Cargo.toml', 'loco-adapter', 'forum crate does not depend on the outbox Loco adapter feature');
requireContains('crates/rustok-forum/src/controllers/mod.rs', 'pub struct ForumHttpRuntime', 'forum HTTP controllers use a narrow runtime state');
requireContains('crates/rustok-media/src/controllers/mod.rs', 'pub struct MediaHttpRuntime', 'media HTTP controllers use a narrow runtime state');
requireContains('crates/rustok-media/src/controllers/mod.rs', 'State(runtime): State<MediaHttpRuntime>', 'media HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-media/src/controllers/mod.rs', 'State(ctx): State<AppContext>', 'media HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-media/src/controllers/mod.rs', 'ctx.shared_store', 'media HTTP handlers do not use Loco shared store as service locator');
for (const rel of [
  'crates/rustok-workflow/src/controllers/workflows.rs',
  'crates/rustok-workflow/src/controllers/steps.rs',
  'crates/rustok-workflow/src/controllers/executions.rs',
  'crates/rustok-workflow/src/controllers/webhook.rs',
]) {
  requireNotContains(rel, 'AppContext', `${rel} handlers do not consume Loco AppContext`);
  requireContains(rel, 'WorkflowHttpRuntime', `${rel} handlers consume narrow workflow runtime state`);
}
requireContains('crates/rustok-workflow/src/controllers/mod.rs', 'pub struct WorkflowHttpRuntime', 'workflow HTTP controllers use a narrow runtime state');
requireContains('crates/rustok-seo/src/controllers/mod.rs', 'pub struct SeoHttpRuntime', 'SEO HTTP controllers use a narrow runtime state');
requireContains('crates/rustok-seo/src/controllers/mod.rs', 'State(runtime): State<SeoHttpRuntime>', 'SEO HTTP handlers consume narrow runtime state');
requireNotContains('crates/rustok-seo/src/controllers/mod.rs', 'State(ctx): State<AppContext>', 'SEO HTTP handlers do not consume Loco AppContext');
requireNotContains('crates/rustok-seo/src/controllers/mod.rs', 'rustok_outbox::loco', 'SEO HTTP handlers do not consume outbox Loco adapter');
requireNotContains('crates/rustok-seo/Cargo.toml', 'loco-adapter', 'SEO crate does not depend on the outbox Loco adapter feature');
requireContains('apps/server/src/services/app_lifecycle.rs', 'let runtime_ctx = ServerRuntimeContext::from_loco_app_context(ctx);', 'runtime worker lifecycle isolates current Loco boundary adapter');
requireNotContains('apps/server/src/services/app_lifecycle.rs', 'RustokSettings::from_settings(&ctx.config.settings)', 'runtime worker lifecycle does not parse settings from Loco config directly');
for (const rel of [
  'apps/server/src/middleware/channel.rs',
  'apps/server/src/middleware/locale.rs',
  'apps/server/src/middleware/tenant.rs',
]) {
  requireNotContains(rel, 'loco_rs::app::AppContext', `${rel} does not consume Loco AppContext`);
  requireContains(rel, 'ServerRuntimeContext', `${rel} consumes server runtime context`);
}
requireNotContains('apps/server/src/middleware/auth_context.rs', 'loco_rs::app::AppContext', 'auth context middleware does not consume Loco AppContext');
requireContains('apps/server/src/middleware/auth_context.rs', 'ServerAuthRuntime', 'auth context middleware consumes narrow auth runtime');
requireNotContains('apps/server/src/extractors/auth.rs', 'loco_rs::app::AppContext', 'auth extractor does not consume Loco AppContext');
requireContains('apps/server/src/extractors/auth.rs', 'ServerAuthRuntime', 'auth extractor consumes narrow auth runtime');
requireNotContains('apps/server/src/extractors/rbac.rs', 'loco_rs::app::AppContext', 'RBAC permission extractor macro does not require Loco AppContext');
requireContains('apps/server/src/extractors/rbac.rs', 'ServerAuthRuntime', 'RBAC permission extractor macro consumes auth runtime bound');
requireNotContains('apps/server/src/guards/module.rs', 'loco_rs::app::AppContext', 'module guard does not consume Loco AppContext');
requireContains('apps/server/src/guards/module.rs', 'ServerRuntimeContext', 'module guard consumes neutral server runtime context');
requireNotContains('apps/server/src/channels/mod.rs', 'loco_rs::app::AppContext', 'channel contract does not expose Loco AppContext');
requireContains('apps/server/src/channels/mod.rs', 'ServerRuntimeContext', 'channel contract exposes neutral server runtime context');
requireNotContains('apps/server/src/services/auth_lifecycle.rs', 'AppContext', 'auth lifecycle service does not expose Loco compatibility entrypoints');
for (const method of [
  'create_user_runtime',
  'register_runtime',
  'login_runtime',
  'refresh_runtime',
  'confirm_password_reset_runtime',
  'update_profile_runtime',
  'change_password_runtime',
  'logout_runtime',
  'list_sessions_runtime',
  'revoke_session_runtime',
  'revoke_all_other_sessions_runtime',
]) {
  requireContains('apps/server/src/services/auth_lifecycle.rs', method, `auth lifecycle exposes ${method} without Loco AppContext`);
  requireContains('apps/server/src/services/auth_lifecycle_provider.rs', method, `auth lifecycle provider consumes ${method}`);
}
requireNotContains('apps/server/src/services/auth_lifecycle_provider.rs', 'loco_rs::app::AppContext', 'auth lifecycle provider does not retain Loco AppContext');
requireContains('apps/server/src/services/auth_lifecycle_provider.rs', 'auth_config: AuthConfig', 'auth lifecycle provider owns explicit auth config dependency');
for (const rel of [
  'apps/server/src/graphql/settings/query.rs',
  'apps/server/src/graphql/settings/mutation.rs',
  'apps/server/src/graphql/system.rs',
]) {
  requireNotContains(rel, 'loco_rs::app::AppContext', `${rel} does not consume Loco AppContext`);
  requireContains(rel, 'ServerRuntimeContext', `${rel} consumes neutral server runtime data`);
}
requireContains('apps/server/src/graphql/settings/mutation.rs', 'ctx.data::<TransactionalEventBus>()?', 'settings GraphQL mutation consumes the schema-owned transactional event bus');
requireContains('apps/server/src/controllers/graphql.rs', '.data(runtime_ctx)', 'GraphQL HTTP requests receive neutral server runtime data');
requireContains('apps/server/src/controllers/graphql.rs', 'data.insert(runtime_ctx);', 'GraphQL WebSocket connections receive neutral server runtime data');
requireNotContains('apps/server/src/controllers/graphql.rs', 'loco_rs::app::AppContext', 'GraphQL controller handlers do not consume Loco AppContext');
requireContains('apps/server/src/controllers/graphql.rs', 'State(runtime_ctx): State<ServerRuntimeContext>', 'GraphQL controller extracts neutral runtime state');
requireContains('apps/server/src/controllers/graphql.rs', 'State(auth_runtime): State<ServerAuthRuntime>', 'GraphQL WebSocket controller extracts narrow auth state');
requireNotContains('apps/server/src/controllers/users.rs', 'loco_rs::app::AppContext', 'users controller handlers do not consume Loco AppContext');
requireContains('apps/server/src/controllers/users.rs', 'State<ServerRuntimeContext>', 'users controller extracts neutral runtime state');
requireNotContains('apps/server/src/controllers/metrics.rs', 'loco_rs::app::AppContext', 'metrics controller does not consume Loco AppContext');
requireContains('apps/server/src/controllers/metrics.rs', 'State(ctx): State<ServerRuntimeContext>', 'metrics controller extracts neutral runtime state');
requireContains('apps/server/src/controllers/metrics.rs', 'State(email_runtime): State<ServerEmailRuntime>', 'metrics controller extracts narrow email runtime state');
requireNotContains('apps/server/src/controllers/health.rs', 'loco_rs::app::AppContext', 'health controller does not consume Loco AppContext');
requireNotContains('apps/server/src/controllers/health.rs', 'loco_rs::controller::format', 'health controller does not use Loco response formatting');
requireContains('apps/server/src/controllers/health.rs', 'rustok_web::json_response', 'health controller uses rustok-web JSON response helper');
requireContains('apps/server/src/controllers/health.rs', 'State(ctx): State<ServerRuntimeContext>', 'health controller extracts neutral runtime state');
requireContains('apps/server/src/controllers/health.rs', 'State(email_runtime): State<ServerEmailRuntime>', 'health readiness extracts narrow email runtime state');
for (const rel of [
  'apps/server/src/controllers/channel.rs',
  'apps/server/src/controllers/flex.rs',
]) {
  requireNotContains(rel, 'loco_rs::app::AppContext', `${rel} handlers do not consume Loco AppContext`);
  requireContains(rel, 'State<ServerRuntimeContext>', `${rel} extracts neutral runtime state`);
}
requireContains('apps/server/src/controllers/flex.rs', 'fn test_runtime_context', 'Flex controller tests use the neutral runtime fixture');
requireNotContains('apps/server/src/controllers/auth.rs', 'loco_rs::app::AppContext', 'auth controller does not consume Loco AppContext');
requireContains('apps/server/src/controllers/auth.rs', 'State(ctx): State<ServerAuthRuntime>', 'auth controller extracts narrow auth runtime state');
requireContains('apps/server/src/controllers/auth.rs', 'State(email_runtime): State<ServerEmailRuntime>', 'auth email endpoints extract narrow email runtime state');
requireNotContains('apps/server/src/controllers/auth.rs', 'auth_config_from_ctx', 'auth controller reads config from the narrow auth runtime');
requireNotContains('apps/server/src/controllers/oauth_metadata.rs', 'loco_rs::app::AppContext', 'OAuth metadata controller does not consume Loco AppContext');
requireContains('apps/server/src/controllers/oauth_metadata.rs', 'State(ctx): State<ServerAuthRuntime>', 'OAuth metadata controller extracts narrow auth runtime state');
requireNotContains('apps/server/src/controllers/oauth.rs', 'loco_rs::app::AppContext', 'OAuth REST controller does not consume Loco AppContext');
requireNotContains('apps/server/src/controllers/oauth.rs', 'auth_config_from_ctx', 'OAuth REST controller reads config from the narrow auth runtime');
requireContains('apps/server/src/controllers/oauth.rs', 'State(ctx): State<ServerAuthRuntime>', 'OAuth token/consent handlers extract narrow auth runtime state');
requireContains('apps/server/src/controllers/oauth.rs', 'State(ctx): State<ServerRuntimeContext>', 'OAuth non-auth runtime handlers extract neutral runtime state');
requireNotContains('apps/server/src/controllers/marketplace_registry.rs', 'loco_rs::app::AppContext', 'marketplace registry controller does not consume Loco AppContext');
requireContains('apps/server/src/controllers/marketplace_registry.rs', 'State(ctx): State<ServerRuntimeContext>', 'marketplace registry controller extracts neutral runtime state');
requireContains('apps/server/src/controllers/marketplace_registry.rs', 'shared_get::<rustok_storage::StorageService>()', 'marketplace registry artifact paths read storage through neutral runtime state');
for (const rel of [
  'apps/server/src/controllers/admin_events.rs',
  'apps/server/src/controllers/installer.rs',
  'apps/server/src/controllers/mcp.rs',
  'apps/server/src/controllers/swagger.rs',
  'apps/server/src/channels/builds.rs',
]) {
  requireNotContains(rel, 'loco_rs::app::AppContext', `${rel} does not consume Loco AppContext`);
  requireContains(rel, 'ServerRuntimeContext', `${rel} consumes neutral runtime state`);
}
requireContains('apps/server/src/services/server_runtime_context.rs', 'pub fn shared_map<T, R>', 'server runtime supports scoped reads of non-clone shared handles');
for (const rel of [
  'apps/server/src/graphql/mutations.rs',
  'apps/server/src/graphql/queries.rs',
  'apps/server/src/graphql/subscriptions.rs',
  'apps/server/src/graphql/types.rs',
]) {
  requireNotContains(rel, 'loco_rs::app::AppContext', `${rel} does not consume Loco AppContext`);
  requireContains(rel, 'DatabaseConnection', `${rel} consumes the schema-owned database handle`);
}
for (const rel of walk('apps/server/src/graphql', (file) => file.endsWith('.rs'))) {
  requireNotContains(rel, 'loco_rs', `GraphQL implementation is Loco-independent: ${rel}`);
}
for (const rel of [
  'crates/rustok-index/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-outbox/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-tenant/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-region/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-comments/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-workflow/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-media/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-customer/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-channel/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-ai/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-product/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-product/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-seo/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-mcp/admin/src/transport/native_server_adapter.rs',
  'crates/rustok-inventory/admin/src/native.rs',
  'crates/rustok-cart/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-region/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-pages/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-blog/storefront/src/transport/native_server_adapter.rs',
  'crates/rustok-order/storefront/src/transport/native_server_adapter/raw_adapter.rs',
  'crates/rustok-fulfillment/storefront/src/transport/native_server_adapter/raw_adapter.rs',
  'crates/rustok-payment/storefront/src/transport/native_server_adapter/raw_adapter.rs',
  'crates/rustok-pricing/storefront/src/transport/native_server_adapter.rs',
]) {
  requireNotContains(rel, 'loco_rs', `${rel} does not depend on Loco runtime context`);
  requireNotContains(rel, 'rustok_outbox::loco', `${rel} does not consume the outbox Loco adapter`);
  requireContains(rel, 'HostRuntimeContext', `${rel} consumes neutral host runtime context`);
}
requireNotContains('crates/rustok-tenant/admin/Cargo.toml', 'loco-rs', 'tenant admin crate does not depend on Loco');
requireNotContains('crates/rustok-region/admin/Cargo.toml', 'loco-rs', 'region admin crate does not depend on Loco');
requireNotContains('crates/rustok-comments/admin/Cargo.toml', 'loco-rs', 'comments admin crate does not depend on Loco');
requireNotContains('crates/rustok-workflow/admin/Cargo.toml', 'loco-rs', 'workflow admin crate does not depend on Loco');
requireNotContains('crates/rustok-media/admin/Cargo.toml', 'loco-rs', 'media admin crate does not depend on Loco');
requireNotContains('crates/rustok-customer/admin/Cargo.toml', 'loco-rs', 'customer admin crate does not depend on Loco');
requireNotContains('crates/rustok-channel/admin/Cargo.toml', 'loco-rs', 'channel admin crate does not depend on Loco');
requireNotContains('crates/rustok-ai/admin/Cargo.toml', 'loco-rs', 'AI admin crate does not depend on Loco');
requireNotContains('crates/rustok-ai/admin/Cargo.toml', 'loco-adapter', 'AI admin crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-product/admin/Cargo.toml', 'loco-rs', 'product admin crate does not depend on Loco');
requireNotContains('crates/rustok-product/admin/Cargo.toml', 'loco-adapter', 'product admin crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-product/storefront/Cargo.toml', 'loco-rs', 'product storefront crate does not depend on Loco');
requireNotContains('crates/rustok-product/storefront/Cargo.toml', 'loco-adapter', 'product storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-seo/admin/Cargo.toml', 'loco-rs', 'SEO admin crate does not depend on Loco');
requireNotContains('crates/rustok-seo/admin/Cargo.toml', 'loco-adapter', 'SEO admin crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-mcp/admin/Cargo.toml', 'loco-rs', 'MCP admin crate does not depend on Loco');
requireNotContains('crates/rustok-mcp/admin/Cargo.toml', 'loco-adapter', 'MCP admin crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-inventory/admin/Cargo.toml', 'loco-rs', 'inventory admin crate does not depend on Loco');
requireNotContains('crates/rustok-inventory/admin/Cargo.toml', 'loco-adapter', 'inventory admin crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-cart/storefront/Cargo.toml', 'loco-rs', 'cart storefront crate does not depend on Loco');
requireNotContains('crates/rustok-cart/storefront/Cargo.toml', 'loco-adapter', 'cart storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-commerce/storefront/Cargo.toml', 'loco-rs', 'commerce storefront crate does not depend on Loco');
requireNotContains('crates/rustok-commerce/storefront/Cargo.toml', 'loco-adapter', 'commerce storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-region/storefront/Cargo.toml', 'loco-rs', 'region storefront crate does not depend on Loco');
requireNotContains('crates/rustok-pages/storefront/Cargo.toml', 'loco-rs', 'pages storefront crate does not depend on Loco');
requireNotContains('crates/rustok-pages/storefront/Cargo.toml', 'loco-adapter', 'pages storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-blog/storefront/Cargo.toml', 'loco-rs', 'blog storefront crate does not depend on Loco');
requireNotContains('crates/rustok-blog/storefront/Cargo.toml', 'loco-adapter', 'blog storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-order/storefront/Cargo.toml', 'loco-rs', 'order storefront crate does not depend on Loco');
requireNotContains('crates/rustok-order/storefront/Cargo.toml', 'loco-adapter', 'order storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-fulfillment/storefront/Cargo.toml', 'loco-rs', 'fulfillment storefront crate does not depend on Loco');
requireNotContains('crates/rustok-fulfillment/storefront/Cargo.toml', 'loco-adapter', 'fulfillment storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-payment/storefront/Cargo.toml', 'loco-rs', 'payment storefront crate does not depend on Loco');
requireNotContains('crates/rustok-payment/storefront/Cargo.toml', 'loco-adapter', 'payment storefront crate does not enable the outbox Loco adapter feature');
requireNotContains('crates/rustok-pricing/storefront/Cargo.toml', 'loco-rs', 'pricing storefront crate does not depend on Loco');
requireNotContains('crates/rustok-pricing/storefront/Cargo.toml', 'loco-adapter', 'pricing storefront crate does not enable the outbox Loco adapter feature');
requireContains('crates/rustok-api/src/permissions.rs', 'pub struct Permission', 'rustok-api owns Permission');
requireContains('crates/rustok-api/src/permissions.rs', 'pub enum Action', 'rustok-api owns Action');
requireContains('crates/rustok-api/src/permissions.rs', 'pub enum Resource', 'rustok-api owns Resource');
requireContains('crates/rustok-api/src/locale.rs', 'pub const PLATFORM_FALLBACK_LOCALE', 'rustok-api owns platform fallback locale');
requireContains('crates/rustok-api/src/locale.rs', 'pub fn extract_locale_tag_from_header', 'rustok-api owns Accept-Language parsing');
requireNotContains('crates/rustok-api/Cargo.toml', 'rustok-core', 'rustok-api never depends on rustok-core');
for (const rel of walk('crates/rustok-api/src', (file) => file.endsWith('.rs'))) {
  requireNotContains(rel, 'rustok_core', `rustok-api source is core-independent: ${rel}`);
}
requireNotContains('crates/rustok-api/Cargo.toml', 'rustok-outbox', 'rustok-api does not depend on outbox runtime');
requireNotContains('crates/rustok-api/Cargo.toml', 'loco-rs', 'rustok-api default manifest does not own Loco runtime');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod ports;', 'rustok-core does not publish a ports module');
requireNotContains('crates/rustok-core/src/lib.rs', 'PortContext', 'rustok-core does not re-export Port contracts');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod permissions;', 'rustok-core does not publish permission contracts');
requireNotContains('crates/rustok-core/src/lib.rs', 'pub mod locale;', 'rustok-core does not publish locale contracts');
requireContains('crates/rustok-core/Cargo.toml', 'rustok-api.workspace = true', 'rustok-core depends on rustok-api contracts');
if (exists('crates/rustok-core/src/ports.rs')) fail('rustok-core ports implementation must be deleted');
else pass('rustok-core ports implementation is absent');
for (const rel of ['crates/rustok-core/src/permissions.rs', 'crates/rustok-core/src/locale.rs']) {
  if (exists(rel)) fail(`${rel} must be deleted after API ownership cutover`);
  else pass(`${rel} is absent`);
}

const rustSources = [...walk('apps', (file) => file.endsWith('.rs')), ...walk('crates', (file) => file.endsWith('.rs'))];
for (const rel of rustSources) {
  const source = read(rel);
  if (/rustok_core::(?:permissions(?:::|\b)|Permission\b|Action\b|Resource\b)/.test(source)) {
    fail(`${rel} uses a removed rustok-core permission path`);
  }
  if (/rustok_core::(?:locale(?:::|\b)|build_locale_candidates\b|locale_tags_match\b|normalize_locale_tag\b|PLATFORM_FALLBACK_LOCALE\b)/.test(source)) {
    fail(`${rel} uses a removed rustok-core locale path`);
  }
  if (shouldForbidSystemAuthority(rel) && source.includes('SecurityContext::system()')) {
    const matches = [...source.matchAll(/SecurityContext::system\(\)/g)];
    for (const match of matches) {
      if (!isTestRel(rel) && !isInsideTestModule(source, match.index ?? 0)) {
        fail(`${rel} grants system authority outside trusted runtime/test code`);
        break;
      }
    }
  }
  if (!isTestRel(rel) && /[A-Za-z0-9_]+_or_system\b/.test(source)) {
    fail(`${rel} exposes an *_or_system authority helper`);
  }
  const ownsApiLocaleContract = rel === path.join('crates', 'rustok-api', 'src', 'locale.rs');
  const isUiI18nInternal = rel.startsWith(path.join('crates', 'rustok-ui-i18n', 'src'));
  if (!ownsApiLocaleContract && !isUiI18nInternal) {
    if (/(?:^|\n)\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+locale_tags_match\s*\(/.test(source)) {
      fail(`${rel} defines a package-local locale_tags_match helper`);
    }
    if (/(?:^|\n)\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+normalize_locale_tag\s*\(/.test(source)) {
      fail(`${rel} defines a package-local normalize_locale_tag helper`);
    }
  }
}
requireNotContains('crates/rustok-api/src/lib.rs', 'pub mod ui;', 'rustok-api does not own UI route/query/input contracts');
requireNotContains('crates/rustok-api/src/lib.rs', 'pub mod route_selection;', 'rustok-api does not own UI route selection contracts');
if (exists('crates/rustok-seo-admin-support/src/locale.rs')) fail('SEO admin support locale duplicate must be deleted');
else pass('SEO admin support locale duplicate is absent');
requireContains('crates/rustok-outbox/src/ports.rs', 'use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};', 'outbox consumes canonical rustok-api Port contracts');
requireContains('crates/rustok-outbox/Cargo.toml', 'rustok-api.workspace = true', 'outbox depends on the neutral API contract layer');
requireContains('crates/rustok-outbox/src/lib.rs', 'pub mod loco;', 'outbox owns its Loco composition adapter');
requireContains('DECISIONS/2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md', 'Status: Accepted', 'Port contract ownership ADR is accepted');

const entries = moduleEntries();
if (entries.length === 0) fail('modules.toml exposes module entries');
else pass(`modules.toml exposes ${entries.length} module entries`);

const optionalEntries = entries.filter((entry) => !entry.required);
if (optionalEntries.length === 0) fail('modules.toml exposes optional modules for generated API composition');
else pass(`modules.toml exposes ${optionalEntries.length} optional modules for generated API composition`);

for (const entry of entries) {
  const pkg = inspectPackageManifest(entry);
  if (!pkg) {
    fail(`${entry.slug}: missing package-local rustok-module.toml at ${entry.modulePath}`);
    continue;
  }
  if (pkg.slugMatches) pass(`${entry.slug}: package manifest slug matches modules.toml`);
  else fail(`${entry.slug}: package manifest slug does not match modules.toml`);

  if (pkg.hasEntryType) pass(`${entry.slug}: package manifest declares crate entry_type`);
  else fail(`${entry.slug}: package manifest missing [crate].entry_type`);

  if ((pkg.hasGraphql || pkg.hasHttp) && !tableContains('docs/modules/registry.md', entry.slug)) {
    fail(`${entry.slug}: publishes API transport but is absent from central module registry`);
  } else if (pkg.hasGraphql || pkg.hasHttp) {
    pass(`${entry.slug}: API transport declaration is represented in central registry`);
  }
}

console.log('API surface contract verification');
for (const message of passes) console.log(`✓ ${message}`);
if (failures.length > 0) {
  for (const message of failures) console.error(`✗ ${message}`);
  console.error(`\n${failures.length} API surface contract violation(s)`);
  process.exit(1);
}
console.log(`\nAll ${passes.length} API surface contract checks passed`);
