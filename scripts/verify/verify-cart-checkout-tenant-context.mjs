#!/usr/bin/env node

// Cart checkout owner tenant context.
//
// The tenant UUID parse is the first place a caller-supplied `PortContext` can be rejected inside
// the port, so the diagnostics there are the template for the rest of the boundary: the exact owner
// operation, the validation phase, and the whole bounded context projection — never the tenant
// value, the actor payload, or the mapped error text.
//
// This verifier used to require the raw form (`tenant_id = %context.tenant_id`, `actor = ?context.actor`,
// `internal_message = %error.message`). That pinned the pre-hardening implementation: the bounded
// source failed the guard and the leaking source passed it. It now forbids the raw form and requires
// the facts.

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

const { source, parser, portImpl } = cart;

// Every operation parses the tenant only after admission has passed, and the parser is told which
// canonical operation it is serving.
for (const [block, operation, label] of [
  [cart.prepare, "PREPARE_CHECKOUT_OPERATION", "prepare tenant context"],
  [cart.readSnapshot, "READ_CHECKOUT_SNAPSHOT_OPERATION", "read tenant context"],
  [cart.complete, "COMPLETE_CHECKOUT_OPERATION", "complete tenant context"],
  [cart.release, "RELEASE_CHECKOUT_OPERATION", "release tenant context"],
]) {
  const admissionIndex = block.indexOf("require_cart_checkout_");
  const parserCall = `let tenant_id = parse_tenant_id(&context, ${operation})?;`;
  const parserIndex = block.indexOf(parserCall);
  requireText(block, parserCall, label);
  if (!(admissionIndex >= 0 && admissionIndex < parserIndex)) {
    failures.push(`${label}: admission must precede operation-aware tenant parsing`);
  }
  if (block.includes("parse_tenant_id(&context)?;")) {
    failures.push(`${label}: tenant parsing must carry the exact owner operation`);
  }
}

// The parser itself: signature, stable public envelope, bounded diagnostics, same error returned.
for (const [value, label] of [
  ["fn parse_tenant_id(", "tenant parser"],
  ["context: &PortContext", "retained context input"],
  ["owner_operation: &'static str", "exact owner operation input"],
  ["Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {", "tenant parse call"],
  ["let error = PortError::validation(", "stable validation construction"],
  ['"cart.tenant_id_invalid"', "stable tenant code"],
  ['"PortContext.tenant_id must be a UUID for cart checkout"', "stable tenant message"],
  ['"cart.tenant_id_nil"', "stable nil tenant code"],
  ['"PortContext.tenant_id must not be a nil UUID for cart checkout"', "stable nil tenant message"],
  ["tracing::warn!(", "validation warning severity"],
  ["owner = CART_CHECKOUT_OWNER", "truthful owner"],
  ["owner_operation,", "exact owner operation"],
  ['validation_phase = "tenant_id"', "tenant validation phase"],
  ['validation_phase = "tenant_id_nil"', "nil tenant validation phase"],
  ["parse_failed = true", "parse failure label"],
  ["boundary = CART_CHECKOUT_BOUNDARY", "cart checkout boundary"],
  ['"cart checkout owner tenant context was rejected with bounded diagnostics"', "tenant rejection event"],
  ["return Err(error);", "same error returned on the nil branch"],
  ["Ok(tenant_id)\n}", "parsed tenant returned"],
]) requireText(parser, value, label);

requireFactFields(failures, parser, CART_CONTEXT_FACT_FIELDS, "tenant parser context facts");
requireFactFields(failures, parser, CART_PORT_ERROR_LOG_FIELDS, "tenant parser error facts");
forbidRawContextLogging(failures, parser, "tenant parser");

const firstWarning = parser.indexOf("tracing::warn!(");
const firstReturn = parser.indexOf("error\n    })?;");
if (!(firstWarning >= 0 && firstWarning < firstReturn)) {
  failures.push("tenant parser: diagnostics must precede returning the stable validation error");
}

// The pre-hardening parser dropped the context and logged the raw value; both shapes stay banned.
for (const [value, label] of [
  ["fn parse_tenant_id(context: &PortContext)", "context-dropping tenant parser"],
  ["let tenant_id = parse_tenant_id(&context)?;", "operation-blind tenant parsing"],
  ["Uuid::parse_str(context.tenant_id.as_str()).map_err(|cause| {", "raw UUID cause diagnostics"],
  ["Tenant context is invalid", "untyped tenant rejection"],
]) forbidText(source, value, label);

for (const [pattern, expected, label] of [
  [/validation_phase = "tenant_id"/g, 1, "tenant validation phase count"],
  [/validation_phase = "tenant_id_nil"/g, 1, "nil tenant validation phase count"],
  [/"cart\.tenant_id_invalid"/g, 1, "stable tenant code count"],
  [/"cart\.tenant_id_nil"/g, 1, "stable nil tenant code count"],
  [
    /"cart checkout owner tenant context was rejected with bounded diagnostics"/g,
    2,
    "tenant rejection event count",
  ],
]) {
  const count = parser.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ["fn log_cart_checkout_operation_start(", "bounded operation start diagnostics"],
  ["fn require_cart_checkout_read_admission(", "read admission helper"],
  ["fn require_cart_checkout_write_admission(", "write admission helper"],
  ["fn log_cart_checkout_admission_rejection(", "admission diagnostics"],
  ["fn snapshot_from_cart(", "snapshot projection"],
  ["fn cart_snapshot_hash(", "snapshot hash"],
  ["fn projection_hash(", "projection hash"],
  ["fn normalize_snapshot_value(", "snapshot normalization"],
  ["fn canonicalize_json(", "canonical JSON"],
  ["fn merge_checkout_order_metadata(", "checkout metadata merge"],
  ["fn cart_error_to_port_error(", "stable public cart mapper"],
]) requireText(source, value, label);

// Each operation now opens with the bounded start event instead of interpolating the tenant and
// cart ids, so the port implementation carries no raw directive at all.
forbidRawContextLogging(failures, portImpl, "cart checkout port implementation");
requireText(
  portImpl,
  "log_cart_checkout_operation_start(",
  "bounded operation start at the port boundary",
);
requireFactFields(failures, cart.operationStart, ["cart_id_non_nil"], "operation start cart identity");
requireFactFields(
  failures,
  cart.operationStart,
  ["order_id_present", "order_id_non_nil"],
  "operation start order identity",
);

if (failures.length > 0) {
  console.error("Cart checkout tenant context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Cart checkout tenant validation keeps the stable envelope, warning severity, exact owner operation, and the full bounded context projection",
);
