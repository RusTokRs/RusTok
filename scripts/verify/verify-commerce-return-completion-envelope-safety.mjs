#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read(
  'crates/rustok-commerce/src/controllers/return_completion_operations.rs',
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

const listRoute = between(
  source,
  'pub async fn list_return_completion_operations(',
  '#[utoipa::path(\n    get,\n    path = "/admin/return-completion-operations/{id}"',
  'return completion list route',
);
const showRoute = between(
  source,
  'pub async fn show_return_completion_operation(',
  '#[utoipa::path(\n    post,\n    path = "/admin/return-completion-operations/{id}/retry"',
  'return completion show route',
);
const retryRoute = between(
  source,
  'pub async fn retry_return_completion_operation(',
  'fn return_completion_operator_policy(',
  'return completion retry route',
);
const policy = between(
  source,
  'fn return_completion_operator_policy(',
  'fn map_operator_error(',
  'return completion operator policy',
);
const mapperStart = source.indexOf('fn map_operator_error(');
const mapper = mapperStart < 0 ? '' : source.slice(mapperStart);
if (mapperStart < 0) failures.push('return completion mapper: unable to isolate source block');

for (const [value, label] of [
  [
    'const RETURN_COMPLETION_OPERATOR_OWNER: &str =',
    'bounded owner constant',
  ],
  [
    'const RETURN_COMPLETION_OPERATOR_BOUNDARY: &str =',
    'bounded boundary constant',
  ],
  ['struct ReturnCompletionOperatorErrorContext {', 'typed route context'],
  ['tenant_id: Uuid,', 'typed tenant identity'],
  ['actor_id: Uuid,', 'typed actor identity'],
  ['operation_id: Option<Uuid>,', 'typed operation identity'],
  ["operation: &'static str,", 'typed route operation'],
  [
    'struct ReturnCompletionOperatorDiagnosticContext {',
    'bounded diagnostic context',
  ],
  [
    'impl From<&ReturnCompletionOperatorErrorContext>',
    'typed-to-diagnostic conversion',
  ],
  [
    'struct ReturnCompletionOperatorDiagnosticError;',
    'bounded diagnostic error',
  ],
  ['formatter.write_str("redacted")', 'redacted Debug output'],
  ["fn uuid_shape(value: Uuid) -> &'static str", 'required UUID shape'],
  [
    "fn optional_uuid_shape(value: Option<Uuid>) -> &'static str",
    'optional UUID shape',
  ],
  ['"nil"', 'nil UUID shape'],
  ['"non_nil"', 'non-nil UUID shape'],
  ['"absent"', 'absent optional UUID shape'],
  ['"present_nil"', 'present nil optional UUID shape'],
  ['"present_non_nil"', 'present non-nil optional UUID shape'],
]) requireText(source, value, label);

for (const [block, operationId, operation, ownerCall, permission, label] of [
  [
    listRoute,
    'None,',
    '"list_return_completion_operations"',
    '.list_operations(',
    'Permission::ORDERS_READ',
    'list route',
  ],
  [
    showRoute,
    'Some(id),',
    '"show_return_completion_operation"',
    '.get_operation(tenant.id, id)',
    'Permission::ORDERS_READ',
    'show route',
  ],
  [
    retryRoute,
    'Some(id),',
    '"retry_return_completion_operation"',
    '.retry_operation(tenant.id, auth.user_id, id)',
    'Permission::ORDERS_MANAGE, Permission::PAYMENTS_MANAGE',
    'retry route',
  ],
]) {
  requireText(block, 'ReturnCompletionOperatorErrorContext::new(', `${label} context`);
  requireText(block, 'tenant.id,', `${label} tenant identity`);
  requireText(block, 'auth.user_id,', `${label} actor identity`);
  requireText(block, operationId, `${label} operation identity`);
  requireText(block, operation, `${label} operation label`);
  requireText(block, ownerCall, `${label} owner call`);
  requireText(block, permission, `${label} permission`);
  requireText(block, 'map_operator_error(', `${label} mapper`);
  requireText(
    block,
    '.with_payment_provider_registry(runtime.payment_provider_registry())',
    `${label} provider registry`,
  );
}

