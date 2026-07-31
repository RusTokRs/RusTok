#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  owner: 'crates/rustok-fulfillment/src/fulfillment_read.rs',
  error: 'crates/rustok-fulfillment/src/error.rs',
  document:
    'crates/rustok-fulfillment/docs/fulfillment-lifecycle-read-diagnostic-safety.md',
  evidence:
    'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-diagnostic-safety-source.json',
  review:
    'crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-diagnostic-safety-source-review.json',
};

const owner = read(paths.owner);
const errorSource = read(paths.error);
const document = read(paths.document);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function functionBody(source, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return '';
  }
  const openBrace = source.indexOf('{', match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return '';
}

function functionBodyAfter(source, anchor, functionName) {
  const anchorIndex = source.indexOf(anchor);
  if (anchorIndex < 0) {
    failures.push(`missing anchor ${anchor}`);
    return '';
  }
  return functionBody(source.slice(anchorIndex), functionName);
}

for (const marker of [
  'pub trait FulfillmentReadPort: Send + Sync',
  'ReadFulfillmentProjectionRequest',
  'ListFulfillmentProjectionsRequest',
  'FulfillmentProjectionPage',
  'FindLatestFulfillmentByOrderProjectionRequest',
  'pub fn in_process_fulfillment_read_port(',
  'impl FulfillmentReadPort for InProcessFulfillmentReadPort',
]) requireText(owner, marker, `${paths.owner}: public surface`);

const implementationAnchor =
  'impl FulfillmentReadPort for InProcessFulfillmentReadPort';
const readBody = functionBodyAfter(
  owner,
  implementationAnchor,
  'read_fulfillment_projection',
);
const listBody = functionBodyAfter(
  owner,
  implementationAnchor,
  'list_fulfillment_projections',
);
const latestBody = functionBodyAfter(
  owner,
  implementationAnchor,
  'find_latest_fulfillment_by_order_projection',
);

for (const marker of [
  'context.require_policy(PortCallPolicy::read())?',
  'parse_tenant_id(&context, "read_fulfillment_projection")?',
  '.get_fulfillment(tenant_id, request.fulfillment_id)',
  'Some(request.fulfillment_id)',
]) requireText(readBody, marker, `${paths.owner}: lookup flow`);
for (const marker of [
  'context.require_policy(PortCallPolicy::read())?',
  'parse_tenant_id(&context, "list_fulfillment_projections")?',
  'let status_length = request.status.as_deref().map(str::len);',
  'ListFulfillmentsInput {',
  'page: request.page',
  'per_page: request.per_page',
  'status: request.status',
  'order_id,',
  'customer_id,',
  'Ok(FulfillmentProjectionPage { items, total })',
]) requireText(listBody, marker, `${paths.owner}: list flow`);
for (const marker of [
  'context.require_policy(PortCallPolicy::read())?',
  'parse_tenant_id(&context, "find_latest_fulfillment_by_order_projection")?',
  '.find_by_order(tenant_id, request.order_id)',
  'Some(request.order_id)',
]) requireText(latestBody, marker, `${paths.owner}: latest flow`);

for (const [body, delegation, label] of [
  [readBody, '.get_fulfillment(', 'lookup'],
  [listBody, '.list_fulfillments(', 'list'],
  [latestBody, '.find_by_order(', 'latest'],
]) {
  const policy = body.indexOf('context.require_policy(PortCallPolicy::read())?');
  const parse = body.indexOf('parse_tenant_id(');
  const ownerCall = body.indexOf(delegation);
  if (!(policy >= 0 && policy < parse && parse < ownerCall)) {
    failures.push(`${paths.owner}: ${label} admission/delegation order changed`);
  }
}

for (const marker of [
  'struct FulfillmentLifecycleReadContextFacts',
  'struct FulfillmentLifecycleOwnerErrorFacts',
  'struct FulfillmentLifecycleReadRequestFacts',
  'fn fulfillment_lifecycle_read_context_facts(',
  'fn fulfillment_lifecycle_owner_error_facts(',
  'fn fulfillment_lifecycle_read_request_facts(',
  'FulfillmentError::Validation(value) =>',
  'FulfillmentError::ShippingOptionNotFound(id) =>',
  'FulfillmentError::FulfillmentNotFound(id) =>',
  'FulfillmentError::InvalidTransition { from, to } =>',
  'FulfillmentError::Database(_) => ("database", 0, 0, 0, 0, true)',
]) requireText(owner, marker, `${paths.owner}: bounded facts`);

for (const marker of [
  'Validation(String)',
  'ShippingOptionNotFound(Uuid)',
  'FulfillmentNotFound(Uuid)',
  'InvalidTransition { from: String, to: String }',
  'Database(#[from] DbErr)',
]) requireText(errorSource, marker, `${paths.error}: retained error shape`);

