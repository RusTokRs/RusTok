#!/usr/bin/env node

// Cart checkout service error context.
//
// Every `CartService` call the port makes is wrapped so the failure is attributed to the exact owner
// operation and the exact service operation, and classified before it is mapped: retryable storage
// and boundary failures are technical (`error`), validation and lifecycle failures are ordinary
// (`warn`). The public `PortError` is produced by one mapper, unchanged from the pre-diagnostic
// behaviour.
//
// The verifier used to require the raw interpolation form (`error = ?error`,
// `tenant_id = %context.tenant_id`, `internal_message = %error.message`) and the older
// `CartError::Database(error)` binding. The bounded source fails both. It now forbids the raw form
// and requires the bounded facts and the `CartError::Database(_)` classification.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging, requireFactFields } from "./lib/bounded-diagnostics.mjs";
import {
  CART_CONTEXT_FACT_FIELDS,
  CART_SERVICE_ERROR_FACT_FIELDS,
  createCartCheckoutSource,
} from "./lib/cart-checkout-source.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const failures = [];

const cart = createCartCheckoutSource({
  read: (relativePath) => readFileSync(new URL(relativePath, root), "utf8"),
  failures,
  label: "cart checkout source",
});

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const { source, portImpl, prepare, readSnapshot, complete, release, serviceMapper, publicErrorMapper } =
  cart;

for (const [block, values, label] of [
  [
    prepare,
    [
      ".get_cart(tenant_id, request.cart_id)",
      'PREPARE_CHECKOUT_OPERATION,\n                    "get_cart",',
      ".begin_checkout(tenant_id, request.cart_id)",
      'PREPARE_CHECKOUT_OPERATION,\n                            "begin_checkout",',
      ".update_context(tenant_id, request.cart_id, request.input)",
      'PREPARE_CHECKOUT_OPERATION,\n                    "update_context",',
    ],
    "prepare service mappings",
  ],
  [
    readSnapshot,
    [
      ".get_cart(tenant_id, cart_id)",
      'READ_CHECKOUT_SNAPSHOT_OPERATION,\n                    "get_cart",',
      ".and_then(snapshot_from_cart)",
    ],
    "read service mapping",
  ],
  [
    complete,
    [
      ".complete_cart(tenant_id, request.cart_id)",
      'COMPLETE_CHECKOUT_OPERATION,\n                    "complete_cart",',
      "merge_checkout_order_metadata(cart.metadata, request.order_id)",
    ],
    "complete service mapping",
  ],
  [
    release,
    [
      ".abandon_cart(tenant_id, cart_id)",
      'RELEASE_CHECKOUT_OPERATION,\n                    "abandon_cart",',
      "snapshot_from_cart(cart)",
    ],
    "release service mapping",
  ],
]) {
  for (const value of values) requireText(block, value, label);
}

// Each call site must route through the service mapper; a direct public mapping would lose the
// owner operation and the severity classification.
for (const value of [
  ".get_cart(tenant_id, request.cart_id)\n            .await\n            .map_err(cart_error_to_port_error)",
  ".begin_checkout(tenant_id, request.cart_id)\n                    .await\n                    .map_err(cart_error_to_port_error)",
  ".update_context(tenant_id, request.cart_id, request.input)\n            .await\n            .map_err(cart_error_to_port_error)",
  ".get_cart(tenant_id, cart_id)\n            .await\n            .map_err(cart_error_to_port_error)",
  ".complete_cart(tenant_id, request.cart_id)\n            .await\n            .map_err(cart_error_to_port_error)",
  ".abandon_cart(tenant_id, cart_id)\n            .await\n            .map_err(cart_error_to_port_error)",
]) forbidText(portImpl, value, "context-dropping cart service mapping");

// Classification happens before the diagnostics, so the event severity is selected from the
// public outcome rather than guessed from the message.
for (const [value, label] of [
  ["fn map_cart_checkout_service_error(", "service error mapper"],
  ["context: &PortContext", "retained context input"],
  ["owner_operation: &'static str", "exact owner operation input"],
  ["service_operation: &'static str", "exact service operation input"],
  ["error: CartError", "original typed cart error input"],
  [
    "let (public_code, public_retryable, technical) = match &error {",
    "public classification before diagnostics",
  ],
  ['CartError::Validation(_) => ("cart.checkout_validation", false, false)', "validation public outcome"],
  ['CartError::CartNotFound(_) => ("cart.not_found", false, false)', "cart not-found public outcome"],
  [
    'CartError::CartLineItemNotFound(_) => ("cart.line_item_not_found", false, false)',
    "line-item not-found public outcome",
  ],
  [
    'CartError::InvalidTransition { .. } => ("cart.checkout_status_conflict", false, false)',
    "transition public outcome",
  ],
  ['CartError::Database(_) => ("cart.database_unavailable", true, true)', "database public outcome"],
  ["CartError::TaxBoundary {", "tax-boundary public outcome"],
  ["code.as_str()", "tax-boundary public code"],
  ["*retryable", "tax-boundary public retryability"],
  [
    "PortErrorKind::Unavailable\n                    | PortErrorKind::Timeout\n                    | PortErrorKind::InvariantViolation",
    "technical tax-boundary classification",
  ],
  ["log_cart_checkout_service_error(", "bounded service diagnostics"],
  ["cart_error_to_port_error(error)", "unchanged public mapper delegation"],
]) requireText(serviceMapper, value, label);

const logIndex = serviceMapper.indexOf("log_cart_checkout_service_error(");
const publicMapIndex = serviceMapper.lastIndexOf("cart_error_to_port_error(error)");
if (!(logIndex >= 0 && logIndex < publicMapIndex)) {
  failures.push("service mapper: diagnostics must precede unchanged public mapping");
}

