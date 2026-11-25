#!/usr/bin/env node

/**
 * Verification script for CC-06: API Surfaces, Transports & Error Hygiene
 *
 * Verifies:
 * 1. CC-06.01: Error envelope consistency (Axum RFC 7807 HttpError/ErrorBody sanitization, GraphQL errors)
 * 2. CC-06.02: DTO vs SeaORM Entity Boundary (controllers and resolvers do not expose raw SeaORM models)
 * 3. CC-06.03: GraphQL DataLoader & N+1 Elimination (request-scoped, tenant/locale isolated batch loaders)
 * 4. CC-06.04: List Pagination & Limit Clamping (strict bounds clamping on limit/per_page)
 * 5. CC-06.05: MCP Tool Schema Validation (schemars JSON Schema derivation & strong typing)
 */

import { readFileSync, readdirSync } from 'node:fs';
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
const forbidText = (content, text, label) => {
  if (content.includes(text)) {
    failures.push(`${label}: contains forbidden text "${text}"`);
  }
};

console.log('Verifying CC-06: API Surfaces, Transports & Error Hygiene...');

// ── 1. CC-06.01: Error Envelope Consistency ─────────────────────────────────
const serverError = read('apps/server/src/error.rs');
requireText(serverError, 'StatusCode::INTERNAL_SERVER_ERROR,', 'apps/server/src/error.rs');
requireText(serverError, '"internal_error",', 'apps/server/src/error.rs');
requireText(serverError, '"Internal server error",', 'apps/server/src/error.rs');
requireText(serverError, 'Self::Database(_error) => {', 'apps/server/src/error.rs (database error sanitization)');
requireText(serverError, 'error_code = "server_database_operation_failed"', 'apps/server/src/error.rs (database error log)');

const rustokWeb = read('crates/libs/rustok-web/src/lib.rs');
requireText(rustokWeb, 'pub struct ErrorBody {', 'crates/libs/rustok-web (canonical ErrorBody)');
requireText(rustokWeb, 'pub struct HttpError {', 'crates/libs/rustok-web (canonical HttpError)');
requireText(rustokWeb, 'pub fn port_error_to_http_error(error: PortError) -> HttpError', 'crates/libs/rustok-web (PortError to HttpError)');
requireText(rustokWeb, '"The requested service is temporarily unavailable"', 'crates/libs/rustok-web (redacted service unavailable)');
requireText(rustokWeb, '"The requested operation could not be completed"', 'crates/libs/rustok-web (redacted invariant violation)');

const graphqlErrors = read('crates/libs/rustok-api/src/graphql/errors.rs');
requireText(graphqlErrors, 'pub enum ErrorCode {', 'crates/libs/rustok-api (GraphQL ErrorCode enum)');
requireText(graphqlErrors, 'Unauthenticated,', 'crates/libs/rustok-api (UNAUTHENTICATED)');
requireText(graphqlErrors, 'PermissionDenied,', 'crates/libs/rustok-api (PERMISSION_DENIED)');
requireText(graphqlErrors, 'InternalError,', 'crates/libs/rustok-api (INTERNAL_ERROR)');
requireText(graphqlErrors, 'BadUserInput,', 'crates/libs/rustok-api (BAD_USER_INPUT)');
requireText(graphqlErrors, 'NotFound,', 'crates/libs/rustok-api (NOT_FOUND)');
requireText(graphqlErrors, 'Conflict,', 'crates/libs/rustok-api (CONFLICT)');

// ── 2. CC-06.02: DTO vs SeaORM Entity Boundary ──────────────────────────────
// Check controllers for public endpoints leaking raw SeaORM models
const controllersDir = path.join(root, 'apps/server/src/controllers');
const controllerFiles = readdirSync(controllersDir).filter((f) => f.endsWith('.rs') && f !== 'mod.rs');
for (const file of controllerFiles) {
  const content = read(`apps/server/src/controllers/${file}`);
  const nonTestContent = content.split('#[cfg(test)]')[0];
  const forbiddenPatterns = [
    /pub async fn [a-zA-Z0-9_]+\s*\([^)]*\)\s*->\s*(?:Result<)?Json<[a-z0-9_]+::Model>/,
    /pub async fn [a-zA-Z0-9_]+\s*\([^)]*\)\s*->\s*(?:Result<)?Json<Vec<[a-z0-9_]+::Model>>/,
    /pub async fn [a-zA-Z0-9_]+\s*\([^)]*\)\s*->\s*(?:Result<)?Json<[a-z0-9_]+::ActiveModel>/,
  ];
  for (const pattern of forbiddenPatterns) {
    if (pattern.test(nonTestContent)) {
      failures.push(`apps/server/src/controllers/${file}: handler leaks raw SeaORM entity across API boundary (${pattern})`);
    }
  }
}

