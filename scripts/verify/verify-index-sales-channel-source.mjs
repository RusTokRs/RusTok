#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const resolve = (relative) => path.join(root, relative);
const read = (relative) => fs.readFileSync(resolve(relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-sales-channel-source] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const channelCargo = read('crates/rustok-channel/Cargo.toml');
forbidMarkers('crates/rustok-channel/Cargo.toml', channelCargo, [
  'rustok-index',
  'register_index_schema_source',
]);

const channelRootPath = 'crates/rustok-channel/src/lib.rs';
const channelRoot = requireMarkers(channelRootPath, [
  'pub struct ChannelRuntimeSelected;',
  'fn register_runtime_extensions(',
  'extensions.insert(ChannelRuntimeSelected);',
]);
forbidMarkers(channelRootPath, channelRoot, [
  'rustok_index',
  'register_index_schema_source',
  'PostgresIndexSourceFactory',
]);
requireMarkers('crates/rustok-channel/tests/index_selection.rs', [
  'channel_module_publishes_only_a_typed_selection_marker_for_index_bridges',
  'assert!(extensions.contains::<ChannelRuntimeSelected>());',
  'assert!(!cargo.contains("rustok-index"));',
]);

const migrationPath =
  'crates/rustok-channel/src/migrations/m20260730_000010_add_channel_index_revision.rs';
const migration = requireMarkers(migrationPath, [
  'ALTER TABLE channels',
  'ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1',
  'chk_channels_index_revision_positive',
  'OLD.index_revision = 9223372036854775807',
  'NEW.index_revision := OLD.index_revision + 1;',
  'trg_channels_bump_index_revision',
  'BEFORE UPDATE ON channels',
]);
forbidMarkers(migrationPath, migration, [
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
]);
requireMarkers('crates/rustok-channel/src/migrations/mod.rs', [
  'mod m20260730_000010_add_channel_index_revision;',
  'Box::new(m20260730_000010_add_channel_index_revision::Migration)',
]);
requireMarkers('crates/rustok-channel/src/migrations/m20260325_000001_create_channels.rs', [
  '.name("idx_channels_tenant_slug")',
  '.col(Channels::TenantId)',
  '.col(Channels::Slug)',
  '.unique()',
]);

const distributionRootPath = 'crates/rustok-distribution/src/lib.rs';
const distributionRoot = requireMarkers(distributionRootPath, [
  'mod channel_index;',
  'register_selected_index_bridges(&mut extensions)?;',
  'channel_index::register(extensions)?;',
  'materialize_index_schema_sources(&mut extensions)?;',
]);
const bridgeCall = distributionRoot.indexOf('channel_index::register(extensions)?;');
const schemaMaterialization = distributionRoot.indexOf(
  'materialize_index_schema_sources(&mut extensions)?;',
);
if (bridgeCall < 0 || schemaMaterialization <= bridgeCall) {
  fail('SalesChannel bridge must register before immutable schema materialization');
}
if (distributionRoot.includes('#[cfg(feature = "mod-product")]\nmod channel_index;')) {
  fail('SalesChannel bridge must not be gated by the Product feature');
}

const sourcePath = 'crates/rustok-distribution/src/channel_index.rs';
const source = requireMarkers(sourcePath, [
  'SALES_CHANNEL_INDEX_SOURCE: &str = "sales-channel-postgres-primary"',
  'SALES_CHANNEL_EVENT_DOMAIN: &str = "rustok-channel.sales-channel-replay-v1"',
  'extensions.contains::<rustok_channel::ChannelRuntimeSelected>()',
  'register_index_schema_source(extensions, "channel", schema)',
  'register_postgres_index_source_factory(',
  'entity: EntityName::new("sales_channel")?',
  'locale_mode: LocaleMode::None',
  'field("slug", IndexValueType::String, true, true)?',
  'field("is_active", IndexValueType::Boolean, true, true)?',
  'impl PostgresIndexSourceFactory for SalesChannelPostgresIndexSourceFactory',
  'impl IndexSource for SalesChannelPostgresIndexSource',
  'FROM channels c',
  'c.index_revision,',
  'c.id > $2',
  'ORDER BY c.id ASC',
  'request.limit() + 1',
  'WITH requested(channel_id) AS (VALUES {})',
  'JOIN requested r ON r.channel_id = c.id',
  'sales_channel_index_locale_forbidden',
  'derive_index_source_event_id(',
  'locale: None',
  'links: Vec::new()',
  '#[serde(deny_unknown_fields)]',
  'selected_sales_channel_schema_is_nonlocalized_and_link_free',
  'selected_sales_channel_cursor_rejects_nil_and_unknown_fields',
  'selected_sales_channel_bridge_skips_partial_registry_without_channel_module',
  'selected_sales_channel_bridge_registers_schema_and_factory',
]);
forbidMarkers(sourcePath, source, [
  'c.settings',
  'IndexLink',
  'IndexLinkValue',
  'LocaleKey',
  'ORDER BY c.index_revision',
  '(c.index_revision, c.id)',
  'SELECT *',
  'index_entities',
  'index_links',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tokio::time::sleep',
  'loop {',
  'rustok_product',
  'rustok_search',
]);

requireMarkers('crates/rustok-distribution/tests/channel_index.rs', [
  'selected_channel_bridge_publishes_schema_and_source_factory',
  'EntityName::new("sales_channel")',
  'factory.owner_module() == "channel"',
  'factory.factory_name() == "sales-channel-postgres-primary"',
]);

requireMarkers('crates/rustok-index/docs/m7-sales-channel-source.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`rustok-channel::sales_channel@1`',
  'stable `channel_id` UUID order',
  '`index_revision` is the mutation `source_version`; it is not the scan cursor.',
  'The schema is link-free.',
  'Channel hard-delete tombstones',
  'Runtime capability presence does not establish persisted schema readiness.',
  'maintainer-run',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-sales-channel-source.mjs'",
]);

console.log('[verify-index-sales-channel-source] OK');
