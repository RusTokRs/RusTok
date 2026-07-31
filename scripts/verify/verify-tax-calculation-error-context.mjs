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
  'fn tax_calculation_context_facts(',
  'tax calculation port',
);
const contextHelper = between(
  source,
  'fn tax_calculation_context_facts(',
  'fn require_tax_calculation_policy(',
  'tax context fact helper',
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
  ['tenant_id_length: context.tenant_id.chars().count()', 'tenant length fact'],
  ['actor_kind,', 'actor kind fact'],
  ['actor_id_length: context.actor.id.chars().count()', 'actor id length fact'],
  ['claim_count: context.claims.len()', 'claim count fact'],
  ['role_count: context.roles.len()', 'role count fact'],
  ['channel_present: context.channel.is_some()', 'channel presence fact'],
  ['locale_length: context.locale.chars().count()', 'locale length fact'],
  ['causation_id_present: context.causation_id.is_some()', 'causation presence fact'],
  ['traceparent_present: context.traceparent.is_some()', 'trace presence fact'],
  ['idempotency_key_present: context.idempotency_key.is_some()', 'idempotency presence fact'],
  ['deadline_ms: context.deadline_ms', 'deadline fact'],
]) requireText(contextHelper, value, label);

for (const [value, label] of [
  [
    '.require_policy(PortCallPolicy::read())\n        .inspect_err(|error| {',
    'unchanged read policy admission',
  ],
  [
    'log_tax_calculation_policy_rejection(context, owner_operation, error);',
    'policy rejection diagnostics',
  ],
  ['owner = "rustok_tax"', 'policy owner log'],
  ['correlation_id = %context.correlation_id', 'policy correlation log'],
  ['tenant_id_length = facts.tenant_id_length', 'policy tenant shape'],
  ['actor_kind = facts.actor_kind', 'policy actor kind'],
  ['channel_present = facts.channel_present', 'policy channel shape'],
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
    ['tenant_id_length = facts.tenant_id_length', `${label} tenant shape`],
    ['actor_kind = facts.actor_kind', `${label} actor kind`],
    ['channel_present = facts.channel_present', `${label} channel shape`],
    ['operation = owner_operation', `${label} operation log`],
    ['boundary = TAX_CALCULATION_PORT_BOUNDARY', `${label} boundary log`],
  ]) requireText(content, value, detail);
}

for (const [value, label] of [
  ['let detail = detail.to_string();', 'request detail bounded conversion'],
  ['detail_present = !detail.trim().is_empty()', 'request detail presence'],
  ['detail_length = detail.chars().count()', 'request detail length'],
  ['code,', 'request stable code log'],
  ['PortError::validation(code, "tax request is invalid")', 'request static public envelope'],
]) requireText(requestMapper, value, label);

for (const [value, label] of [
  ['let detail = detail.to_string();', 'result detail bounded conversion'],
  ['detail_present = !detail.trim().is_empty()', 'result detail presence'],
  ['detail_length = detail.chars().count()', 'result detail length'],
  ['code,', 'result stable code log'],
  ['PortError::invariant_violation(code, "tax calculation result is invalid")', 'result static public envelope'],
]) requireText(resultMapper, value, label);

for (const [value, label] of [
  ['TaxError::Validation(message)', 'owner validation cause capture'],
  ['validation_message_present = !message.trim().is_empty()', 'owner validation presence'],
  ['validation_message_length = message.chars().count()', 'owner validation length'],
  ['code = "tax.validation"', 'owner validation stable code log'],
  ['PortError::validation("tax.validation", "tax request is invalid")', 'owner validation static public envelope'],
]) requireText(ownerMapper, value, label);

for (const value of [
  'PortError::validation(code, detail)',
  'PortError::validation("tax.validation", message)',
  'PortError::invariant_violation(code, detail)',
  '.map_err(tax_error_to_port_error)',
  'detail = %detail',
  'error = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'channel = ?context.channel',
  'locale = %context.locale',
  'causation_id = ?context.causation_id',
  'traceparent = ?context.traceparent',
  'idempotency_key = ?context.idempotency_key',
]) forbidText(source, value, 'unsafe tax diagnostic or public mapping');

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

console.log(
  '✔ Tax calculation owner errors retain stable envelopes, correlation, and safe context/detail shape',
);