for (const [value, label] of [
  ["if technical_failure {", "technical severity selection"],
  ["tracing::error!(", "technical error severity"],
  ["tracing::warn!(", "ordinary warning severity"],
  ["owner = CART_CHECKOUT_OWNER", "truthful owner field"],
  ["owner_operation,", "exact owner operation field"],
  ["service_operation,", "exact service operation field"],
  ["public_code,", "selected public code field"],
  ["public_retryable,", "selected public retryability field"],
  ["boundary = CART_CHECKOUT_BOUNDARY", "cart checkout boundary field"],
  ['"cart checkout owner service operation failed with bounded diagnostics"', "technical event"],
  ['"cart checkout owner service operation was rejected with bounded diagnostics"', "ordinary event"],
]) requireText(cart.serviceErrorLog, value, label);

requireFactFields(failures, cart.serviceErrorLog, CART_CONTEXT_FACT_FIELDS, "service context facts");
requireFactFields(
  failures,
  cart.serviceErrorLog,
  CART_SERVICE_ERROR_FACT_FIELDS,
  "service error facts",
);
requireFactFields(
  failures,
  cart.serviceErrorFacts,
  CART_SERVICE_ERROR_FACT_FIELDS,
  "service error fact projection",
);
forbidRawContextLogging(failures, cart.serviceErrorLog, "service error diagnostics");
forbidRawContextLogging(failures, serviceMapper, "service error mapper");

const technicalIndex = cart.serviceErrorLog.indexOf("tracing::error!(");
const ordinaryIndex = cart.serviceErrorLog.indexOf("tracing::warn!(");
if (!(technicalIndex >= 0 && ordinaryIndex > technicalIndex)) {
  failures.push("service diagnostics: technical severity must precede the ordinary branch");
}

for (const [pattern, expected, label] of [
  [/map_cart_checkout_service_error\(/g, 7, "service mapper definition/use count"],
  [/"get_cart"/g, 2, "get-cart service operation count"],
  [/"begin_checkout"/g, 1, "begin-checkout service operation count"],
  [/"update_context"/g, 1, "update-context service operation count"],
  [/"complete_cart"/g, 1, "complete-cart service operation count"],
  [/"abandon_cart"/g, 1, "abandon-cart service operation count"],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

const directPortMappings = portImpl.match(/\.map_err\(cart_error_to_port_error\)/g)?.length ?? 0;
if (directPortMappings !== 1) {
  failures.push(
    `local prepare-input mapping count: expected 1 direct public mapper use, found ${directPortMappings}`,
  );
}
requireText(
  prepare,
  "validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;",
  "local prepare-input mapping preserved",
);

// The public envelope is unchanged, and the two branches that carry owner text emit their own
// bounded event instead of the text.
for (const [value, label] of [
  ["CartError::Validation(message)", "validation mapping"],
  ['"cart.checkout_validation"', "stable validation code"],
  ['"cart checkout request or projection is invalid"', "stable validation message"],
  ["CartError::CartNotFound(_)", "cart not-found mapping"],
  ['"cart.not_found"', "stable cart not-found code"],
  ['"cart was not found"', "stable cart not-found message"],
  ["CartError::CartLineItemNotFound(_)", "line-item not-found mapping"],
  ['"cart.line_item_not_found"', "stable line-item code"],
  ['"cart line item was not found"', "stable line-item message"],
  ["CartError::InvalidTransition { .. }", "transition mapping"],
  ['"cart.checkout_status_conflict"', "stable transition code"],
  ['"cart status transition conflicts with checkout lifecycle"', "stable transition message"],
  ["CartError::Database(_)", "database mapping"],
  ['"cart.database_unavailable"', "stable database code"],
  ['"cart storage is temporarily unavailable"', "stable database message"],
  ["CartError::TaxBoundary {", "tax boundary mapping"],
  ["PortError::new(kind, code, message, retryable)", "tax boundary propagation"],
  ["validation_message_length = message.chars().count()", "validation text shape"],
  ["error_variant = \"database\"", "database failure label"],
]) requireText(publicErrorMapper, value, label);

forbidRawContextLogging(failures, publicErrorMapper, "public error mapper");
forbidText(publicErrorMapper, "CartError::Database(error)", "raw database error binding");
forbidText(publicErrorMapper, "CartError::Validation(message) => {\n            tracing::warn!(\n                error = ?message", "raw validation text");

for (const [value, label] of [
  ["fn require_cart_checkout_read_admission(", "read admission helper"],
  ["fn require_cart_checkout_write_admission(", "write admission helper"],
  ["fn log_cart_checkout_admission_rejection(", "admission diagnostics"],
  ["fn parse_tenant_id(", "operation-aware tenant parser"],
  ["fn snapshot_from_cart(", "snapshot projection"],
  [".map_err(cart_error_to_port_error)?;", "local snapshot mapping"],
  ["fn cart_snapshot_hash(", "snapshot hash"],
  ["fn projection_hash(", "projection hash"],
  ["fn normalize_snapshot_value(", "snapshot normalization"],
  ["fn canonicalize_json(", "canonical JSON"],
  ["fn merge_checkout_order_metadata(", "checkout metadata merge"],
  ["fn canonical_json_is_independent_of_object_key_order()", "canonical hash test source"],
  [
    "fn snapshot_normalization_removes_volatile_projection_fields()",
    "snapshot normalization test source",
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error("Cart checkout service error context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Cart checkout service failures keep the exact owner and service operations, the original CartError, stable public outcomes, and technical-versus-ordinary severity",
);
