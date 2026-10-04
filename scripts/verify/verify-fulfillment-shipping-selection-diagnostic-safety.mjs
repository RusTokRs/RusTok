#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const paths = {
  owner: "crates/modules/rustok-fulfillment/src/ports.rs",
  error: "crates/modules/rustok-fulfillment/src/error.rs",
  doc: "crates/modules/rustok-fulfillment/docs/shipping-selection-diagnostic-safety.md",
  plan: "crates/modules/rustok-fulfillment/docs/implementation-plan.md",
  evidence:
    "crates/modules/rustok-fulfillment/contracts/evidence/shipping-selection-diagnostic-safety-source.json",
  nativeClient:
    "crates/modules/rustok-fulfillment/storefront/src/transport/native_server_adapter/native_client_error_safety.rs",
  review:
    "crates/modules/rustok-fulfillment/contracts/evidence/shipping-selection-diagnostic-safety-source-review.json",
};

const owner = read(paths.owner);
const errorSource = read(paths.error);
const doc = read(paths.doc);
const plan = read(paths.plan);
const nativeClient = read(paths.nativeClient);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

function functionBody(content, functionName) {
  const signature = new RegExp(
    `(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}(?:<[^>]*>)?\\s*\\(`,
  );
  const match = signature.exec(content);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = content.indexOf("{", match.index);
  let depth = 0;
  for (let index = openBrace; index >= 0 && index < content.length; index += 1) {
    if (content[index] === "{") depth += 1;
    if (content[index] === "}") {
      depth -= 1;
      if (depth === 0) return content.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function ${functionName}`);
  return "";
}

function functionBodyAfter(content, anchor, functionName) {
  const anchorIndex = content.indexOf(anchor);
  if (anchorIndex < 0) {
    failures.push(`missing anchor ${anchor}`);
    return "";
  }
  return functionBody(content.slice(anchorIndex), functionName);
}

for (const marker of [
  "pub trait ShippingSelectionPort: Send + Sync",
  "async fn list_seller_shipping_options(",
  "async fn select_shipping_option(",
  "ListSellerShippingOptionsRequest",
  "SelectShippingOptionPortRequest",
  "SellerShippingOptionsSnapshot",
  "SelectedShippingOptionSnapshot",
]) requireText(owner, marker, `${paths.owner}: public surface`);

const implementationAnchor =
  "impl ShippingSelectionPort for crate::FulfillmentService";
const listBody = functionBodyAfter(
  owner,
  implementationAnchor,
  "list_seller_shipping_options",
);
for (const marker of [
  "context.require_policy(PortCallPolicy::read())?",
  'parse_port_tenant_id(&context, "list_seller_shipping_options")?',
  ".list_shipping_options(",
  ".filter(|option|",
  ".map(ShippingOptionProjection::from_response)",
]) requireText(listBody, marker, `${paths.owner}: list flow`);
for (const [earlier, later] of [
  ["context.require_policy(PortCallPolicy::read())?", "parse_port_tenant_id("],
  ["parse_port_tenant_id(", ".list_shipping_options("],
]) {
  if (listBody.indexOf(earlier) < 0 || listBody.indexOf(earlier) >= listBody.indexOf(later)) {
    failures.push(`${paths.owner}: list admission/delegation order changed`);
  }
}

const selectBody = functionBodyAfter(
  owner,
  implementationAnchor,
  "select_shipping_option",
);
for (const marker of [
  "if !option.active",
  '"fulfillment.shipping_option_inactive"',
  "PortErrorKind::Conflict",
  '"shipping option is not active"',
]) requireText(selectBody, marker, `${paths.owner}: inactive-option guard`);
for (const marker of [
  'parse_port_tenant_id(&context, "select_shipping_option")?',
  ".get_shipping_option(",
  "ShippingOptionProjection::from_response(option)",
]) requireText(selectBody, marker, `${paths.owner}: select flow`);

const selectWriteAdmission = [
  "context.require_policy(PortCallPolicy::write())?",
  "context.require_write_semantics()?",
].find((marker) => selectBody.includes(marker));
if (!selectWriteAdmission) {
  failures.push(`${paths.owner}: select flow must enforce write idempotency/deadline semantics`);
}
const selectOrder = [
  selectWriteAdmission,
  "parse_port_tenant_id(",
  ".get_shipping_option(",
].map((marker) => marker === undefined ? -1 : selectBody.indexOf(marker));
if (!selectOrder.every((value, index) => value >= 0 && (index === 0 || selectOrder[index - 1] < value))) {
  failures.push(`${paths.owner}: select admission/delegation order changed`);
}

for (const [content, value, label] of [
  [
    nativeClient,
    "return ShippingSelectionTransportError::Validation(message.clone());",
    paths.nativeClient + ": preserve validation messages",
  ],
  [
    nativeClient,
    "ShippingSelectionTransportError::ServerFn(",
    paths.nativeClient + ": stable transport failure envelope",
  ],
  [
    nativeClient,
    "pub(super) fn map_error(",
    paths.nativeClient + ": map_error",
  ],
  [
    nativeClient,
    "error_kind,",
    paths.nativeClient + ": bounded error kind",
  ],
  [
    nativeClient,
    "raw_message_present",
    paths.nativeClient + ": raw message presence shape",
  ],
  [
    nativeClient,
    "raw_message_length",
    paths.nativeClient + ": raw message length",
  ],
  [
    nativeClient,
    "FULFILLMENT_STOREFRONT_NATIVE_CLIENT_PUBLIC_MESSAGE",
    paths.nativeClient + ": stable public message",
  ],
]) requireText(content, value, label);

for (const forbidden of [
  "raw_error = ?error",
  "error = ?error",
  "message = %message",
]) forbidText(nativeClient, forbidden, paths.nativeClient + ": raw transport error payload");

for (const marker of [
  "struct FulfillmentPortContextFacts",
  "struct FulfillmentOwnerErrorFacts",
  "fn fulfillment_port_context_facts(",
  "fn fulfillment_owner_error_facts(",
  'crate::FulfillmentError::Validation(value) =>',
  'crate::FulfillmentError::ShippingOptionNotFound(id) =>',
  'crate::FulfillmentError::FulfillmentNotFound(id) =>',
  'crate::FulfillmentError::InvalidTransition { from, to } =>',
  'crate::FulfillmentError::Database(_) => ("database", 0, 0, 0, 0, true)',
]) requireText(owner, marker, `${paths.owner}: bounded owner facts`);

for (const marker of [
  "Validation(String)",
  "ShippingOptionNotFound(Uuid)",
  "FulfillmentNotFound(Uuid)",
  "InvalidTransition { from: String, to: String }",
  "Database(#[from] DbErr)",
]) requireText(errorSource, marker, `${paths.error}: retained error shape`);

const tenantParser = functionBody(owner, "parse_port_tenant_id");
for (const marker of [
  "map_err(|_|",
  "tenant_id_parse_failed = true",
  "tenant_id_length = context_facts.tenant_id_length",
  "actor_kind = context_facts.actor_kind",
  "claim_count = context_facts.claim_count",
  "role_count = context_facts.role_count",
  "boundary = SHIPPING_SELECTION_BOUNDARY",
]) requireText(tenantParser, marker, `${paths.owner}: bounded tenant parser`);
for (const forbidden of [
  "|error|",
  "error = ?error",
  "tenant_id = %context.tenant_id",
]) forbidText(tenantParser, forbidden, `${paths.owner}: tenant parser payload`);

const mapper = functionBody(owner, "fulfillment_error_to_port_error");
for (const marker of [
  "let error_facts = fulfillment_owner_error_facts(&error);",
  "let (kind, code, message, retryable, technical_failure) = match &error",
  '"fulfillment.validation"',
  '"fulfillment.shipping_option_not_found"',
  '"fulfillment.fulfillment_not_found"',
  '"fulfillment.invalid_transition"',
  '"fulfillment.database_unavailable"',
  "tracing::error!(",
  "tracing::warn!(",
  "error_variant = error_facts.error_variant",
  "text_field_count = error_facts.text_field_count",
  "text_total_length = error_facts.text_total_length",
  "uuid_field_count = error_facts.uuid_field_count",
  "uuid_non_nil_count = error_facts.uuid_non_nil_count",
  "opaque_payload_present = error_facts.opaque_payload_present",
  "PortError::new(kind, code, message, retryable)",
]) requireText(mapper, marker, `${paths.owner}: bounded owner mapper`);
for (const forbidden of [
  "error = ?error",
  "error = %message",
  "resource_id = %id",
  "from = %from",
  "to = %to",
  "tenant_id = %context.tenant_id",
]) forbidText(mapper, forbidden, `${paths.owner}: complete owner payload`);

for (const [key, expected] of Object.entries({
  native_client_diagnostic_redaction_closed: true,
  native_client_raw_error_payload_logged: false,
  native_client_error_shape_logged: true,
  native_client_validation_message_preserved: true,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(paths.evidence + ": source_contract." + key + " must be " + expected);
  }
}

for (const [key, expected] of Object.entries({
  shared_storefront_selection_policy_present: true,
  shared_selection_active_state_enforced: true,
  shared_selection_currency_enforced: true,
  shared_selection_channel_enforced: true,
  shared_selection_profile_compatibility_enforced: true,
  native_selection_uses_host_selected_shipping_read_port: true,
  native_selection_validates_before_cart_mutation: true,
  owner_inactive_selection_rejected: true,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(paths.evidence + ": source_contract." + key + " must be " + expected);
  }
}

for (const [key, expected] of Object.entries({
  complete_fulfillment_error_logged: false,
  database_error_payload_logged: false,
  uuid_parser_payload_logged: false,
  validation_text_logged: false,
  transition_text_logged: false,
  resource_uuid_logged: false,
  raw_tenant_id_logged: false,
  static_error_variant_logged: true,
  error_text_shape_logged: true,
  error_uuid_shape_logged: true,
  opaque_payload_presence_logged: true,
  tenant_parse_failure_logged: true,
  safe_context_shape_logged: true,
  database_severity_changed: false,
  ordinary_severity_changed: false,
  public_code_changed: false,
  public_message_changed: false,
  public_kind_changed: false,
  public_retryability_changed: false,
  shipping_option_read_diagnostic_cleanup_closed: false,
  fulfillment_lifecycle_read_diagnostic_cleanup_closed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}
if (evidence.validation?.compile_proven !== false) {
  failures.push(`${paths.evidence}: compile_proven must remain false`);
}
for (const [key, expected] of Object.entries({
  public_api_preserved: true,
  read_admission_order_preserved: true,
  write_admission_order_preserved: true,
  owner_delegation_preserved: true,
  all_public_port_errors_preserved: true,
  complete_fulfillment_error_logging_removed: true,
  database_error_payload_removed: true,
  uuid_parser_payload_removed: true,
  raw_context_values_removed: true,
  runtime_evidence_claimed: false,
})) {
  if (review.review_findings?.[key] !== expected) {
    failures.push(`${paths.review}: review_findings.${key} must be ${expected}`);
  }
}

for (const marker of [
  "Status: **source-ready / unvalidated**",
  "All five `FulfillmentError` variants are classified through a closed static label",
  "Shipping-option projection read diagnostics in `shipping_option_read.rs`",
  "fulfillment lifecycle read diagnostics in `fulfillment_read.rs`",
]) requireText(doc, marker, `${paths.doc}: truthful source status`);
for (const marker of [
  "The native FFA surface remains seller/cart selection through",
  "Shipping-selection owner payload diagnostics are source-closed / unvalidated",
  "verify-fulfillment-shipping-selection-diagnostic-safety.mjs",
]) requireText(plan, marker, `${paths.plan}: synchronized plan`);

if (failures.length > 0) {
  console.error("Fulfillment shipping-selection diagnostic-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Fulfillment shipping selection preserves admission, filtering, delegation, and public PortError behavior while retaining only bounded context and owner-error facts",
);
