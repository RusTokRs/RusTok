#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-topic-route-identity-owner.json')
);
const service = read('crates/rustok-forum/src/services/topic_route.rs');
const migration = read(
  'crates/rustok-forum/src/migrations/m20260805_000024_add_forum_topic_route_aliases.rs'
);
const migrationRegistry = read('crates/rustok-forum/src/migrations/mod.rs');
const serviceRegistry = read('crates/rustok-forum/src/services/mod.rs');
const exports = read('crates/rustok-forum/src/lib.rs');
const errors = read('crates/rustok-forum/src/error.rs');
const controllers = read('crates/rustok-forum/src/controllers/mod.rs');
const docs = read(
  'crates/rustok-forum/docs/forum-24a-topic-route-identity-owner.md'
);
const plan = read('crates/rustok-forum/docs/implementation-plan.md');

assert.equal(contract.contract, 'forum_topic_route_identity_owner_v1');
assert.equal(contract.task, 'FORUM-24A');
assert.equal(contract.route.canonical_shape, '/{locale}/forum/t/{short_id}/{slug}');
assert.equal(contract.route.short_id_length, 12);
assert.equal(contract.route.slug_is_identity, false);
assert.equal(contract.resolution.canonical_merge_resolution_reused, true);
assert.equal(contract.resolution.short_id_collision_fails_closed, true);
assert.equal(contract.aliases.append_only, true);
assert.equal(contract.aliases.target_slug_is_recomputed, true);
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.rest_route_changed, false);
assert.equal(contract.compatibility.storefront_route_mounted, false);

for (const marker of [
  'pub const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12',
  'pub enum ForumTopicRouteDisposition',
  'pub struct ForumTopicRouteDescriptor',
  'pub struct ForumTopicRouteResolution',
  'pub struct ForumTopicRouteService',
  'ForumTopicCanonicalResolutionService::new',
  'LIMIT 2',
  'TopicRouteResolutionConflict',
  'record_redirect_alias_in_tx',
  'record_gone_alias_in_tx',
  'ON CONFLICT (tenant_id, locale, short_id, slug) DO NOTHING',
  'short_identity_is_stable_and_lowercase'
]) {
  assert.ok(service.includes(marker), `missing service marker: ${marker}`);
}

for (const marker of [
  'CREATE TABLE IF NOT EXISTS forum_topic_route_aliases',
  'UNIQUE (tenant_id, locale, short_id, slug)',
  "disposition = 'redirect'",
  "disposition = 'gone'",
  'forum topic route aliases are append-only',
  'forum_topic_route_alias_update',
  'forum_topic_route_alias_delete'
]) {
  assert.ok(migration.includes(marker), `missing migration marker: ${marker}`);
}

for (const forbidden of [
  'CREATE TABLE IF NOT EXISTS forum_topic_route_registry',
  'UPDATE forum_topic_route_aliases',
  'DELETE FROM forum_topic_route_aliases'
]) {
  assert.ok(!migration.includes(forbidden), `forbidden migration marker: ${forbidden}`);
}

assert.ok(
  migrationRegistry.includes(
    'mod m20260805_000024_add_forum_topic_route_aliases;'
  )
);
assert.ok(
  migrationRegistry.includes(
    'm20260805_000024_add_forum_topic_route_aliases::Migration'
  )
);
assert.ok(serviceRegistry.includes('mod topic_route;'));
assert.ok(serviceRegistry.includes('ForumTopicRouteService'));
assert.ok(exports.includes('ForumTopicRouteService'));
assert.ok(errors.includes('TopicRouteNotFound'));
assert.ok(errors.includes('FORUM_TOPIC_ROUTE_NOT_FOUND'));
assert.ok(errors.includes('TopicRouteResolutionConflict'));
assert.ok(errors.includes('FORUM_TOPIC_ROUTE_RESOLUTION_CONFLICT'));
assert.ok(controllers.includes('ForumError::TopicRouteNotFound'));
assert.ok(controllers.includes('ForumError::TopicRouteResolutionConflict'));
assert.ok(docs.includes('FORUM-24A'));
assert.ok(docs.includes('/{locale}/forum/t/{short_id}/{slug}'));
assert.ok(plan.includes('FORUM-24A'));

console.log('forum topic route identity owner source contract verified');
