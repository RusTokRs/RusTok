#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const authoritySource = read('crates/rustok-api/src/context/host_authority.rs');
const apiContextSource = read('crates/rustok-api/src/context/mod.rs');
const apiLibSource = read('crates/rustok-api/src/lib.rs');
const serverLibSource = read('apps/server/src/lib.rs');
const credentialSource = read('apps/server/src/host_authority.rs');
const tenantMiddlewareSource = read('apps/server/src/middleware/auth_context.rs');
const graphqlControllerSource = read('apps/server/src/controllers/graphql.rs');
const eventsNativeSource = read(
  'crates/rustok-events-module/admin/src/transport/native_server_adapter.rs',
);
const iggyNativeSource = read(
  'crates/rustok-iggy-connector/admin/src/transport/native_server_adapter.rs',
);
const systemSource = read('apps/server/src/graphql/system.rs');
const settingsModuleSource = read('apps/server/src/graphql/settings/mod.rs');
const settingsQuerySource = read('apps/server/src/graphql/settings/query.rs');
const settingsMutationSource = read('apps/server/src/graphql/settings/mutation.rs');
const oauthGuardSource = read('apps/server/src/services/oauth_admin_guard.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = end ? content.indexOf(end, startIndex + start.length) : content.length;
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const requireOrder = (content, values, label) => {
  let cursor = -1;
  for (const value of values) {
    const next = content.indexOf(value, cursor + 1);
    if (next < 0 || next <= cursor) {
      failures.push(`${label}: expected ordered marker ${value}`);
      return;
    }
    cursor = next;
  }
};

for (const [value, label] of [
  ['pub enum HostAuthority', 'typed host authority'],
  ['Read,', 'host read authority'],
  ['Manage,', 'host manage authority'],
  ['pub struct HostAuthorityContext', 'typed host context'],
  ['actor_id: Uuid', 'operator identity binding'],
  ['(!actor_id.is_nil()).then_some', 'nil actor rejection'],
  ['pub fn for_actor', 'explicit actor constructor'],
  ['pub const fn allows', 'authority hierarchy'],
  ['(HostAuthority::Read, HostAuthority::Read)', 'explicit read hierarchy'],
  ['(HostAuthority::Manage, HostAuthority::Read)', 'explicit manage-to-read hierarchy'],
  ['(HostAuthority::Manage, HostAuthority::Manage)', 'explicit manage hierarchy'],
  ['HOST_AUTHORITY_REQUIRED', 'static denial'],
]) {
  requireText(authoritySource, value, label);
}
forbidText(
  authoritySource,
  '(HostAuthority::Manage, _)',
  'host authority hierarchy must not wildcard future levels',
);
requireText(apiContextSource, 'HostAuthorityContext', 'context export');
requireText(apiLibSource, 'HostAuthorityContext', 'crate export');
requireText(serverLibSource, 'pub mod host_authority;', 'server-owned credential module');

for (const [value, label] of [
  ['RUSTOK_HOST_AUTHORITY_CREDENTIALS', 'host-owned credential configuration'],
  ['x-rustok-host-token', 'dedicated host credential header'],
  ['token_sha256', 'stored token digest'],
  ['Sha256::digest(token.as_bytes())', 'presented token hashing'],
  ['presented_hash.ct_eq(&credential.token_sha256)', 'constant-time digest comparison'],
  ['MIN_TOKEN_BYTES', 'minimum token length'],
  ['MAX_TOKEN_BYTES', 'maximum token length'],
  ['MAX_CREDENTIALS', 'bounded credential set'],
  ['credential.actor_id.is_nil()', 'nil actor rejection'],
  ['must not contain duplicate token hashes', 'ambiguous token rejection'],
  ['HostAuthorityContext::for_actor', 'typed operator issuance'],
  ['headers.remove(HOST_AUTHORITY_TOKEN_HEADER)', 'raw credential removal'],
  ['tokio::task_local!', 'request-local typed authority'],
  ['pub async fn with_host_authority_scope', 'typed authority scope entry'],
  ['pub fn current_host_authority()', 'typed authority scope read'],
  ['header_is_removed_before_typed_authority_is_returned', 'header removal regression'],
  ['invalid_header_is_removed_before_denial', 'denied header removal regression'],
  ['typed_authority_is_request_scoped_and_does_not_leak', 'scope leakage regression'],
  ['rotation_can_overlap_distinct_tokens_for_the_same_actor', 'rotation overlap regression'],
]) {
  requireText(credentialSource, value, label);
}
for (const [value, label] of [
  ['OAuthApp', 'tenant OAuth applications must not issue host authority'],
  ['client_id', 'tenant OAuth client ids must not issue host authority'],
  ['Permission::', 'tenant permissions must not issue host authority'],
  ['UserRole', 'tenant roles must not issue host authority'],
  ['TenantContext', 'tenant identity must not issue host authority'],
  ['.scopes', 'OAuth scopes must not issue host authority'],
  ['.metadata', 'OAuth metadata must not issue host authority'],
]) {
  forbidText(credentialSource, value, label);
}
requireText(
  oauthGuardSource,
  'Permission::SETTINGS_MANAGE',
  'tenant OAuth admin remains tenant-managed and therefore outside host credentials',
);
forbidText(
  oauthGuardSource,
  'HostAuthority',
  'tenant OAuth admin must not manage host credentials',
);

const middlewareResolve = between(
  tenantMiddlewareSource,
  'pub async fn resolve_optional(',
  'fn is_human_user_self_service_path(',
  'host and tenant middleware composition',
);
requireOrder(
  middlewareResolve,
  [
    'take_host_authority(&mut parts.headers)',
    'resolve_current_user(&mut parts, &ctx).await',
    'parts.extensions.insert(AuthContextExtension(AuthContext {',
    'parts.extensions.insert(host_authority);',
    'with_host_authority_scope(',
    'with_rbac_request_scope(rbac_scope, next.run(req))',
  ],
  'one-shot host credential and tenant authentication composition',
);
requireText(
  middlewareResolve,
  'Err(crate::error::Error::Unauthorized(_))',
  'invalid presented host credential rejection',
);
forbidText(
  middlewareResolve,
  'resolve_host_authority(&parts.headers)',
  'raw host credential must not remain readable',
);
forbidText(
  tenantMiddlewareSource,
  'Permission::SETTINGS_',
  'native host authority must not derive from tenant settings permissions',
);

const graphqlWebsocket = between(
  graphqlControllerSource,
  'async fn graphql_ws_handler(',
  'async fn handle_graphql_ws(',
  'GraphQL WebSocket request data',
);
forbidText(
  graphqlWebsocket,
  'HOST_AUTHORITY_TOKEN_HEADER',
  'WebSocket host authority stays fail-closed',
);
const wsConnectionData = between(
  graphqlControllerSource,
  'async fn build_ws_connection_data(',
  'pub fn router()',
  'GraphQL WebSocket connection data',
);
forbidText(
  wsConnectionData,
  'HostAuthorityContext',
  'WebSocket connection data must not retain host authority',
);

const nativeRead = between(
  eventsNativeSource,
  'pub async fn event_delivery_configuration_native()',
  'pub async fn update_event_delivery_profile_native(',
  'native host-global read',
);
const nativeManage = between(
  eventsNativeSource,
  'pub async fn update_event_delivery_profile_native(',
  'pub(super) async fn fetch_configuration()',
  'native host-global mutation',
);
requireOrder(
  nativeRead,
  [
    'extract::<HostAuthorityContext>()',
    'authority.allows(HostAuthority::Read)',
    'shared_get::<SharedEventDeliveryControl>()',
    '.configuration()',
  ],
  'native event configuration read',
);
requireOrder(
  nativeManage,
  [
    'extract::<HostAuthorityContext>()',
    'authority.allows(HostAuthority::Manage)',
    'shared_get::<SharedEventDeliveryControl>()',
    '.update_profile(profile, authority.actor_id())',
  ],
  'native event profile mutation',
);
for (const [block, label] of [
  [nativeRead, 'native event configuration read'],
  [nativeManage, 'native event profile mutation'],
]) {
  forbidText(block, 'Permission::SETTINGS_', label);
  forbidText(block, 'has_effective_permission', label);
}

const iggyNativeRead = between(
  iggyNativeSource,
  'pub async fn iggy_connector_configuration_native()',
  'pub async fn update_iggy_connector_configuration_native(',
  'native Iggy host-global read',
);
const iggyNativeManage = between(
  iggyNativeSource,
  'pub async fn update_iggy_connector_configuration_native(',
  'pub(super) async fn fetch_configuration()',
  'native Iggy host-global mutation',
);
requireOrder(
  iggyNativeRead,
  [
    'extract::<HostAuthorityContext>()',
    'authority.allows(HostAuthority::Read)',
    'shared_get::<SharedIggyConnectorControl>()',
    '.configuration()',
  ],
  'native Iggy configuration read',
);
requireOrder(
  iggyNativeManage,
  [
    'extract::<HostAuthorityContext>()',
    'authority.allows(HostAuthority::Manage)',
    'extract::<AuthContext>()',
    'extract::<TenantContext>()',
    'auth.tenant_id != tenant.id',
    'shared_get::<SharedIggyConnectorControl>()',
    '.update_configuration(',
    'authority.actor_id(),',
    'auth.tenant_id,',
  ],
  'native Iggy configuration mutation',
);
for (const [block, label] of [
  [iggyNativeRead, 'native Iggy configuration read'],
  [iggyNativeManage, 'native Iggy configuration mutation'],
]) {
  forbidText(block, 'Permission::SETTINGS_', label);
  forbidText(block, 'has_effective_permission', label);
}
forbidText(
  iggyNativeManage,
  'auth.user_id',
  'native Iggy mutation audit actor must be host-owned',
);

const systemGuard = between(
  systemSource,
  'fn require_host_authority(',
  '// ── Query',
  'System GraphQL host authority helper',
);
requireOrder(
  systemGuard,
  [
    'current_host_authority()',
    '.filter(|authority| authority.allows(required))',
    'permission_denied("host-global authority required")',
  ],
  'System GraphQL typed authority consumption',
);
forbidText(systemGuard, 'HeaderMap', 'System GraphQL raw header access');
forbidText(
  systemGuard,
  'resolve_host_authority',
  'System GraphQL credential revalidation',
);
for (const [start, end, operation] of [
  ['async fn system_health(', 'async fn cache_health(', 'system health'],
  ['async fn cache_health(', 'async fn events_status(', 'cache health'],
  ['async fn events_status(', 'async fn session_stats(', 'events status'],
]) {
  const block = between(systemSource, start, end, operation);
  requireText(
    block,
    'require_host_authority(ctx, HostAuthority::Read)?;',
    `${operation} host read guard`,
  );
  forbidText(block, 'Permission::LOGS_', `${operation} tenant logs authority`);
}

for (const [value, label] of [
  ['current_host_authority()', 'Settings GraphQL typed host authority'],
  ['fn require_host_actor', 'Iggy tenant secret-owner helper'],
  ['ctx.data::<crate::context::AuthContext>()', 'Iggy tenant authentication'],
  ['ctx.data::<crate::context::TenantContext>()', 'Iggy routed tenant owner'],
  ['require_tenant_settings_scope(auth, tenant.id)?;', 'Iggy tenant equality'],
]) {
  requireText(settingsModuleSource, value, label);
}
forbidText(settingsModuleSource, 'HeaderMap', 'Settings GraphQL raw header access');
forbidText(
  settingsModuleSource,
  'resolve_host_authority',
  'Settings GraphQL credential revalidation',
);
forbidText(
  settingsModuleSource,
  'authority.actor_id() != auth.user_id',
  'host operator identity must not be inferred from tenant auth actor',
);

for (const [source, start, end, guard, service, operation] of [
  [
    settingsQuerySource,
    'async fn iggy_connector_configuration(',
    'async fn event_delivery_configuration(',
    'require_host_authority(ctx, HostAuthority::Read)?;',
    'IggyConnectorSettingsService::configuration',
    'Iggy configuration read',
  ],
  [
    settingsQuerySource,
    'async fn event_delivery_configuration(',
    'async fn platform_settings(',
    'require_host_authority(ctx, HostAuthority::Read)?;',
    'EventDeliverySettingsService::configuration',
    'event delivery configuration read',
  ],
  [
    settingsMutationSource,
    'async fn update_iggy_connector_configuration(',
    'async fn update_event_delivery_configuration(',
    'require_host_actor(ctx, HostAuthority::Manage)?;',
    'IggyConnectorSettingsService::save',
    'Iggy configuration mutation',
  ],
  [
    settingsMutationSource,
    'async fn update_event_delivery_configuration(',
    'async fn update_platform_settings(',
    'require_host_authority(ctx, HostAuthority::Manage)?;',
    'EventDeliverySettingsService::save_profile',
    'event delivery configuration mutation',
  ],
]) {
  const block = between(source, start, end, operation);
  requireOrder(block, [guard, service], operation);
  forbidText(block, 'Permission::SETTINGS_', `${operation} tenant settings authority`);
  forbidText(block, 'has_effective_permission', `${operation} tenant permission check`);
}

const iggyMutation = between(
  settingsMutationSource,
  'async fn update_iggy_connector_configuration(',
  'async fn update_event_delivery_configuration(',
  'Iggy configuration mutation actor audit',
);
requireOrder(
  iggyMutation,
  [
    'require_host_actor(ctx, HostAuthority::Manage)?;',
    'authority.actor_id()',
    'auth.tenant_id',
  ],
  'Iggy mutation operator and secret-owner inputs',
);

if (failures.length > 0) {
  console.error('Host-global authority boundary verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Host-owned opaque credentials are removed before dispatch and issue request-scoped typed HTTP/native authority for Events and Iggy independently from tenant OAuth, RBAC, scopes, roles and metadata; WebSocket remains fail-closed',
);