const parser = functionBody(owner, 'parse_tenant_id');
for (const marker of [
  'map_err(|_|',
  'tenant_id_parse_failed = true',
  'tenant_id_length = context_facts.tenant_id_length',
  'actor_kind = context_facts.actor_kind',
  'claim_count = context_facts.claim_count',
  'role_count = context_facts.role_count',
  'boundary = FULFILLMENT_LIFECYCLE_READ_BOUNDARY',
  '"fulfillment.context_invalid"',
]) requireText(parser, marker, `${paths.owner}: bounded tenant parser`);
for (const forbidden of [
  '|error|',
  'error = ?error',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
]) forbidText(parser, forbidden, `${paths.owner}: parser payload`);

const mapper = functionBody(owner, 'map_owner_error');
for (const marker of [
  'let error_facts = fulfillment_lifecycle_owner_error_facts(&error);',
  'let request_facts = fulfillment_lifecycle_read_request_facts(',
  'let (kind, code, message, retryable, technical_failure) = match &error',
  'tracing::error!(',
  'tracing::warn!(',
  'fulfillment_id_present = request_facts.fulfillment_id_present',
  'fulfillment_id_non_nil = request_facts.fulfillment_id_non_nil',
  'order_id_present = request_facts.order_id_present',
  'order_id_non_nil = request_facts.order_id_non_nil',
  'customer_id_present = request_facts.customer_id_present',
  'customer_id_non_nil = request_facts.customer_id_non_nil',
  'status_present = request_facts.status_present',
  'status_length = ?request_facts.status_length',
  'error_variant = error_facts.error_variant',
  'text_total_length = error_facts.text_total_length',
  'uuid_non_nil_count = error_facts.uuid_non_nil_count',
  'opaque_payload_present = error_facts.opaque_payload_present',
  'PortError::new(kind, code, message, retryable)',
]) requireText(mapper, marker, `${paths.owner}: bounded owner mapper`);
for (const forbidden of [
  'error = ?error',
  'error = %message',
  'tenant_id = %context.tenant_id',
  'actor = ?context.actor',
  'fulfillment_id = ?fulfillment_id',
  'order_id = ?order_id',
  'customer_id = ?customer_id',
  'from = %from',
  'to = %to',
]) forbidText(mapper, forbidden, `${paths.owner}: owner payload`);

for (const marker of [
  '"fulfillment.validation"',
  '"fulfillment.shipping_option_not_found"',
  '"fulfillment.fulfillment_not_found"',
  '"fulfillment.invalid_transition"',
  '"fulfillment.database_unavailable"',
  '"fulfillment request is invalid"',
  '"shipping option was not found"',
  '"fulfillment was not found"',
  '"fulfillment lifecycle transition conflicts with the current state"',
  '"fulfillment storage is temporarily unavailable"',
]) requireText(mapper, marker, `${paths.owner}: stable public envelope`);

for (const [key, expected] of Object.entries({
  complete_fulfillment_error_logged: false,
  database_error_payload_logged: false,
  uuid_parser_payload_logged: false,
  validation_text_logged: false,
  transition_text_logged: false,
  resource_uuid_logged: false,
  raw_context_values_logged: false,
  static_error_variant_logged: true,
  aggregate_error_shape_logged: true,
  safe_context_shape_logged: true,
  request_identity_shape_logged: true,
  status_shape_logged: true,
  tenant_parse_failure_logged: true,
  database_severity_changed: false,
  ordinary_severity_changed: false,
  read_policy_order_changed: false,
  tenant_parse_order_changed: false,
  owner_delegation_changed: false,
  pagination_filter_semantics_changed: false,
  optional_latest_semantics_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  broad_ecommerce_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
if (evidence.validation?.compile_proven !== false) {
  failures.push(`${paths.evidence}: compile_proven must remain false`);
}
for (const [key, expected] of Object.entries({
  public_trait_preserved: true,
  request_response_dtos_preserved: true,
  canonical_factory_preserved: true,
  read_admission_order_preserved: true,
  owner_delegation_preserved: true,
  pagination_filter_semantics_preserved: true,
  optional_latest_semantics_preserved: true,
  all_public_port_errors_preserved: true,
  complete_fulfillment_error_logging_removed: true,
  database_error_payload_removed: true,
  uuid_parser_payload_removed: true,
  raw_context_values_removed: true,
  resource_uuid_removed: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  'Status: **source-ready / unvalidated**',
  'single fulfillment lookup',
  'filtered and paginated fulfillment list',
  'optional latest fulfillment by order',
  'three currently identified Fulfillment owner diagnostic slices',
  'broader ecommerce cleanup remain open',
]) requireText(document, marker, `${paths.document}: truthful status`);

if (failures.length > 0) {
  console.error('Fulfillment lifecycle read diagnostic-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ fulfillment lifecycle reads preserve admission, pagination/filtering, owner delegation, optional latest semantics, and public PortError behavior while retaining only bounded context, request, and owner-error facts',
);
