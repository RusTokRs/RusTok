#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(new URL('crates/rustok-tax/src/ports.rs', root), 'utf8');
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  [
    'const TAX_CALCULATION_PORT_BOUNDARY: &str = "tax_calculation_port";',
    'stable tax calculation owner boundary',
  ],
  ['PortErrorKind', 'typed port error classification import'],
  ['let owner_operation = "calculate_tax";', 'owner operation assignment'],
  [
    'require_tax_calculation_policy(&context, owner_operation)?;',
    'shared policy admission helper',
  ],
  ['fn require_tax_calculation_policy(', 'policy helper definition'],
  [
    '.require_policy(PortCallPolicy::read())\n        .inspect_err(|error| {',
    'unchanged read policy admission',
  ],
  [
    'log_tax_calculation_policy_rejection(context, owner_operation, error);',
    'rejection diagnostics before unchanged return',
  ],
  ['fn tax_calculation_context_facts(', 'safe context fact helper'],
  ['fn tax_port_error_kind(', 'closed port error kind helper'],
  ['PortErrorKind::Validation => "validation"', 'validation kind label'],
  ['PortErrorKind::NotFound => "not_found"', 'not-found kind label'],
  ['PortErrorKind::Conflict => "conflict"', 'conflict kind label'],
  ['PortErrorKind::Forbidden => "forbidden"', 'forbidden kind label'],
  ['PortErrorKind::Unavailable => "unavailable"', 'unavailable kind label'],
  ['PortErrorKind::Timeout => "timeout"', 'timeout kind label'],
  [
    'PortErrorKind::InvariantViolation => "invariant_violation"',
    'invariant kind label',
  ],
  ['fn log_tax_calculation_policy_rejection(', 'structured diagnostic helper'],
  ['owner = "rustok_tax"', 'truthful tax owner'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id_length = facts.tenant_id_length', 'tenant shape'],
  ['actor_kind = facts.actor_kind', 'actor kind'],
  ['actor_id_length = facts.actor_id_length', 'actor id shape'],
  ['claim_count = facts.claim_count', 'claim count'],
  ['role_count = facts.role_count', 'role count'],
  ['channel_present = facts.channel_present', 'channel presence'],
  ['channel_length = ?facts.channel_length', 'channel length'],
  ['locale_length = facts.locale_length', 'locale length'],
  ['causation_id_present = facts.causation_id_present', 'causation presence'],
  ['causation_id_length = ?facts.causation_id_length', 'causation length'],
  ['traceparent_present = facts.traceparent_present', 'trace presence'],
  ['traceparent_length = ?facts.traceparent_length', 'trace length'],
  ['idempotency_key_present = facts.idempotency_key_present', 'idempotency presence'],
  ['idempotency_key_length = ?facts.idempotency_key_length', 'idempotency length'],
  ['deadline_ms = ?facts.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['code = %error.code', 'stable port code'],
  [
    'error_kind = tax_port_error_kind(&error.kind)',
    'closed port error kind label',
  ],
  [
    'error_message_present = !error.message.is_empty()',
    'port error message presence',
  ],
  [
    'error_message_length = error.message.chars().count()',
    'port error message length',
  ],
  ['retryable = error.retryable', 'original retryability'],
  ['boundary = TAX_CALCULATION_PORT_BOUNDARY', 'boundary identity'],
  [
    '"tax calculation policy admission failed with bounded diagnostics"',
    'technical diagnostic event',
  ],
  [
    '"tax calculation policy admission was rejected with bounded diagnostics"',
    'ordinary diagnostic event',
  ],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'severity classification',
  ],
]) {
  requireText(value, label);
}

const operationIndex = source.indexOf('let owner_operation = "calculate_tax";');
const policyIndex = source.indexOf('require_tax_calculation_policy(&context, owner_operation)?;');
const validationIndex = source.indexOf(
  'let expected_currency = validate_tax_request(&context, owner_operation, &request)?;',
);
if (!(operationIndex >= 0 && operationIndex < policyIndex && policyIndex < validationIndex)) {
  failures.push('calculate_tax must assign operation, admit policy, then validate request');
}

for (const [value, label] of [
  [
    '.map_err(|error| tax_error_to_port_error(&context, owner_operation, error))?',
    'existing owner error mapping',
  ],
  ['PortError::validation(code, "tax request is invalid")', 'stable request envelope'],
  [
    'PortError::invariant_violation(code, "tax calculation result is invalid")',
    'stable result envelope',
  ],
  ['PortError::validation("tax.validation", "tax request is invalid")', 'stable service envelope'],
  ['validate_tax_result(', 'existing result validation'],
]) {
  requireText(value, label);
}

for (const [value, label] of [
  ['error = ?error', 'complete PortError debug payload'],
  ['error = %error', 'complete PortError display payload'],
  ['error_kind = ?error.kind', 'debug-formatted PortError kind'],
  ['error_message = %error.message', 'raw PortError message'],
  ['message = %error.message', 'raw PortError message alias'],
  ['tenant_id = %context.tenant_id', 'raw tenant context'],
  ['actor = ?context.actor', 'raw actor context'],
  ['channel = ?context.channel', 'raw channel context'],
  ['locale = %context.locale', 'raw locale context'],
  ['causation_id = ?context.causation_id', 'raw causation context'],
  ['traceparent = ?context.traceparent', 'raw trace context'],
  ['idempotency_key = ?context.idempotency_key', 'raw idempotency context'],
]) {
  forbidText(value, label);
}

forbidText(
  'context.require_policy(PortCallPolicy::read())?;\n        let owner_operation',
  'policy admission before owner operation',
);

if (failures.length > 0) {
  console.error('Tax calculation policy context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Tax calculation policy rejections retain correlation and bounded PortError shape without logging the complete envelope',
);
