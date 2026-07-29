#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const evidencePath =
  "crates/rustok-product/contracts/evidence/product-catalog-grpc-maintainer-test-attestation.json";
const verifierPath =
  "scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.mjs";
const expectedCommands = [
  "node scripts/verify/verify-product-catalog-grpc-service-host.mjs",
  "node scripts/verify/verify-product-catalog-grpc-service-host.test.mjs",
  "node scripts/verify/verify-product-catalog-grpc-authentication.mjs",
  "node scripts/verify/verify-product-catalog-grpc-authentication.test.mjs",
  "cargo test -p rustok-product-catalog-service",
  "cargo test -p rustok-product-transport --lib",
  "cargo test -p rustok-product-transport --test port_conformance",
  "cargo test -p rustok-commerce --test product_remote_consumer_behavior",
  "cargo test -p rustok-ai --features server --lib remote_product_",
];
const expectedRemainingGates = [
  "standalone_provider_postgresql_schema_preflight_runtime_evidence",
  "authenticated_separate_process_commerce_end_to_end_evidence",
  "authenticated_separate_process_ai_end_to_end_evidence",
  "retained_runtime_logs_or_ci_artifacts_for_transport_promotion",
];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required attestation file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function parseJson(relativePath) {
  const source = read(relativePath);
  if (!source) return null;
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function same(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function requireText(source, marker, description) {
  if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
}

const evidence = parseJson(evidencePath);
const productRegistry = parseJson(
  "crates/rustok-product/contracts/product-fba-registry.json",
);
const aiProductRegistry = parseJson(
  "crates/rustok-ai-product/contracts/ai-product-fba-registry.json",
);
const productPlan = read("crates/rustok-product/docs/implementation-plan.md");
const aiProductPlan = read("crates/rustok-ai-product/docs/implementation-plan.md");
const cargoLock = read("Cargo.lock");

if (evidence) {
  if (evidence.schema_version !== 1) failures.push("attestation schema version drift");
  if (evidence.module !== "product") failures.push("attestation module drift");
  if (evidence.packet !== "product-catalog-grpc-maintainer-test-attestation") {
    failures.push("attestation packet identity drift");
  }
  if (evidence.status !== "maintainer_attested_passed") {
    failures.push("attestation must retain maintainer_attested_passed status");
  }
  const attestation = evidence.attestation ?? {};
  if (attestation.source !== "repository_maintainer_chat_confirmation") {
    failures.push("attestation source drift");
  }
  if (Number.isNaN(Date.parse(attestation.attested_at))) {
    failures.push("attestation timestamp must be ISO-8601");
  }
  if (attestation.statement !== "tests passed") {
    failures.push("attestation must retain the exact maintainer statement");
  }
  if (attestation.agent_independently_verified !== false) {
    failures.push("attestation must not claim independent agent verification");
  }
  if (attestation.raw_logs_retained !== false) {
    failures.push("attestation must not claim retained raw logs");
  }
  if (attestation.exact_local_git_commit !== null) {
    failures.push("attestation must not invent the maintainer local revision");
  }
  if (!/^[0-9a-f]{40}$/.test(attestation.repository_head_observed_after_attestation ?? "")) {
    failures.push("attestation observed repository head must be a full SHA");
  }

  const commands = evidence.commands ?? [];
  if (!same(commands.map((entry) => entry.command), expectedCommands)) {
    failures.push("attestation command set or order drift");
  }
  for (const entry of commands) {
    if (entry.result !== "passed") failures.push(`${entry.command}: result must be passed`);
    if (entry.evidence_quality !== "maintainer_attested_no_raw_log") {
      failures.push(`${entry.command}: evidence quality drift`);
    }
  }
  if (!same(evidence.remaining_gates, expectedRemainingGates)) {
    failures.push("attestation remaining promotion gates drift");
  }
  if (evidence.promotion?.product_status !== "boundary_ready") {
    failures.push("attestation must keep Product boundary_ready");
  }
  if (evidence.promotion?.transport_verified_claimed !== false) {
    failures.push("attestation must not claim transport_verified");
  }
}

requireText(
  cargoLock,
  'name = "rustok-product-catalog-service"',
  "Cargo.lock Product service package entry",
);

if (productRegistry) {
  if (productRegistry.status !== "boundary_ready") {
    failures.push("Product must remain boundary_ready");
  }
  if (productRegistry.evidence?.grpc_maintainer_test_attestation !== evidencePath) {
    failures.push("Product registry must link the maintainer test attestation");
  }
  if (productRegistry.evidence?.grpc_maintainer_test_attestation_verifier !== verifierPath) {
    failures.push("Product registry must link the attestation verifier");
  }
  const retained = productRegistry.maintainer_test_attestation ?? {};
  if (retained.status !== "passed_no_raw_logs_end_to_end_pending") {
    failures.push("Product registry maintainer attestation status drift");
  }
  if (retained.executed_command_count !== expectedCommands.length) {
    failures.push("Product registry attested command count drift");
  }
  if (!same(retained.remaining_gates, expectedRemainingGates)) {
    failures.push("Product registry remaining attestation gates drift");
  }
}

if (aiProductRegistry) {
  if (aiProductRegistry.status !== "boundary_ready") {
    failures.push("AI-product must remain boundary_ready");
  }
  if (aiProductRegistry.evidence?.product_grpc_maintainer_test_attestation !== evidencePath) {
    failures.push("AI-product registry must link Product gRPC test attestation");
  }
  const behavior = aiProductRegistry.remote_consumer_behavior ?? {};
  if (behavior.maintainer_harness_status !== "passed_no_raw_logs_end_to_end_pending") {
    failures.push("AI-product maintainer harness status drift");
  }
  if (behavior.separate_process_end_to_end_status !== "pending") {
    failures.push("AI-product separate-process end-to-end must remain pending");
  }
}

for (const [source, description] of [
  [productPlan, "Product implementation plan"],
  [aiProductPlan, "AI-product implementation plan"],
]) {
  requireText(source, "maintainer-attested", description);
  requireText(source, "raw logs were not retained", description);
  requireText(source, "separate-process", description);
}
requireText(
  productPlan,
  "standalone PostgreSQL schema-preflight runtime evidence remains open",
  "Product implementation plan",
);
requireText(
  productPlan,
  "Product remains `boundary_ready` rather than `transport_verified`",
  "Product implementation plan",
);
requireText(
  productPlan,
  "- [x] Execute the Product catalog service-host unit, authentication, and loopback conformance test suites.",
  "Product verification checklist",
);
requireText(
  productPlan,
  "- [x] Execute the Commerce and AI remote consumer behavior harnesses.",
  "Product verification checklist",
);
requireText(
  productPlan,
  "- [ ] Execute the standalone PostgreSQL schema preflight and retain runtime logs.",
  "Product verification checklist",
);
requireText(
  productPlan,
  "- [ ] Retain authenticated separate-process Commerce and AI end-to-end evidence.",
  "Product verification checklist",
);

if (failures.length > 0) {
  console.error("Product catalog gRPC maintainer test attestation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product catalog gRPC maintainer test attestation verification passed");
