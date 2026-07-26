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
    'context.require_policy(PortCallPolicy::read()).map_err(|error| {',
    'unchanged read policy admission',
  ],
  [
    'log_tax_calculation_policy_rejection(context, owner_operation, &error);',
    'rejection diagnostics before rethrow',
  ],
  ['fn log_tax_calculation_policy_rejection(', 'structured diagnostic helper'],
  ['owner = "rustok_tax"', 'truthful tax owner'],
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['operation = owner_operation', 'exact owner operation'],
  ['code = %error.code', 'original port code'],
  ['error_kind = ?error.kind', 'original port kind'],
  ['retryable = error.retryable', 'original retryability'],
  ['boundary = TAX_CALCULATION_PORT_BOUNDARY', 'boundary identity'],
  ['"tax calculation policy admission failed"', 'error diagnostic event'],
  [
    '"tax calculation policy admission was rejected"',
    'warning diagnostic event',
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
  ['error\n    })', 'original PortError rethrow'],
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
  '✔ Tax calculation policy rejections retain complete owner context and the original PortError',
);
