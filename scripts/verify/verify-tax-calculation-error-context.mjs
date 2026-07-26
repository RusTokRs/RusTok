#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const source = read('crates/rustok-tax/src/ports.rs');
const portContract = read('crates/rustok-api/src/ports.rs');
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

const calculationPort = between(
  source,
  'impl TaxCalculationPort for crate::TaxService {',
  'fn require_tax_calculation_policy(',
  'tax calculation port',
);
const policyHelper = between(
  source,
  'fn require_tax_calculation_policy(',
  'fn validate_tax_request(',
  'tax calculation policy helper',
);
const requestMapper = between(
  source,
  'fn tax_request_error(',
  'fn tax_result_error(',
  'tax request mapper',
);
const resultMapper = between(
  source,
  'fn tax_result_error(',
  'fn tax_error_to_port_error(',
  'tax result mapper',
);
const ownerMapper = between(
  source,
  'fn tax_error_to_port_error(',
  '#[cfg(test)]',
  'tax owner mapper',
);

for (const [value, label] of [
  [
    'use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};',
    'typed port imports',
  ],
  ['let owner_operation = "calculate_tax";', 'owner operation'],
  [
    'require_tax_calculation_policy(&context, owner_operation)?;',
    'read policy requirement',
  ],
  ['tax_error_to_port_error(&context, owner_operation, error)', 'context-aware owner mapper call'],
  ['validate_tax_request(&context, owner_operation, &request)', 'context-aware request validation'],
  ['validate_tax_result(', 'context-aware result validation'],
]) requireText(calculationPort, value, label);

for (const [value, label] of [
  [
    'context.require_policy(PortCallPolicy::read()).map_err(|error| {',
    'unchanged read policy admission',
  ],
  [
    'log_tax_calculation_policy_rejection(context, owner_operation, &error);',
    'policy rejection diagnostics',
  ],
  ['owner = "rustok_tax"', 'policy owner log'],
  ['correlation_id = %context.correlation_id', 'policy correlation log'],
  ['tenant_id = %context.tenant_id', 'policy tenant log'],
  ['channel = ?context.channel', 'policy channel log'],
  ['operation = owner_operation', 'policy operation log'],
  ['boundary = TAX_CALCULATION_PORT_BOUNDARY', 'policy boundary log'],
]) requireText(policyHelper, value, label);

for (const [content, label] of [
  [requestMapper, 'tax request mapper'],
  [resultMapper, 'tax result mapper'],
  [ownerMapper, 'tax owner mapper'],
]) {
  for (const [value, detail] of [
    ['context: &PortContext', `${label} context input`],
    ["owner_operation: &'static str", `${label} operation input`],
    ['owner = "rustok_tax"', `${label} owner log`],
    ['correlation_id = %context.correlation_id', `${label} correlation log`],
    ['tenant_id = %context.tenant_id', `${label} tenant log`],
    ['channel = ?context.channel', `${label} channel log`],
    ['operation = owner_operation', `${label} operation log`],
  ]) requireText(content, value, detail);
}

for (const [value, label] of [
  ['detail = %detail', 'request internal detail log'],
  ['code,', 'request stable code log'],
  ['PortError::validation(code, "tax request is invalid")', 'request static public envelope'],
]) requireText(requestMapper, value, label);

for (const [value, label] of [
  ['detail = %detail', 'result internal detail log'],
  ['code,', 'result stable code log'],
  ['PortError::invariant_violation(code, "tax calculation result is invalid")', 'result static public envelope'],
]) requireText(resultMapper, value, label);

for (const [value, label] of [
  ['TaxError::Validation(message)', 'owner validation cause capture'],
  ['error = %message', 'owner validation internal cause log'],
  ['code = "tax.validation"', 'owner validation stable code log'],
  ['PortError::validation("tax.validation", "tax request is invalid")', 'owner validation static public envelope'],
]) requireText(ownerMapper, value, label);

for (const value of [
  'PortError::validation(code, detail)',
  'PortError::validation("tax.validation", message)',
  'PortError::invariant_violation(code, detail)',
  '.map_err(tax_error_to_port_error)',
  'context.require_policy(PortCallPolicy::read())?;\n        let owner_operation',
]) forbidText(source, value, 'unsafe tax public mapping');

for (const [value, label] of [
  ['pub struct PortContext {', 'shared port context'],
  ['pub correlation_id: String', 'shared correlation field'],
  ['pub channel: Option<String>', 'shared channel field'],
  ['pub struct PortError {', 'shared port error'],
  ['pub fn validation(', 'typed validation constructor'],
  ['pub fn invariant_violation(', 'typed invariant constructor'],
]) requireText(portContract, value, label);

if (failures.length > 0) {
  console.error('Tax calculation error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Tax calculation errors retain owner, channel, correlation, and static public envelopes');
