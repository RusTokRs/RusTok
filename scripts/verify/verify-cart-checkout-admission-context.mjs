#!/usr/bin/env node

// Cart checkout owner admission.
//
// Admission is the first thing every cart checkout operation does, so it is also the first place an
// unauthorised caller is observed. The diagnostics must therefore report the rejection against the
// exact canonical operation and the admission phase, with the bounded context projection and the
// original error's shape — and must never interpolate the context values or the error text.
//
// The verifier used to require `error = ?error`, `tenant_id = %context.tenant_id`, and the literal
// `internal_message = %error.message`. Those are exactly the fields the hardening removed, so the
// guard is now written the other way round: it forbids the raw form and requires the bounded facts.

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

const { source, admission, portImpl, portErrorLog } = cart;

for (const [value, label] of [
  ["use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};", "typed admission imports"],
  ['const CART_CHECKOUT_OWNER: &str = "rustok_cart";', "truthful cart owner"],
  ['const CART_CHECKOUT_BOUNDARY: &str = "cart_checkout_port";', "stable cart checkout boundary"],
  ['const PREPARE_CHECKOUT_OPERATION: &str = "prepare_checkout";', "prepare operation"],
  ['const READ_CHECKOUT_SNAPSHOT_OPERATION: &str = "read_checkout_snapshot";', "read snapshot operation"],
  ['const COMPLETE_CHECKOUT_OPERATION: &str = "complete_checkout";', "complete operation"],
  ['const RELEASE_CHECKOUT_OPERATION: &str = "release_checkout";', "release operation"],
]) requireText(cart.constants, value, label);

// The helper intercepts the policy error without consuming it: `inspect_err` keeps the original
// `PortError` available both to the diagnostics and to the caller's return value.
for (const [value, label] of [
  ["fn require_cart_checkout_read_admission(", "read admission helper"],
  [".require_policy(PortCallPolicy::read())", "read policy interception"],
  ["inspect_err(|error| {", "non-consuming error interception"],
  [
    'log_cart_checkout_admission_rejection(context, owner_operation, "policy", error);',
    "read policy diagnostics",
  ],
  ["fn require_cart_checkout_write_admission(", "write admission helper"],
  [".require_policy(PortCallPolicy::write())", "write policy interception"],
  ["context.require_write_semantics()", "write semantics interception"],
  ['"write_semantics",', "write semantics phase"],
  ["fn log_cart_checkout_admission_rejection(", "shared rejection diagnostics"],
  ["context: &PortContext", "retained context input"],
  ["owner_operation: &'static str", "exact owner operation input"],
  ["admission_phase: &'static str", "admission phase input"],
  ["error: &PortError", "original port error input"],
  [
    "log_cart_checkout_port_error(context, owner_operation, admission_phase, error);",
    "single bounded diagnostic implementation",
  ],
]) requireText(admission, value, label);

// Policy rejection must be reported before the write-semantics phase is consulted, so the phase
// label always names the first check that failed.
const readPolicyIndex = admission.indexOf("require_policy(PortCallPolicy::write())");
const writeSemanticsIndex = admission.indexOf("require_write_semantics()");
if (!(readPolicyIndex >= 0 && readPolicyIndex < writeSemanticsIndex)) {
  failures.push("write admission: policy must be checked before write semantics");
}
for (const [phase, expected, label] of [
  ['"policy"', 2, "policy"],
  ['"write_semantics"', 1, "write semantics"],
]) {
  const count = admission.match(new RegExp(phase, "g"))?.length ?? 0;
  if (count !== expected) failures.push(`${label} phase count: expected ${expected}, found ${count}`);
}
for (const value of ["map_err(|error| {", "map_err(|_| {"]) {
  forbidText(admission, value, "consuming admission interception");
}

