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
const serverAuthSource = read('apps/server/src/auth.rs');
const oauthTokenSource = read('apps/server/src/services/oauth_token_service.rs');
const tenantMiddlewareSource = read('apps/server/src/middleware/auth_context.rs');
const eventsNativeSource = read(
  'crates/rustok-events-module/admin/src/transport/native_server_adapter.rs',
);
const systemSource = read('apps/server/src/graphql/system.rs');
const settingsModuleSource = read('apps/server/src/graphql/settings/mod.rs');
const settingsQuerySource = read('apps/server/src/graphql/settings/query.rs');
const settingsMutationSource = read('apps/server/src/graphql/settings/mutation.rs');
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

const hostPolicyBlock = between(
  serverAuthSource,
  'pub struct HostAuthorityPolicy',
  'pub struct PasswordResetClaims',
  'host operator policy',
);
for (const [value, label] of [
  ['RUSTOK_HOST_AUTHORITY_CLIENTS', 'host-owned configuration key'],
  ['<client-uuid>=read|manage', 'documented exact mapping format'],
  ['Uuid::parse_str', 'exact client UUID parsing'],
  ['client_id.is_nil()', 'nil client rejection'],
  ['contains duplicate client UUID', 'duplicate client rejection'],
  ['SecurityActorKind::Service', 'service-only authority'],
  ['grant_type != CLIENT_CREDENTIALS_GRANT_TYPE', 'client-credentials-only authority'],
  ['HostAuthorityContext::for_actor(authority, actor_id)', 'typed operator context'],
]) {
  requireText(hostPolicyBlock, value, label);
}
for (const [value, label] of [
  ['Permission::', 'tenant permissions must not issue host authority'],
  ['has_effective_permission', 'tenant permission implication must not issue host authority'],
  ['.scopes', 'OAuth scopes must not issue host authority'],
  ['.metadata', 'OAuth app metadata must not issue host authority'],
  ['UserRole::', 'tenant roles must not issue host authority'],
]) {
  forbidText(hostPolicyBlock, value, label);
}

const serviceTokenIssue = between(
  oauthTokenSource,
  'fn issue_service_access_token(',
  'async fn prepare_user_tokens(',
  'service token issuance',
);
requireText(
  serviceTokenIssue,
  'CLIENT_CREDENTIALS_GRANT',
  'service tokens retain explicit client-credentials grant',
);
forbidText(
  serviceTokenIssue,
  'HostAuthorityContext',
  'JWT issuance must not persist host authority',
);

const userTokenIssue = between(
  oauthTokenSource,
  'async fn prepare_user_tokens(',
  'async fn commit_authorization_code_exchange(',
  'user token issuance',
);
requireText(
  userTokenIssue,
  'AUTHORIZATION_CODE_GRANT',
  'authorization-code access tokens remain user grants',
);
forbidText(
  userTokenIssue,
  'HostAuthorityContext',
  'authorization-code and refresh issuance stay tenant-only',
);

const authorizationCodeExchange = between(
  oauthTokenSource,
  'AUTHORIZATION_CODE_GRANT => {',
  'REFRESH_TOKEN_GRANT => {',
  'authorization-code exchange',
);
requireOrder(
  authorizationCodeExchange,
  ['AUTHORIZATION_CODE_GRANT => {', 'prepare_user_tokens('],
  'authorization-code exchange uses tenant-user token preparation',
);
const refreshExchange = between(
  oauthTokenSource,
  'REFRESH_TOKEN_GRANT => {',
  'other => {',
  'refresh-token exchange',
);
requireOrder(
  refreshExchange,
  ['REFRESH_TOKEN_GRANT => {', 'prepare_user_tokens('],
  'refresh-token exchange uses tenant-user token preparation',
);

requireOrder(
  tenantMiddlewareSource,
  [
    'resolve_current_user(&mut parts, &ctx).await',
    'host_authority_context_for_authenticated_actor(',
    'parts.extensions.insert(host_authority);',
    'parts.extensions.insert(AuthContextExtension(AuthContext {',
  ],
  'native request host-authority issuance after authentication',
);
forbidText(
  tenantMiddlewareSource,
  'Permission::SETTINGS_',
  'native host authority must not derive from tenant settings permissions',
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

const systemGuard = between(
  systemSource,
  'fn require_host_authority(',
  '// ── Query',
  'system host authority helper',
);
requireOrder(
  systemGuard,
  [
    'ctx.data::<AuthContext>()',
    'host_authority_context_for_authenticated_actor(',
    '.filter(|authority| authority.allows(required))',
  ],
  'System GraphQL host authority derivation',
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
  ['fn require_host_authority(', 'settings host authority helper'],
  ['host_authority_context_for_authenticated_actor(', 'settings host policy lookup'],
  ['fn require_host_actor', 'settings host actor helper'],
  ['authority.actor_id() != auth.user_id', 'operator/auth actor equality'],
]) {
  requireText(settingsModuleSource, value, label);
}

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
  '✔ Exact host-configured client_credentials services receive typed host authority; human, tenant RBAC, scope, metadata and wildcard paths remain denied',
);
