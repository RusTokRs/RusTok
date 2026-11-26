#!/usr/bin/env node

/**
 * Verification script for CC-07: Multi-Tenancy & Effective Context Isolation
 *
 * Verifies:
 * 1. CC-07.01: Universal Query Filter Invariant (tenant_id query filtering across domain entities)
 * 2. CC-07.02: Request Host Smuggling & Boundary Defense (duplicate host check, trusted proxy extraction, subdomain guards)
 * 3. CC-07.03: Cross-Tenant Relation Verification (foreign key and parent relations validate matching tenant_id)
 * 4. CC-07.04: Context Propagation (middleware -> request extensions -> GraphQL context & WebSocket init -> outbox events)
 */

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8');

const failures = [];
const requireText = (content, text, label) => {
  if (!content.includes(text)) {
    failures.push(`${label}: missing expected text "${text}"`);
  }
};
const requirePattern = (content, pattern, label) => {
  if (!pattern.test(content)) {
    failures.push(`${label}: missing pattern ${pattern}`);
  }
};
const forbidText = (content, text, label) => {
  if (content.includes(text)) {
    failures.push(`${label}: contains forbidden text "${text}"`);
  }
};

console.log('Verifying CC-07: Multi-Tenancy & Effective Context Isolation...');

// ── 1. CC-07.01: Universal Query Filter Invariant ───────────────────────────
const alloyStorage = read('crates/modules/alloy/src/storage/sea_orm/mod.rs');
requireText(alloyStorage, 'fn scoped_by_id(&self, id: ScriptId) -> sea_orm::Select<Entity>', 'alloy storage scoped_by_id');
requireText(alloyStorage, 'select.filter(Column::TenantId.eq(tenant_id))', 'alloy storage tenant_id filter');
requireText(alloyStorage, 'fn ensure_script_scope(&self, script: &Script) -> ScriptResult<()>', 'alloy storage ensure_script_scope');
requireText(alloyStorage, 'if self.tenant_id.is_some_and(|t| script.tenant_id != t)', 'alloy cross-tenant access check');

const flexAttached = read('crates/modules/flex/src/attached_definitions.rs');
requireText(flexAttached, '.filter(Column::TenantId.eq(tenant_id))', 'flex attached definitions tenant_id filter');

const commentsService = read('crates/modules/rustok-comments/src/services.rs');
requireText(commentsService, 'pub async fn create_comment_in_tx(', 'comments create_comment_in_tx');
requireText(commentsService, 'if tenant_id.is_nil() {', 'comments reject nil tenant_id');
requireText(commentsService, 'tenant_id: Set(tenant_id),', 'comments insert tenant_id');

// ── 2. CC-07.02: Request Host Smuggling & Boundary Defense ──────────────────
const tenantResolution = read('apps/server/src/middleware/tenant_resolution.rs');
requirePattern(tenantResolution, /\.get_all\(axum::http::header::HOST\)[\s\S]*?\.nth\(1\)[\s\S]*?\.is_some\(\)/, 'reject duplicate Host header');
requireText(tenantResolution, 'TenantResolutionError::ConflictingHeaderValues', 'conflicting host header error');
requireText(tenantResolution, 'peer_ip_from_extensions(req.extensions())', 'peer ip extraction for request trust');
requireText(tenantResolution, 'extract_effective_host(req.headers(), peer_ip, &settings.runtime.request_trust)', 'trusted proxy effective host extraction');
requireText(tenantResolution, 'TenantIdentifierValidator::validate_host(canonical_host)', 'canonical host validation');
requireText(tenantResolution, 'pub(crate) fn subdomain_identifier(', 'subdomain_identifier function');
requireText(tenantResolution, 'TenantResolutionError::BaseDomainRequiresTenantSlug', 'base domain requires slug error');
requireText(tenantResolution, 'TenantResolutionError::NestedSubdomain', 'nested subdomain rejected');
requireText(tenantResolution, 'TenantResolutionError::NoBaseDomainMatch', 'no base domain match error');
requireText(tenantResolution, 'pub(crate) fn validate_resolved_slug(', 'validate_resolved_slug function');
requireText(tenantResolution, 'TenantResolutionError::ConflictingTenantAssertions', 'conflicting tenant assertions rejected');

// ── 3. CC-07.03: Cross-Tenant Relation Verification ─────────────────────────
const channelResolution = read('crates/modules/rustok-channel/src/resolution.rs');
requireText(channelResolution, 'pub struct RequestFacts {', 'rustok-channel RequestFacts');
requireText(channelResolution, 'pub tenant_id: Uuid,', 'RequestFacts mandatory tenant_id');
requireText(channelResolution, 'if facts.tenant_id.is_nil() {', 'channel resolution rejects nil tenant');
requirePattern(channelResolution, /Ok\(detail\) if detail\.channel\.tenant_id != facts\.tenant_id => \{/, 'channel header ID cross-tenant rejection');
requireText(channelResolution, 'does not belong to tenant', 'channel header ID cross-tenant rejection trace');
requirePattern(channelResolution, /\.get_channel_detail_by_slug\(facts\.tenant_id,\s*slug\)/, 'channel slug lookup tenant scoped');

requireText(commentsService, 'if let Some(parent_comment_id) = input.parent_comment_id {', 'parent comment check');
requirePattern(commentsService, /\.find_comment_in_tx\(txn,\s*tenant_id,\s*parent_comment_id,\s*true\)/, 'parent comment lookup tenant scoped');
requireText(commentsService, 'if parent.thread_id != thread.id {', 'parent comment same thread check');

// ── 4. CC-07.04: Context Propagation ────────────────────────────────────────
const tenantMiddleware = read('apps/server/src/middleware/tenant.rs');
requireText(tenantMiddleware, 'req.extensions_mut().insert(TenantContextExtension(context));', 'tenant context in request extensions');

const tenantExtractor = read('apps/server/src/extractors/tenant.rs');
requireText(tenantExtractor, 'pub struct CurrentTenant(pub TenantContext);', 'CurrentTenant extractor definition');
requirePattern(tenantExtractor, /parts[\s\S]*?\.tenant_context\(\)[\s\S]*?\.ok_or\(StatusCode::NOT_FOUND\)/, 'parts tenant_context extraction');

const graphqlController = read('apps/server/src/controllers/graphql.rs');
requireText(graphqlController, 'tenant_ctx: TenantContext,', 'GraphQL HTTP TenantContext extractor');
requirePattern(graphqlController, /\.data\(tenant_ctx\)/, 'GraphQL HTTP TenantContext data injection');
requireText(graphqlController, 'tenant::resolve_tenant_context_by_slug(&runtime_ctx, &tenant_slug)', 'GraphQL WebSocket tenant resolution by slug');
requireText(graphqlController, 'ensure_ws_channel_tenant(channel.as_ref(), tenant_ctx.id)?;', 'GraphQL WebSocket channel tenant validation');

const eventContract = read('crates/libs/rustok-events/src/contract.rs');
requireText(eventContract, 'pub fn tenant_id(&self) -> Uuid {', 'event envelope tenant_id getter');
requireText(eventContract, 'EventValidationError::NilUuid("tenant_id")', 'reject event with nil tenant_id');

if (failures.length > 0) {
  console.error('\n❌ CC-07 Verification FAILED:');
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log('✔ All CC-07 architectural invariants verified successfully.');
