#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('crates/rustok-commerce/src/graphql/mod.rs', root),
  'utf8',
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const requireBefore = (content, first, second, label) => {
  const firstIndex = content.indexOf(first);
  const secondIndex = content.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    failures.push(`${label}: ${first} must precede ${second}`);
  }
};

const productMapper = between(
  source,
  'pub(crate) fn map_product_service_error(',
  'pub(crate) fn current_tenant_scope(',
  'product mapper',
);
const channelAdmission = between(
  source,
  'pub(crate) async fn require_storefront_channel_enabled(',
  '#[cfg(test)]',
  'channel admission',
);

for (const [value, label] of [
  ['#[path = "safe_query.rs"]\nmod query;', 'safe query routing'],
  ['pub struct CommerceQueryRoot(', 'query root'],
  ['pub use mutations::CommerceMutation;', 'mutation export'],
  ['pub(crate) const MODULE_SLUG: &str = "commerce";', 'module slug'],
  ['pub(crate) const PRODUCT_MODULE_SLUG: &str = "product";', 'product module slug'],
  [
    'const COMMERCE_GRAPHQL_CHANNEL_OWNER: &str = "rustok_commerce.storefront_channel";',
    'channel owner',
  ],
  [
    'const COMMERCE_GRAPHQL_CHANNEL_BOUNDARY: &str = "commerce_graphql_channel_admission";',
    'channel boundary',
  ],
  [
    'const COMMERCE_GRAPHQL_CHANNEL_OPERATION: &str = "require_storefront_channel_enabled";',
    'channel operation',
  ],
  ['struct CommerceGraphqlChannelDiagnosticContext', 'diagnostic context'],
  ['impl From<&RequestContext> for CommerceGraphqlChannelDiagnosticContext', 'context projection'],
  ['tenant_id_shape: uuid_shape(context.tenant_id)', 'tenant UUID shape'],
  ['channel_id_shape: optional_uuid_shape(context.channel_id)', 'channel UUID shape'],
  [
    'channel_slug_shape: optional_text_shape(context.channel_slug.as_deref())',
    'channel slug shape',
  ],
  ['struct CommerceGraphqlChannelDiagnosticError;', 'diagnostic error'],
  ['formatter.write_str("redacted")', 'redacted Debug'],
  ['fn uuid_shape(value: uuid::Uuid)', 'UUID shape helper'],
  ['fn optional_uuid_shape(value: Option<uuid::Uuid>)', 'optional UUID shape helper'],
  ['fn optional_text_shape(value: Option<&str>)', 'optional text shape helper'],
]) requireText(source, value, label);

for (const [value, label] of [
  [
    'rustok_product::map_product_public_error(&error, operation, "commerce_graphql_product")',
    'owner product mapping',
  ],
  ['async_graphql::Error::new(public.message)', 'product public message'],
  ['extensions.set("code", public.code)', 'product public code'],
  ['extensions.set("retryable", public.retryable)', 'product retryability'],
  [
    'extensions.set("correlation_id", public.correlation_id.to_string())',
    'product correlation extension',
  ],
]) requireText(productMapper, value, label);

for (const [value, label] of [
  ['let Some(request_context) = ctx.data_opt::<RequestContext>() else', 'optional request context'],
  ['return Ok(());', 'context-free compatibility'],
  [
    'let diagnostic_context = CommerceGraphqlChannelDiagnosticContext::from(request_context);',
    'diagnostic projection',
  ],
  ['let db = ctx.data::<DatabaseConnection>()?;', 'database dependency'],
  [
    'is_module_enabled_for_request_channel(db, request_context, MODULE_SLUG)',
    'channel owner call',
  ],
  ['.map_err(|_error| {', 'discarded storage cause'],
  ['let error = CommerceGraphqlChannelDiagnosticError;', 'redacted error shadow'],
  ['error = ?error', 'redacted error field'],
  ['owner = COMMERCE_GRAPHQL_CHANNEL_OWNER', 'owner log field'],
  ['tenant_id_shape = diagnostic_context.tenant_id_shape', 'tenant shape log'],
  ['channel_id_shape = diagnostic_context.channel_id_shape', 'channel ID shape log'],
  ['channel_slug_shape = diagnostic_context.channel_slug_shape', 'channel slug shape log'],
  ['operation = COMMERCE_GRAPHQL_CHANNEL_OPERATION', 'operation log'],
  ['error_kind = "storage"', 'storage error kind'],
  ['public_code = "COMMERCE_AVAILABILITY_UNAVAILABLE"', 'storage public code'],
  ['retryable = true', 'storage retryability'],
  ['boundary = COMMERCE_GRAPHQL_CHANNEL_BOUNDARY', 'storage boundary'],
  [
    '"Commerce availability could not be verified"',
    'stable availability message',
  ],
  ['if !enabled {', 'disabled policy'],
  ['tracing::warn!(', 'disabled warning'],
  ['error_kind = "module_disabled"', 'disabled error kind'],
  ['public_code = "MODULE_NOT_ENABLED"', 'disabled public code log'],
  ['retryable = false', 'disabled retryability'],
  [
    'async_graphql::Error::new("Commerce is not enabled for the current channel")',
    'stable disabled message',
  ],
  ['ext.set("code", "MODULE_NOT_ENABLED")', 'stable disabled code'],
]) requireText(channelAdmission, value, label);

requireBefore(
  channelAdmission,
  'let diagnostic_context = CommerceGraphqlChannelDiagnosticContext::from(request_context);',
  'is_module_enabled_for_request_channel(db, request_context, MODULE_SLUG)',
  'projection before owner call',
);
requireBefore(
  channelAdmission,
  'let error = CommerceGraphqlChannelDiagnosticError;',
  'tracing::error!(',
  'error shadow before diagnostic event',
);

for (const value of [
  'error = ?_error',
  'error = ?error,\n                tenant_id =',
  'tenant_id = %request_context.tenant_id',
  'channel_id = ?request_context.channel_id',
  'channel_slug = ?request_context.channel_slug',
  'channel_slug = %request_context.channel_slug',
  'tenant_id_shape = %request_context.tenant_id',
  'channel_id_shape = ?request_context.channel_id',
  'channel_slug_shape = ?request_context.channel_slug',
  '"Module check failed: {err}"',
  'format!("Module check failed: {err}")',
  'request_context.channel_slug.as_deref().unwrap_or("current")',
]) forbidText(channelAdmission, value, 'raw channel diagnostic or dynamic public error');

if ((channelAdmission.match(/tracing::error!\(/g) ?? []).length !== 1) {
  failures.push('expected one channel dependency error event');
}
if ((channelAdmission.match(/tracing::warn!\(/g) ?? []).length !== 1) {
  failures.push('expected one disabled-channel warning event');
}
if ((channelAdmission.match(/boundary = COMMERCE_GRAPHQL_CHANNEL_BOUNDARY/g) ?? []).length !== 2) {
  failures.push('expected both channel events to use the bounded boundary');
}

if (failures.length > 0) {
  console.error('Commerce GraphQL root diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL channel admission keeps stable envelopes and emits bounded redacted diagnostics',
);