// ── 3. CC-06.03: GraphQL DataLoader & N+1 Elimination ───────────────────────
const tenantNameLoader = read('apps/server/src/graphql/loaders.rs');
requireText(tenantNameLoader, 'impl Loader<Uuid> for TenantNameLoader', 'apps/server TenantNameLoader');
requireText(tenantNameLoader, 'tenants::Column::Id.is_in(keys)', 'TenantNameLoader batched WHERE IN query');
requireText(tenantNameLoader, 'fn tenant_name_loader_error(', 'TenantNameLoader redacted error helper');
requireText(tenantNameLoader, '"Tenant name lookup failed"', 'TenantNameLoader safe message');

const profileLoader = read('crates/modules/rustok-profiles/src/loader.rs');
requireText(profileLoader, 'pub struct ProfileSummaryLoaderKey {', 'rustok-profiles ProfileSummaryLoaderKey');
requireText(profileLoader, 'pub tenant_id: Uuid,', 'ProfileSummaryLoaderKey tenant_id isolation');
requireText(profileLoader, 'pub requested_locale: Option<String>,', 'ProfileSummaryLoaderKey requested_locale');
requireText(profileLoader, 'pub tenant_default_locale: Option<String>,', 'ProfileSummaryLoaderKey tenant_default_locale');
requireText(profileLoader, 'struct ProfileSummaryBatchKey {', 'ProfileSummaryLoader batch key');
requireText(profileLoader, 'impl Loader<ProfileSummaryLoaderKey> for ProfileSummaryLoader', 'ProfileSummaryLoader Loader implementation');

const audiencePolicy = read('apps/server/src/graphql/profile_summary_policy.rs');
requireText(audiencePolicy, 'impl Extension for ProfileSummaryAudiencePolicyExtension', 'apps/server ProfileSummaryAudiencePolicyExtension');
requireText(audiencePolicy, 'ProfileSummaryLoader::for_audience(db, audience)', 'request-scoped ProfileSummaryLoader');

// ── 4. CC-06.04: List Pagination & Limit Clamping ───────────────────────────
const adminEvents = read('apps/server/src/controllers/admin_events.rs');
requireText(adminEvents, 'let limit = query.limit.clamp(1, 200);', 'admin_events.rs limit clamping');

const mcpManagement = read('crates/modules/rustok-mcp/src/persistence/service.rs');
requireText(mcpManagement, 'fn clamp_limit(limit: Option<u64>, default: u64, max: u64) -> u64', 'MCP persistence service clamp_limit');
requireText(mcpManagement, 'limit.unwrap_or(default).clamp(1, max)', 'MCP persistence service clamp expression');

// ── 5. CC-06.05: MCP Tool Schema Validation ─────────────────────────────────
const mcpTools = read('crates/modules/rustok-mcp/src/tools.rs');
requireText(mcpTools, 'use schemars::JsonSchema;', 'rustok-mcp JsonSchema import');
requireText(mcpTools, 'pub struct ModuleQueryRequest {', 'rustok-mcp ModuleQueryRequest');
requireText(mcpTools, 'pub struct ModuleLookupRequest {', 'rustok-mcp ModuleLookupRequest');
requireText(mcpTools, 'RegistryToolInvocationError::InvalidArguments', 'rustok-mcp InvalidArguments error');

const mcpServer = read('crates/modules/rustok-mcp/src/server.rs');
requireText(mcpServer, 'use schemars::schema_for;', 'rustok-mcp server schema_for import');
requireText(mcpServer, 'let req: ModuleQueryRequest =', 'rustok-mcp server typed request deserialization');
requireText(mcpServer, 'rmcp::ErrorData::invalid_params(', 'rustok-mcp server invalid_params rejection');

const mcpController = read('apps/server/src/controllers/mcp.rs');
requireText(mcpController, 'fn parse_tool_args<T: serde::de::DeserializeOwned>(', 'apps/server/mcp.rs parse_tool_args');
requireText(mcpController, 'serde_json::from_value(arguments.unwrap_or_else(|| serde_json::json!({}))).map_err(Into::into)', 'apps/server/mcp.rs typed parse');

if (failures.length > 0) {
  console.error('\n❌ CC-06 Verification FAILED:');
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log('✔ All CC-06 architectural invariants verified successfully.');
