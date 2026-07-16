#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireMarker(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function forbidMarker(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const migrationPath =
  "crates/rustok-payment/src/migrations/m20260714_000119_require_refund_creation_identity.rs";
const migrationRegistryPath = "crates/rustok-payment/src/migrations/mod.rs";
const refundServicePath = "crates/rustok-payment/src/services/refund_creation.rs";
const legacyPaymentServicePath = "crates/rustok-payment/src/services/payment.rs";
const orchestrationPath = "crates/rustok-commerce/src/services/payment_orchestration.rs";
const restPath = "crates/rustok-commerce/src/controllers/admin/payments.rs";
const graphqlPath =
  "crates/rustok-commerce/src/graphql/mutations/provider_operations.rs";
const graphqlReturnPath =
  "crates/rustok-commerce/src/graphql/mutations/provider_return_helpers.rs";
const adminReturnPath = "crates/rustok-commerce/src/controllers/admin/returns.rs";
const packagePath = "package.json";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";

const migration = read(migrationPath);
const migrationRegistry = read(migrationRegistryPath);
const refundService = read(refundServicePath);
const legacyPaymentService = read(legacyPaymentServicePath);
const orchestration = read(orchestrationPath);
const rest = read(restPath);
const graphql = read(graphqlPath);
const graphqlReturn = read(graphqlReturnPath);
const adminReturn = read(adminReturnPath);
const packageJson = read(packagePath);
const plan = read(planPath);

for (const marker of [
  "ALTER COLUMN creation_key SET NOT NULL",
  "refund creation identity is required",
  "creation_request_hash",
  "legacy:",
]) {
  requireMarker(migration, marker, `${migrationPath}: missing ${marker}`);
}
requireMarker(
  migrationRegistry,
  "m20260714_000119_require_refund_creation_identity",
  `${migrationRegistryPath}: migration 000119 is not registered`,
);
for (const marker of [
  "pub async fn create_or_replay",
  "refund_request_hash",
  "find_by_creation_key",
  "UniqueConstraintViolation",
]) {
  requireMarker(refundService, marker, `${refundServicePath}: missing ${marker}`);
}
forbidMarker(
  legacyPaymentService,
  "pub async fn create_refund(",
  `${legacyPaymentServicePath}: legacy identity-less refund creation API must not exist`,
);
forbidMarker(
  legacyPaymentService,
  "CreateRefundInput",
  `${legacyPaymentServicePath}: PaymentService must not own refund creation input`,
);
for (const marker of [
  "pub async fn create_refund_idempotent",
  "PaymentRefundCreationService",
  "workflow_refund_creation_key",
  "order_return:",
  "order_change:",
]) {
  requireMarker(orchestration, marker, `${orchestrationPath}: missing ${marker}`);
}
requireMarker(
  rest,
  'headers.get("idempotency-key")',
  `${restPath}: REST refund must require Idempotency-Key`,
);
requireMarker(
  rest,
  ".create_refund_idempotent(",
  `${restPath}: REST refund must use explicit idempotent API`,
);
requireMarker(
  graphql,
  "idempotency_key: String",
  `${graphqlPath}: GraphQL refund must require idempotencyKey`,
);
requireMarker(
  graphql,
  ".create_refund_idempotent(",
  `${graphqlPath}: GraphQL refund must use explicit idempotent API`,
);
requireMarker(
  graphqlReturn,
  'format!("order_return:{return_id}:refund")',
  `${graphqlReturnPath}: GraphQL return refund identity drift`,
);
requireMarker(
  adminReturn,
  'format!("order_return:{id}:refund")',
  `${adminReturnPath}: admin return refund identity drift`,
);
forbidMarker(
  rest,
  ".create_refund(tenant.id, id, input)",
  `${restPath}: REST must not call legacy refund creation`,
);
forbidMarker(
  graphql,
  ".create_refund(\n",
  `${graphqlPath}: GraphQL must not call legacy refund creation`,
);
requireMarker(
  packageJson,
  '"verify:payment:refund-identity"',
  `${packagePath}: refund identity verifier script is not registered`,
);
requireMarker(
  packageJson,
  "npm run verify:payment:refund-identity",
  `${packagePath}: aggregate verification must run refund identity guard`,
);
requireMarker(
  plan,
  "refund `creation_key`",
  `${planPath}: main plan must track refund creation identity`,
);
requireMarker(
  plan,
  "verify-payment-refund-identity.mjs",
  `${planPath}: main plan must list refund identity verifier`,
);

if (failures.length > 0) {
  console.error("payment refund identity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment refund identity verification passed");