for (const [block, values, label] of [
  [
    cart.prepare,
    [
      "require_cart_checkout_write_admission(&context, PREPARE_CHECKOUT_OPERATION)?;",
      "let tenant_id = parse_tenant_id(&context, PREPARE_CHECKOUT_OPERATION)?;",
      ".get_cart(tenant_id, request.cart_id)",
      ".begin_checkout(tenant_id, request.cart_id)",
      ".update_context(tenant_id, request.cart_id, request.input)",
      "snapshot_from_cart(cart)",
    ],
    "prepare checkout behavior",
  ],
  [
    cart.readSnapshot,
    [
      "require_cart_checkout_read_admission(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;",
      "let tenant_id = parse_tenant_id(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;",
      ".get_cart(tenant_id, cart_id)",
      ".and_then(snapshot_from_cart)",
    ],
    "read snapshot behavior",
  ],
  [
    cart.complete,
    [
      "require_cart_checkout_write_admission(&context, COMPLETE_CHECKOUT_OPERATION)?;",
      "let tenant_id = parse_tenant_id(&context, COMPLETE_CHECKOUT_OPERATION)?;",
      ".complete_cart(tenant_id, request.cart_id)",
      "merge_checkout_order_metadata(cart.metadata, request.order_id)",
      "snapshot_from_cart(cart)",
    ],
    "complete checkout behavior",
  ],
  [
    cart.release,
    [
      "require_cart_checkout_write_admission(&context, RELEASE_CHECKOUT_OPERATION)?;",
      "let tenant_id = parse_tenant_id(&context, RELEASE_CHECKOUT_OPERATION)?;",
      ".abandon_cart(tenant_id, cart_id)",
      "snapshot_from_cart(cart)",
    ],
    "release checkout behavior",
  ],
]) {
  for (const value of values) requireText(block, value, label);
  const admissionIndex = block.indexOf("require_cart_checkout_");
  const tenantIndex = block.indexOf("let tenant_id = parse_tenant_id(&context,");
  if (!(admissionIndex >= 0 && admissionIndex < tenantIndex)) {
    failures.push(`${label}: admission must precede tenant parsing`);
  }
}

// The bounded mapper is the one diagnostic implementation for every rejection the port observes:
// admission, local validation, and delegation failures alike.
for (const [value, label] of [
  ["fn log_cart_checkout_port_error(", "bounded port error diagnostics"],
  ["context: &PortContext", "retained context input"],
  ["owner_operation: &'static str", "exact owner operation input"],
  ["phase: &'static str", "admission and local phase input"],
  ["error: &PortError", "original port error input"],
  [
    "PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation",
    "technical severity classification",
  ],
  ["if matches!(", "severity selection"],
  ["tracing::error!(", "technical rejection severity"],
  ["tracing::warn!(", "ordinary rejection severity"],
  ["owner = CART_CHECKOUT_OWNER", "truthful owner field"],
  ["owner_operation,", "exact operation field"],
  ["phase,", "phase field"],
  ['"cart checkout owner boundary failed with bounded diagnostics"', "technical event"],
  ['"cart checkout owner boundary was rejected with bounded diagnostics"', "ordinary event"],
]) requireText(portErrorLog, value, label);

for (const [value, label] of [
  "let context_facts = cart_checkout_context_facts(context);",
  "let error_facts = cart_checkout_port_error_facts(error);",
]) requireText(portErrorLog, value, label);

requireFactFields(failures, portErrorLog, CART_CONTEXT_FACT_FIELDS, "admission context facts");
requireFactFields(failures, portErrorLog, CART_PORT_ERROR_LOG_FIELDS, "admission error facts");
forbidRawContextLogging(failures, portErrorLog, "admission diagnostics");
forbidRawContextLogging(failures, admission, "admission helpers");

const technicalIndex = portErrorLog.indexOf("tracing::error!(");
const ordinaryIndex = portErrorLog.indexOf("tracing::warn!(");
if (!(technicalIndex >= 0 && ordinaryIndex > technicalIndex)) {
  failures.push("admission diagnostics: technical severity must precede the ordinary branch");
}

for (const value of [
  "context.require_policy(PortCallPolicy::read())?;",
  "context.require_policy(PortCallPolicy::write())?;",
  "context.require_write_semantics()?;",
]) forbidText(portImpl, value, "context-dropping direct admission");

for (const [pattern, expected, label] of [
  [/require_cart_checkout_read_admission\(/g, 2, "read helper definition/use count"],
  [/require_cart_checkout_write_admission\(/g, 4, "write helper definition/use count"],
  [/log_cart_checkout_admission_rejection\(/g, 4, "diagnostic helper definition/use count"],
]) {
  const count = source.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ["fn snapshot_from_cart(", "snapshot projection"],
  ["fn cart_snapshot_hash(", "snapshot hash"],
  ["fn projection_hash(", "projection hash"],
  ["fn normalize_snapshot_value(", "snapshot normalization"],
  ["fn canonicalize_json(", "canonical JSON"],
  ["fn merge_checkout_order_metadata(", "checkout order metadata merge"],
  ["fn cart_error_to_port_error(", "stable public cart mapping"],
  ["fn canonical_json_is_independent_of_object_key_order()", "canonical hash test source"],
  [
    "fn snapshot_normalization_removes_volatile_projection_fields()",
    "snapshot normalization test source",
  ],
]) requireText(source, value, label);

if (failures.length > 0) {
  console.error("Cart checkout admission context verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Cart checkout owner admission keeps the original PortError, exact operation and phase, the bounded context projection, and technical-versus-ordinary severity",
);
