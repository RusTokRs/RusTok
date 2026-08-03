#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function fail(message) {
  throw new Error(`[verify-marketplace-registry-freshness] ${message}`);
}

function source(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

function requireMarkers(relativePath, markers) {
  const contents = source(relativePath);
  const missing = markers.filter((marker) => !contents.includes(marker));
  if (missing.length > 0) {
    fail(`${relativePath} is missing: ${missing.join(', ')}`);
  }
  return contents;
}

function forbidMarkers(relativePath, markers) {
  const contents = source(relativePath);
  const present = markers.filter((marker) => contents.includes(marker));
  if (present.length > 0) {
    fail(`${relativePath} contains forbidden markers: ${present.join(', ')}`);
  }
}

try {
  requireMarkers('crates/rustok-api/src/module_marketplace.rs', [
    'pub enum MarketplaceRegistryStatus',
    'pub struct MarketplaceRegistryFreshness',
    'pub registry_id: String',
    'pub last_success_unix_ms: Option<u64>',
    'pub consecutive_failures: u64',
  ]);
  forbidMarkers('crates/rustok-api/src/module_marketplace.rs', [
    'pub registry_url:',
    'pub endpoint:',
    'pub last_error:',
  ]);

  requireMarkers('crates/rustok-modules/src/marketplace.rs', [
    'fn registry_freshness(&self) -> Vec<MarketplaceRegistryFreshness>',
    'Local compiled-manifest composition is intentionally not represented',
  ]);
  requireMarkers('apps/server/src/services/marketplace_catalog_adapter.rs', [
    '.filter_map(map_registry_freshness)',
    'snapshot.provider == "local-manifest"',
    'MarketplaceProviderHealthStatus::Disabled => return None',
  ]);

  const graphqlQueries = requireMarkers('apps/server/src/graphql/queries.rs', [
    'async fn marketplace_registry_freshness(',
    'ensure_modules_manage_permission(ctx).await?',
    '.registry_freshness()',
  ]);
  if (graphqlQueries.includes('.provider_health()')) {
    fail('GraphQL must consume the owner catalog port, not server-local provider health');
  }
  requireMarkers('apps/server/src/graphql/types.rs', [
    'pub struct MarketplaceRegistryFreshness',
    'impl From<rustok_api::MarketplaceRegistryFreshness>',
  ]);

  requireMarkers('apps/admin/src/features/modules/transport/types.rs', [
    'MARKETPLACE_REGISTRY_FRESHNESS_QUERY',
    'marketplaceRegistryFreshness { registryId status lastSuccessUnixMs consecutiveFailures }',
  ]);
  requireMarkers('apps/admin/src/features/modules/transport/client.rs', [
    'pub async fn fetch_marketplace_registry_freshness(',
    'marketplace_registry_freshness_native()',
  ]);
  requireMarkers('apps/admin/src/features/modules/transport/native_server_adapter.rs', [
    'endpoint = "admin/marketplace-registry-freshness"',
    'Permission::MODULES_MANAGE',
    '.0.registry_freshness()',
  ]);
  requireMarkers('apps/admin/src/features/modules/components/modules_list.rs', [
    'Federated registry freshness',
    'format_registry_last_success',
    'registry.consecutive_failures',
  ]);

  requireMarkers('apps/next-admin/src/shared/api/modules.ts', [
    'export interface MarketplaceRegistryFreshness',
    'export async function listMarketplaceRegistryFreshness(',
    'marketplaceRegistryFreshness {',
  ]);
  requireMarkers('apps/next-admin/src/features/modules/components/modules-list.tsx', [
    'Federated registry freshness',
    'formatRegistryLastSuccess',
    'registry.consecutiveFailures',
  ]);

  for (const relativePath of [
    'apps/server/src/graphql/queries.rs',
    'apps/server/src/services/marketplace_catalog_adapter.rs',
    'apps/admin/src/features/modules/transport/types.rs',
    'apps/admin/src/features/modules/transport/client.rs',
    'apps/admin/src/features/modules/transport/native_server_adapter.rs',
  ]) {
    forbidMarkers(relativePath, ['#[allow(dead_code)]', '#[allow(unused_imports)]']);
  }

  console.log(
    '[verify-marketplace-registry-freshness] owner projection, operator transports, and admin parity verified',
  );
} catch (error) {
  if (
    error instanceof Error &&
    error.message.startsWith('[verify-marketplace-registry-freshness]')
  ) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
