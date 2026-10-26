#!/usr/bin/env node

// Cart checkout local error context.
//
// "Local" failures are the ones the port itself detects: an invalid prepare input, an unparsable or
// non-checkout cart status, and a snapshot projection that cannot be built. Each one must be
// reported with the exact owner operation, the exact local operation, and the bounded context and
// error facts; the public `PortError` the caller receives must not change.
//
// The verifier used to require the raw mapper body (`error = ?error`, `internal_message = %error.message`,
// and the full `?context.*` field list). That body no longer exists: the mapper delegates to the one
// bounded diagnostic implementation. The guard now asserts the delegation, the preserved public
// error, and the bounded facts.

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging, requireFactFields } from "./lib/bounded-diagnostics.mjs";
import {
  CART_CONTEXT_FACT_FIELDS,
  CART_PORT_ERROR_LOG_FIELDS,
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

const { source, portImpl, prepare, readSnapshot, complete, release, localMapper } = cart;

// Prepare keeps its lifecycle order: validate input, parse status, admit the status, project the
// snapshot — each step mapped through the local mapper with its own operation label.
for (const [value, label] of [
  ["let prepare_input_result = (|| {", "prepare validation result capture"],
  [
    "validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;",
    "stable prepare validation mapping",
  ],
  ["prepare_input_result.map_err(|error| {", "prepare validation interception"],
  ['"validate_prepare_input",', "prepare validation context"],
  ["CartStatus::parse(cart.status.as_str()).ok_or_else(|| {", "prepare status parser"],
  ['"parse_cart_status",', "prepare status context"],
  ['"cart.invalid_status"', "stable invalid-status code"],
  ['format!("invalid cart status `{}`", cart.status)', "stable invalid-status message"],
  ['"require_checkout_status",', "checkout status rejection context"],
  ['"cart.checkout_status_conflict"', "stable checkout conflict code"],
  [
    'format!("cart cannot enter checkout from `{}`", status.as_str())',
    "stable checkout conflict message",
  ],
  ["snapshot_from_cart(cart).map_err(|error| {", "prepare snapshot interception"],
  ['"snapshot_from_cart",', "prepare snapshot context"],
]) requireText(prepare, value, label);

for (const [block, operation, label] of [
  [readSnapshot, "READ_CHECKOUT_SNAPSHOT_OPERATION", "read snapshot context"],
  [complete, "COMPLETE_CHECKOUT_OPERATION", "complete snapshot context"],
  [release, "RELEASE_CHECKOUT_OPERATION", "release snapshot context"],
]) {
  requireText(block, '"snapshot_from_cart",', `${label} local operation`);
  requireText(block, operation, `${label} owner operation`);
  requireText(block, "map_cart_checkout_local_port_error(", `${label} local mapper`);
}
requireText(readSnapshot, ".and_then(snapshot_from_cart);", "read snapshot projection preserved");
requireText(
  complete,
  "merge_checkout_order_metadata(cart.metadata, request.order_id)",
  "completion metadata preserved",
);

// The mapper keeps the `PortError` value and hands a reference to the shared diagnostic path; it
// does not build its own field list, so there is exactly one place where diagnostics can drift.
for (const [value, label] of [
  ["fn map_cart_checkout_local_port_error(", "local mapper definition"],
  ["context: &PortContext", "retained context input"],
  ["owner_operation: &'static str", "exact owner operation input"],
  ["local_operation: &'static str", "exact local operation input"],
  ["error: PortError", "mapped port error input"],
  [
    "log_cart_checkout_port_error(context, owner_operation, local_operation, &error);",
    "shared bounded diagnostic delegation",
  ],
  ["error\n}", "same mapped error returned"],
]) requireText(localMapper, value, label);

forbidRawContextLogging(failures, localMapper, "local error mapper");
forbidRawContextLogging(failures, portImpl, "cart checkout port implementation");

const diagnosticIndex = localMapper.indexOf("log_cart_checkout_port_error(");
const returnIndex = localMapper.lastIndexOf("error\n}");
if (!(diagnosticIndex >= 0 && diagnosticIndex < returnIndex)) {
  failures.push("local mapper: diagnostics must precede returning the same PortError");
}

// The pre-hardening prepare path returned a conflict before the snapshot work and logged the raw
// error; neither shape may come back.
for (const value of [
  "validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;\n\n        let cart",
  'return Err(PortError::conflict(\n                    "cart.checkout_status_conflict"',
]) forbidText(prepare, value, "context-dropping local prepare error");

for (const [pattern, expected, label] of [
  [/map_cart_checkout_local_port_error\(/g, 12, "local mapper use count inside the port"],
  [/"validate_prepare_input"/g, 1, "prepare validation operation count"],
  [/"parse_cart_status"/g, 1, "status parsing operation count"],
  [/"require_checkout_status"/g, 1, "status admission operation count"],
  [/"snapshot_from_cart"/g, 4, "snapshot projection operation count"],
]) {
  const scope = pattern.source.includes("map_cart_checkout_local_port_error") ? portImpl : source;
  const count = scope.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

// The bounded diagnostics themselves are shared with admission; both facts groups are required here
// so a regression in the shared helper still fails the local-error contract.
requireFactFields(failures, cart.portErrorLog, CART_CONTEXT_FACT_FIELDS, "local error context facts");
requireFactFields(failures, cart.portErrorLog, CART_PORT_ERROR_LOG_FIELDS, "local error error facts");

for (const [value, label] of [
  ["fn require_cart_checkout_read_admission(", "read admission helper"],
  ["fn require_cart_checkout_write_admission(", "write admission helper"],
  ["fn log_cart_checkout_admission_rejection(", "admission diagnostics"],
  ["fn map_cart_checkout_service_error(", "service error diagnostics"],
  ["fn parse_tenant_id(", "operation-aware tenant parser"],
  ["fn snapshot_from_cart(", "snapshot projection"],
  ["fn cart_snapshot_hash(", "snapshot hash"],
  ["fn projection_hash(", "projection hash"],
  ["fn normalize_snapshot_value(", "snapshot normalization"],
  ["fn canonicalize_json(", "canonical JSON"],
  ["fn merge_checkout_order_metadata(", "checkout metadata merge"],
  ["fn cart_error_to_port_error(", "stable public cart mapper"],
  ["fn canonical_json_is_independent_of_object_key_order()", "canonical hash test source"],
  [
    "fn snapshot_normalization_removes_volatile_projection_fields()",
    "snapshot normalization test source",
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error("Cart checkout local error context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Cart checkout local validation, lifecycle, and snapshot failures keep the exact owner and local operations, the unchanged PortError, and the shared bounded diagnostics",
);
