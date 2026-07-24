#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const authorization = read('crates/rustok-seo/src/authorization.rs');
const applications = read('crates/rustok-seo/src/services/applications.rs');
const lifecycle = read('apps/server/src/services/app_lifecycle.rs');
const controllers = read('crates/rustok-seo/src/controllers/mod.rs');
const graphql = read('crates/rustok-seo/src/graphql/mod.rs');
const nativeAdmin = read(
  'crates/rustok-seo/admin/src/transport/native_server_adapter.rs',
);

for (const [value, label] of [
  ['pub struct SeoWorkerAuthorization', 'opaque worker grant'],
  ['_private: ()', 'private worker grant field'],
  ['pub fn from_runtime_config(', 'config-backed grant factory'],
  ['if !host_runs_background_workers || !seo_worker_enabled', 'fail-closed runtime authorization'],
  ['return Err(SeoError::PermissionDenied);', 'worker authorization rejection'],
]) {
  requireText(authorization, value, label);
}

for (const [signature, label] of [
  ['pub async fn execute_next_sitemap_job(', 'sitemap worker facade'],
  ['pub async fn execute_next_bulk_job(', 'bulk worker facade'],
  ['pub async fn execute_next_index_repair_replay_job(', 'index worker facade'],
]) {
  const block = sliceBetween(applications, signature, '\n    }', label);
  requireText(block, '_authorization: &SeoWorkerAuthorization', `${label} grant`);
}

for (const [value, label] of [
  ['SeoWorkerAuthorization::from_runtime_config(', 'host grant construction'],
  ['settings.runtime.runs_background_workers()', 'host worker-mode authorization'],
  ['seo_bulk_worker_enabled', 'SEO worker-switch authorization'],
  ['authorization: SeoWorkerAuthorization', 'grant ownership in worker loop'],
  ['.execute_next_bulk_job(&authorization)', 'authorized worker execution'],
]) {
  requireText(lifecycle, value, label);
}
forbidText(
  lifecycle,
  'service.bulk().execute_next_bulk_job().await',
  'unauthorized worker call',
);

const sitemapIndex = sliceBetween(
  controllers,
  'pub async fn sitemap_index(',
  '\npub async fn sitemap_file(',
  'public sitemap index',
);
requireText(
  sitemapIndex,
  'SeoHttpError::not_found("SEO sitemap index not found")',
  'missing sitemap fail closed',
);
forbidText(sitemapIndex, '.generate_sitemaps(', 'public GET sitemap generation');

const restIndexRepair = sliceBetween(
  controllers,
  'pub async fn index_repair_replay_json(',
  '\npub async fn bulk_artifact_download(',
  'REST index operator',
);
requireText(restIndexRepair, 'auth: AuthContext', 'REST authenticated operator');
requireText(restIndexRepair, 'Permission::SEO_MANAGE', 'REST index manage permission');

const graphqlSitemap = sliceBetween(
  graphql,
  'async fn generate_seo_sitemaps(',
  '\n    async fn queue_seo_bulk_apply(',
  'GraphQL sitemap operator',
);
requireText(graphqlSitemap, 'Permission::SEO_GENERATE', 'GraphQL sitemap generate permission');
const graphqlIndex = sliceBetween(
  graphql,
  'async fn run_seo_index_repair_replay(',
  '\n}',
  'GraphQL index operator',
);
requireText(graphqlIndex, 'Permission::SEO_MANAGE', 'GraphQL index manage permission');
for (const forbidden of [
  'execute_next_bulk_job',
  'execute_next_sitemap_job',
  'execute_next_index_repair_replay_job',
]) {
  forbidText(graphql, forbidden, 'GraphQL worker execution');
}

const nativeSitemap = sliceBetween(
  nativeAdmin,
  'pub(super) async fn seo_generate_sitemaps_native(',
  '\n#[server(prefix = "/api/fn", endpoint = "seo/settings")]',
  'native sitemap operator',
);
requireText(nativeSitemap, 'Permission::SEO_GENERATE', 'native sitemap generate permission');
const nativeIndex = sliceBetween(
  nativeAdmin,
  'pub(super) async fn seo_index_repair_replay_native(',
  '\n#[cfg(all(test, feature = "ssr"))]',
  'native index operator',
);
requireText(nativeIndex, 'Permission::SEO_MANAGE', 'native index manage permission');

if (failures.length > 0) {
  console.error('SEO entrypoint authorization verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ SEO worker execution requires an explicit runtime grant and external operator entry points remain RBAC-gated',
);
