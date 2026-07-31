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

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireCount = (source, value, expected, label) => {
  const count = source.split(value).length - 1;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
};

function functionBody(source, functionName) {
  const match = new RegExp(`(?:pub\\(super\\)\\s+)?(?:pub\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`).exec(source);
  if (!match) {
    failures.push(`missing function ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace < 0) {
    failures.push(`missing body for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated body for ${functionName}`);
  return "";
}

const paths = {
  routing: "crates/rustok-commerce/admin/src/transport/mod.rs",
  facade: "crates/rustok-commerce/admin/src/transport/promotion.rs",
  safety:
    "crates/rustok-commerce/admin/src/transport/promotion_client_error_safety.rs",
  native: "crates/rustok-commerce/admin/src/transport/native_server_adapter.rs",
  nativeSsr:
    "crates/rustok-commerce/admin/src/transport/native_server_adapter_ssr.rs",
  evidence:
    "crates/rustok-commerce/contracts/evidence/admin-promotion-client-transport-error-safety-source.json",
  review:
    "crates/rustok-commerce/contracts/evidence/admin-promotion-client-transport-error-safety-source-review.json",
  doc: "crates/rustok-commerce/docs/admin-promotion-client-transport-error-safety.md",
  commercePlan: "crates/rustok-commerce/docs/implementation-plan.md",
  nativeGuard:
    "scripts/verify/verify-commerce-admin-promotion-native-error-safety.mjs",
};

const routing = read(paths.routing);
const facade = read(paths.facade);
const safety = read(paths.safety);
const native = read(paths.native);
const nativeSsr = read(paths.nativeSsr);
const evidence = JSON.parse(read(paths.evidence));
const review = JSON.parse(read(paths.review));
const doc = read(paths.doc);
const commercePlan = read(paths.commercePlan);
const nativeGuard = read(paths.nativeGuard);

requireText(
  routing,
  "mod promotion_client_error_safety;",
  `${paths.routing}: client error policy wiring`,
);
requireText(
  routing,
  "pub use promotion::{apply_cart_promotion, preview_cart_promotion};",
  `${paths.routing}: promotion exports`,
);

for (const marker of [
  "use super::promotion_client_error_safety::PromotionClientErrorContext;",
  "Result<CommerceCartPromotionPreview, ApiError>",
  "Result<CommerceAdminCartSnapshot, ApiError>",
]) {
  requireText(facade, marker, `${paths.facade}: preserved facade contract`);
}
requireCount(
  facade,
  ".map_err(|promotion_error| context.map_error(promotion_error))",
  2,
  `${paths.facade}: final client mappings`,
);

for (const [operation, constructor, call] of [
  [
    "preview_cart_promotion",
    "PromotionClientErrorContext::for_preview",
    "native_server_adapter::preview_cart_promotion(",
  ],
  [
    "apply_cart_promotion",
    "PromotionClientErrorContext::for_apply",
    "native_server_adapter::apply_cart_promotion(",
  ],
]) {
  const body = functionBody(facade, operation);
  requireText(body, constructor, `${paths.facade}: ${operation} context`);
  requireText(body, call, `${paths.facade}: ${operation} native call`);
  requireText(
    body,
    ".map_err(|promotion_error| context.map_error(promotion_error))",
    `${paths.facade}: ${operation} final mapping`,
  );
  if (body.indexOf(constructor) > body.indexOf(call)) {
    failures.push(`${paths.facade}: ${operation} context must precede native call`);
  }
}

for (const marker of [
  'const COMMERCE_ADMIN_PROMOTION_CLIENT_OWNER: &str =',
  '"rustok_commerce.admin_promotion_transport"',
  'const COMMERCE_ADMIN_PROMOTION_CLIENT_BOUNDARY: &str =',
  '"commerce_admin_promotion_client_transport"',
  '"Commerce admin promotion request could not be completed"',
  "struct PromotionClientErrorFacts",
  "pub(super) struct PromotionClientErrorContext",
  'Self::new("preview_cart_promotion", cart_id)',
  'Self::new("apply_cart_promotion", cart_id)',
  "let error_facts = promotion_client_error_facts(&error);",
  "error_variant = error_facts.error_variant",
  "error_message_present = error_facts.message_present",
  "error_message_length = error_facts.message_length",
  "owner_operation = self.operation",
  "correlation_id = %self.correlation_id",
  "cart_id_present = self.cart_id_length > 0",
  "cart_id_length = self.cart_id_length",
  "payload_present = self.payload_present",
  'code = "commerce.admin_promotion_client_transport_failed"',
  "boundary = COMMERCE_ADMIN_PROMOTION_CLIENT_BOUNDARY",
  "ApiError::ServerFn(COMMERCE_ADMIN_PROMOTION_CLIENT_PUBLIC_MESSAGE.to_string())",
  "fn promotion_client_error_facts(error: &ApiError)",
  'ApiError::Graphql(message) => ("graphql", message)',
  'ApiError::ServerFn(message) => ("server_fn", message)',
  "message_present: !message.trim().is_empty()",
  "message_length: message.chars().count()",
]) {
  requireText(safety, marker, `${paths.safety}: safe final mapping`);
}

const mapperBody = functionBody(safety, "map_error");
requireText(
  mapperBody,
  "promotion_client_error_facts(&error)",
  `${paths.safety}: error shape must be captured before logging`,
);
for (const forbidden of [
  "raw_error = ?error",
  "error = ?error",
  "error = %error",
  "message = %",
  "cart_id = %",
  "cart_id = ?",
  "payload = ?",
  "draft = ?",
  "source_id =",
  "line_item_id =",
  "discount_percent =",
  "amount =",
  "metadata_json =",
  "error.to_string()",
]) {
  forbidText(safety, forbidden, `${paths.safety}: raw request or error text`);
}

for (const source of [native, nativeSsr]) {
  for (const [operation, nativeCall] of [
    [
      "preview_cart_promotion",
      "commerce_admin_preview_cart_promotion_native(cart_id, payload)",
    ],
    [
      "apply_cart_promotion",
      "commerce_admin_apply_cart_promotion_native(cart_id, payload)",
    ],
  ]) {
    const body = functionBody(source, operation);
    requireText(body, nativeCall, `native adapter: preserved ${operation} call`);
    requireText(body, ".map_err(Into::into)", `native adapter: preserved ${operation} mapping`);
  }
}

for (const endpoint of [
  'endpoint = "commerce/admin/preview-cart-promotion"',
  'endpoint = "commerce/admin/apply-cart-promotion"',
]) {
  requireText(nativeSsr, endpoint, `${paths.nativeSsr}: preserved endpoint`);
}
requireText(
  nativeGuard,
  "Commerce admin promotion native error-safety source invariants passed",
  `${paths.nativeGuard}: prior server-side guard remains registered`,
);

if (
  evidence.status !==
  "commerce_admin_promotion_client_transport_error_safety_source_unvalidated"
) {
  failures.push(`${paths.evidence}: unexpected status ${evidence.status}`);
}
if (
  review.status !==
  "commerce_admin_promotion_client_transport_error_safety_source_reviewed_unvalidated"
) {
  failures.push(`${paths.review}: unexpected status ${review.status}`);
}

for (const [key, expected] of Object.entries({
  native_adapters_changed: false,
  server_functions_changed: false,
  order_change_transport_changed: false,
  request_response_dto_changed: false,
  api_error_contract_preserved: true,
  operation_count: 2,
  context_created_before_native_call: true,
  raw_native_error_public: false,
  static_public_message: true,
  original_error_logged_privately: false,
  original_error_shape_logged: true,
  error_variant_logged: true,
  error_message_length_only: true,
  raw_native_error_logged: false,
  per_call_correlation_logging: true,
  safe_request_shape_only: true,
  cart_id_values_logged: false,
  promotion_payload_values_logged: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_contract.${key} must be ${expected}`);
  }
}

for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "hydrate_compile_proven",
  "ssr_compile_proven",
  "mounted_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

requireText(doc, "Status: **source-ready / unvalidated**", `${paths.doc}: status`);
requireText(
  doc,
  "Commerce admin promotion request could not be completed",
  `${paths.doc}: static public message`,
);
requireText(
  doc,
  "The complete native error and its message are not logged",
  `${paths.doc}: raw diagnostic removal`,
);
requireText(
  commercePlan,
  "Finish correlation-safe mapper cleanup",
  `${paths.commercePlan}: broad ecommerce cleanup remains open`,
);

if (failures.length > 0) {
  console.error("Commerce Admin promotion client transport error-safety verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "Commerce Admin promotion preview/apply failures use a correlation-safe static final envelope and shape-only diagnostics; execution evidence remains open",
);