for (const [value, label] of [
  [
    'PostOrderOrchestrationError::Validation(message) if message.contains("was not found")',
    'not-found classifier',
  ],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['"return_completion_operation_not_found"', 'not-found code'],
  ['"Return completion operation not found"', 'not-found message'],
  ['message.contains("currently leased")', 'lease conflict classifier'],
  ['message.contains("requires reconciliation")', 'reconciliation classifier'],
  ['message.contains("terminally failed")', 'terminal failure classifier'],
  ['message.contains("already completed")', 'completed classifier'],
  ['message.contains("different completion command")', 'command classifier'],
  [
    'message.contains("already bound to another command")',
    'bound command classifier',
  ],
  ['message.contains("command hash does not match")', 'hash classifier'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['"return_completion_operation_conflict"', 'conflict code'],
  [
    '"Return completion operation conflicts with the current state"',
    'static conflict message',
  ],
  ['OrderError::Database(_) | rustok_order::error::OrderError::Core(_)', 'storage classifier'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage unavailable status'],
  ['"return_completion_storage_unavailable"', 'storage unavailable code'],
  [
    '"Return completion recovery storage is unavailable"',
    'storage unavailable message',
  ],
  ['_ => None', 'shared fallback admission'],
]) requireText(policy, value, label);

for (const [value, label] of [
  [
    'return_completion_operator_policy(&error)',
    'typed route policy selection',
  ],
  [
    'return super::admin::map_post_order_orchestration_error(error);',
    'shared fallback',
  ],
  [
    'let context = ReturnCompletionOperatorDiagnosticContext::from(&context);',
    'diagnostic context shadow',
  ],
  [
    'let error = ReturnCompletionOperatorDiagnosticError;',
    'diagnostic error shadow',
  ],
  ['error = ?error', 'redacted diagnostic event'],
  ['owner = RETURN_COMPLETION_OPERATOR_OWNER', 'owner event'],
  [
    'source_owner = "rustok_commerce.post_order_orchestration"',
    'source owner event',
  ],
  ['tenant_id = %context.tenant_id', 'tenant shape event'],
  ['actor_id = %context.actor_id', 'actor shape event'],
  ['operation_id = ?context.operation_id', 'operation ID shape event'],
  ['operation = %context.operation', 'route operation event'],
  ['error_kind,', 'error kind event'],
  ['public_code = code', 'public code event'],
  ['status = %status', 'HTTP status event'],
  ['boundary = RETURN_COMPLETION_OPERATOR_BOUNDARY', 'boundary event'],
  ['HttpError::new(status, code, message)', 'static public envelope'],
]) requireText(mapper, value, label);

requireBefore(
  mapper,
  'return_completion_operator_policy(&error)',
  'let error = ReturnCompletionOperatorDiagnosticError;',
  'typed policy before diagnostic shadow',
);
requireBefore(
  mapper,
  'let error = ReturnCompletionOperatorDiagnosticError;',
  'tracing::error!(',
  'diagnostic shadow before event',
);

for (const value of [
  'HttpError::new(\n                StatusCode::CONFLICT,\n                "return_completion_operation_conflict",\n                message,',
  'HttpError::new(StatusCode::CONFLICT, "return_completion_operation_conflict", message)',
]) forbidText(source, value, 'unsafe raw conflict envelope');

const mapperUses = source.match(/map_operator_error\(/g) ?? [];
if (mapperUses.length !== 4) {
  failures.push(`expected mapper definition plus three uses, found ${mapperUses.length}`);
}
const contextUses = source.match(/ReturnCompletionOperatorErrorContext::new\(/g) ?? [];
if (contextUses.length !== 3) {
  failures.push(`expected three route contexts, found ${contextUses.length}`);
}

for (const [value, label] of [
  ['pub fn axum_router()', 'router'],
  ['"/compensation-sweep"', 'unrelated route absence guard'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'list pagination'],
  ['Ok(Json(operation))', 'show response'],
  ['Ok(Json(order_return))', 'retry response'],
]) {
  if (label === 'unrelated route absence guard') {
    forbidText(source, value, label);
  } else {
    requireText(source, value, label);
  }
}

if (failures.length > 0) {
  console.error('Commerce return completion envelope-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Return completion operator conflicts use static envelopes and bounded route diagnostics',
);
