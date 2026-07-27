#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);

const wrapper = readFileSync(
  new URL('crates/rustok-tax/src/calculation_context.rs', root),
  'utf8',
);
const library = readFileSync(new URL('crates/rustok-tax/src/lib.rs', root), 'utf8');
const owner = readFileSync(new URL('crates/rustok-tax/src/ports.rs', root), 'utf8');
const evidence = readFileSync(
  new URL('crates/rustok-tax/docs/calculation-local-context.md', root),
  'utf8',
);
const failures = [];

const requireIn = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidIn = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const [value, label] of [
  ['mod calculation_context;', 'private context wrapper module'],
  ['pub mod ports;', 'legacy module compatibility'],
  ['pub use calculation_context::{', 'canonical wrapper exports'],
  ['InProcessTaxCalculationPort, in_process_tax_calculation_port', 'root type and factory'],
  ['pub use ports::TaxCalculationPort;', 'root trait contract'],
]) {
  requireIn(library, value, label);
}
forbidIn(library, 'pub use ports::*;', 'legacy wildcard root construction');

for (const [value, label] of [
  ['const TAX_OWNER: &str = "rustok_tax";', 'truthful owner'],
  ['const CALCULATE_TAX_OPERATION: &str = "calculate_tax";', 'public operation'],
  ['const TAX_CALCULATION_BOUNDARY: &str = "tax_calculation_port";', 'stable boundary'],
  ['pub struct InProcessTaxCalculationPort', 'canonical wrapper type'],
  ['inner: TaxService', 'unchanged owner service delegation'],
  ['pub fn new() -> Self', 'default wrapper constructor'],
  ['pub fn from_service(inner: TaxService) -> Self', 'host-composed service constructor'],
  ['pub fn in_process_tax_calculation_port() -> Arc<dyn TaxCalculationPort>', 'canonical root factory'],
  ['let diagnostic_context = context.clone();', 'delegated context retention'],
  ['let diagnostic_facts = tax_calculation_diagnostic_facts(&request);', 'safe request fact retention'],
  ['let result = self.inner.calculate_tax(context, request).await;', 'unchanged owner delegation'],
  ['result.map_err(|error| {', 'post-delegation outcome mapping'],
  ['map_tax_calculation_local_port_error(', 'local mapper use'],
  ['_ => return error,', 'unknown outcome pass-through'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation', 'technical severity classification'],
  ['"tax calculation local technical outcome retained delegated context"', 'technical diagnostic event'],
  ['"tax calculation local outcome retained delegated context"', 'ordinary diagnostic event'],
  ['error\n}', 'same delegated error return'],
]) {
  requireIn(wrapper, value, label);
}

const cloneIndex = wrapper.indexOf('let diagnostic_context = context.clone();');
const factsIndex = wrapper.indexOf(
  'let diagnostic_facts = tax_calculation_diagnostic_facts(&request);',
);
const delegateIndex = wrapper.indexOf(
  'let result = self.inner.calculate_tax(context, request).await;',
);
const mapIndex = wrapper.indexOf('result.map_err(|error| {');
if (!(cloneIndex >= 0 && cloneIndex < factsIndex && factsIndex < delegateIndex && delegateIndex < mapIndex)) {
  failures.push('wrapper must retain context and safe facts before unchanged delegation, then map');
}

for (const [value, label] of [
  ['correlation_id = %context.correlation_id', 'correlation context'],
  ['tenant_id = %context.tenant_id', 'tenant context'],
  ['actor = ?context.actor', 'actor context'],
  ['channel = ?context.channel', 'channel context'],
  ['locale = %context.locale', 'locale context'],
  ['causation_id = ?context.causation_id', 'causation context'],
  ['traceparent = ?context.traceparent', 'trace context'],
  ['idempotency_key = ?context.idempotency_key', 'idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'deadline context'],
  ['currency_code_length = facts.currency_code_length', 'currency shape'],
  ['channel_id = ?facts.channel_id', 'typed channel identity'],
  ['customer_tax_exempt = facts.customer_tax_exempt', 'tax exemption flag'],
  ['taxable_amount_count = facts.taxable_amount_count', 'taxable amount count'],
  ['line_item_target_count = facts.line_item_target_count', 'line target count'],
  ['shipping_target_count = facts.shipping_target_count', 'shipping target count'],
  ['dual_target_count = facts.dual_target_count', 'dual target count'],
  ['country_rule_count = facts.country_rule_count', 'country rule count'],
  ['provider_id_length = ?facts.provider_id_length', 'provider id length'],
  ['channel_provider_id_length = ?facts.channel_provider_id_length', 'channel provider id length'],
  ['country_code_length = ?facts.country_code_length', 'country code length'],
  ['internal_code = %error.code', 'stable internal code'],
  ['internal_message = %error.message', 'public-safe internal message'],
  ['error_kind = ?error.kind', 'typed error kind'],
  ['retryable = error.retryable', 'retryability'],
  ['boundary = TAX_CALCULATION_BOUNDARY', 'boundary field'],
]) {
  requireIn(wrapper, value, label);
}

for (const [code, message, label] of [
  ['tax.currency_code_invalid', 'tax request is invalid', 'request currency validation'],
  ['tax.negative_policy_rate', 'tax request is invalid', 'policy rate validation'],
  ['tax.country_code_invalid', 'tax request is invalid', 'country code validation'],
  ['tax.negative_country_rate', 'tax request is invalid', 'country rate validation'],
  ['tax.duplicate_country_rule', 'tax request is invalid', 'country rule uniqueness'],
  ['tax.negative_taxable_amount', 'tax request is invalid', 'taxable amount validation'],
  ['tax.validation', 'tax request is invalid', 'owner provider validation'],
  ['tax.negative_total', 'tax calculation result is invalid', 'negative result total'],
  ['tax.exempt_customer_charged', 'tax calculation result is invalid', 'exempt result invariant'],
  ['tax.total_overflow', 'tax calculation result is invalid', 'result total overflow'],
  ['tax.total_mismatch', 'tax calculation result is invalid', 'result total mismatch'],
  ['tax.provider_id_invalid', 'tax calculation result is invalid', 'provider identity invariant'],
  ['tax.negative_line', 'tax calculation result is invalid', 'negative result line'],
  ['tax.currency_code_invalid', 'tax calculation result is invalid', 'result currency validation'],
  ['tax.currency_mismatch', 'tax calculation result is invalid', 'result currency mismatch'],
  ['tax.unknown_taxable_target', 'tax calculation result is invalid', 'unknown result target'],
]) {
  requireIn(wrapper, `("${code}", "${message}")`, label);
}

for (const [value, label] of [
  ['currency_code =', 'raw currency value'],
  ['provider_id =', 'raw provider identity'],
  ['channel_provider_id =', 'raw channel provider identity'],
  ['country_code =', 'raw country identity'],
  ['tax_rate =', 'raw tax rate'],
  ['amount =', 'raw monetary amount'],
  ['item_tax_class =', 'raw item tax class'],
  ['shipping_tax_class =', 'raw shipping tax class'],
  ['description =', 'raw description'],
  ['metadata =', 'raw provider metadata'],
]) {
  forbidIn(wrapper, value, label);
}

for (const [value, label] of [
  ['pub fn in_process_tax_calculation_port() -> Arc<dyn TaxCalculationPort>', 'legacy module factory'],
  ['require_tax_calculation_policy(&context, owner_operation)?;', 'unchanged policy admission'],
  ['PortError::validation(code, "tax request is invalid")', 'stable request envelope'],
  ['PortError::invariant_violation(code, "tax calculation result is invalid")', 'stable result envelope'],
  ['PortError::validation("tax.validation", "tax request is invalid")', 'stable owner envelope'],
]) {
  requireIn(owner, value, label);
}

for (const [value, label] of [
  ['Status: **source-ready / unvalidated**', 'unvalidated evidence status'],
  ['Direct callers that deliberately construct through `rustok_tax::ports`', 'legacy bypass disclosure'],
  ['does not record raw currency, provider ids, country codes, tax rates, monetary amounts', 'payload privacy contract'],
  ['No architecture status is promoted from source-only evidence.', 'promotion boundary'],
]) {
  requireIn(evidence, value, label);
}

if (failures.length > 0) {
  console.error('Tax calculation local context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Canonical tax calculation retains safe local outcome context and returns the same PortError',
);
