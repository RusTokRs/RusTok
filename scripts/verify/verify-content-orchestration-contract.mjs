#!/usr/bin/env node
import { readFileSync } from 'node:fs';

const checks = [];
const read = (path) => readFileSync(path, 'utf8');
const service = read('crates/rustok-content/src/services/content_orchestration_service.rs');
const resolver = read('crates/rustok-content/src/services/canonical_url_service.rs');
const plan = read('crates/rustok-content/docs/implementation-plan.md');
const docs = read('crates/rustok-content/docs/README.md');
const runbook = read('crates/rustok-content/docs/runbook.md');
const registry = read('docs/modules/registry.md');
const pkg = read('package.json');

function check(name, ok, hint) {
  checks.push({ name, ok, hint });
}

function includesAll(text, markers) {
  return markers.every((marker) => text.includes(marker));
}

const operations = [
  {
    name: 'promote_topic_to_post',
    event: 'DomainEvent::TopicPromotedToPost',
    scopes: ['Resource::ForumTopics, Action::Moderate', 'Resource::BlogPosts, Action::Create'],
  },
  {
    name: 'demote_post_to_topic',
    event: 'DomainEvent::PostDemotedToTopic',
    scopes: ['Resource::BlogPosts, Action::Moderate', 'Resource::ForumTopics, Action::Create'],
  },
  {
    name: 'split_topic',
    event: 'DomainEvent::TopicSplit',
    scopes: ['Resource::ForumTopics, Action::Moderate'],
  },
  {
    name: 'merge_topics',
    event: 'DomainEvent::TopicsMerged',
    scopes: ['Resource::ForumTopics, Action::Moderate'],
  },
];

for (const op of operations) {
  const start = service.indexOf(`pub async fn ${op.name}`);
  const end = operations
    .map((candidate) => service.indexOf(`pub async fn ${candidate.name}`, start + 1))
    .filter((idx) => idx > start)
    .sort((a, b) => a - b)[0] ?? service.indexOf('    fn ensure_scope', start);
  const body = start >= 0 && end > start ? service.slice(start, end) : '';

  check(`${op.name}: public service method exists`, start >= 0, `missing pub async fn ${op.name}`);
  check(
    `${op.name}: bridge trait method exists`,
    service.includes(`async fn ${op.name}(`),
    `ContentOrchestrationBridge must expose ${op.name}`,
  );
  check(
    `${op.name}: idempotency is checked before bridge execution`,
    body.includes('ensure_idempotency_key') && body.indexOf('fetch_idempotent_result') < body.indexOf(`.${op.name}(`),
    `${op.name} must check/replay idempotency before invoking bridge`,
  );
  check(
    `${op.name}: required RBAC scopes are enforced`,
    op.scopes.every((scope) => body.includes(scope)),
    `${op.name} is missing one or more RBAC scope checks`,
  );
  check(
    `${op.name}: canonical URL mutations are transactional`,
    body.includes('apply_canonical_url_mutations') && body.indexOf('apply_canonical_url_mutations') > body.indexOf(`.${op.name}(`),
    `${op.name} must apply bridge URL updates inside the same transaction`,
  );
  check(
    `${op.name}: outbox event is emitted`,
    body.includes('publish_in_tx') && body.includes(op.event),
    `${op.name} must emit ${op.event}`,
  );
  check(
    `${op.name}: audit/idempotency record is persisted`,
    body.includes('persist_orchestration_record') && body.includes(`operation: "${op.name}"`),
    `${op.name} must persist orchestration_operation + audit_log`,
  );
}

check(
  'canonical mutation helper normalizes route, locale and target kind',
  includesAll(service, ['normalize_target_kind', 'normalize_route_url', 'normalize_locale_code']),
  'canonical URL updates must normalize target kind, route and locale',
);
check(
  'canonical mutation helper preserves retired URLs as aliases',
  includesAll(service, ['retired_targets', 'delete_by_id(retired_canonical.id)', 'alias_urls.insert(retired_canonical.canonical_url.clone())']),
  'retired canonical targets must be atomically retired and redirected',
);
check(
  'canonical mutation helper publishes URL outbox events',
  includesAll(service, ['DomainEvent::CanonicalUrlChanged', 'DomainEvent::UrlAliasPurged']),
  'canonical URL changes must emit both URL events when aliases are present',
);
check(
  'route resolver keeps alias-first redirect semantics',
  resolver.indexOf('url_alias::Entity::find()') >= 0 && resolver.indexOf('url_alias::Entity::find()') < resolver.indexOf('canonical_url::Entity::find()') && resolver.includes('redirect_required: true'),
  'CanonicalUrlService must resolve aliases before canonical routes',
);
check(
  'route resolver uses shared locale fallback',
  resolver.includes('normalize_locale_code') && resolver.includes('resolve_by_locale'),
  'CanonicalUrlService must use shared locale normalization/fallback helpers',
);
check(
  'local docs mention the compile-free content verifier',
  plan.includes('npm run verify:content:orchestration') && docs.includes('npm run verify:content:orchestration') && runbook.includes('npm run verify:content:orchestration'),
  'implementation plan, docs README and runbook must mention npm run verify:content:orchestration',
);
check(
  'central registry points content to orchestration guardrail',
  registry.includes('npm run verify:content:orchestration'),
  'docs/modules/registry.md content row must mention the guardrail',
);
check(
  'package.json exposes the content verifier',
  pkg.includes('"verify:content:orchestration": "node scripts/verify/verify-content-orchestration-contract.mjs"'),
  'package.json must define verify:content:orchestration',
);

const failed = checks.filter((item) => !item.ok);
if (failed.length > 0) {
  console.error('content orchestration contract verification failed:');
  for (const item of failed) {
    console.error(`- ${item.name}: ${item.hint}`);
  }
  process.exit(1);
}

console.log(`content orchestration contract verification passed (${checks.length} checks)`);
