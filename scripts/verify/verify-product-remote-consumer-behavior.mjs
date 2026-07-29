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
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required remote Product consumer behavior file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function forbidAll(source, markers, description) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
  }
}

const commerceCargo = read("crates/rustok-commerce/Cargo.toml");
const commerceBehavior = read(
  "crates/rustok-commerce/tests/product_remote_consumer_behavior.rs",
);
const checkoutPlan = read(
  "crates/rustok-commerce/src/services/checkout_plan_builder.rs",
);
const aiCargo = read("crates/rustok-ai/Cargo.toml");
const aiBehavior = read("crates/rustok-ai/src/direct_product_attributes.rs");
const registrySource = read(
  "crates/rustok-product/contracts/product-fba-registry.json",
);
const plan = read("crates/rustok-product/docs/implementation-plan.md");
const aiProductRegistrySource = read(
  "crates/rustok-ai-product/contracts/ai-product-fba-registry.json",
);
const aiProductPlan = read("crates/rustok-ai-product/docs/implementation-plan.md");

requireAll(commerceCargo, [
  'rustok-product-transport = { path = "../rustok-product-transport" }',
  'tokio-stream = { version = "0.1", features = ["net"] }',
  "tonic.workspace = true",
  'name = "product_remote_consumer_behavior"',
  'path = "tests/product_remote_consumer_behavior.rs"',
], "Commerce remote behavior test registration");
requireAll(commerceBehavior, [
  "ProductCatalogReadServiceServer::with_interceptor",
  "ProductCatalogGrpcService::new",
  "GrpcProductCatalogReadProvider::connect",
  "ProductCatalogReadRuntime::external",
  "ProductCatalogReadProfile::External",
  "CheckoutPlanBuilder::new",
  ".build(tenant_id, actor_id, Uuid::new_v4(), &input, &snapshot)",
  "CheckoutError::BoundaryFailure",
  'assert_eq!(stage, "read_checkout_product_projection")',
  '"product.remote_unavailable"',
  '"product.remote_timeout"',
  "assert!(retryable)",
  "remote_product_unavailable_blocks_checkout_without_snapshot_fallback",
  "remote_product_timeout_blocks_checkout_without_snapshot_fallback",
], "Commerce remote Product hard-dependency harness");
forbidAll(commerceBehavior, [
  "CatalogService::new",
  "fallback_to_cart_snapshot",
  "use_cart_snapshot_fallback",
], "Commerce remote Product fallback ownership");
requireAll(checkoutPlan, [
  "self.product_catalog_read_port",
  ".read_product_projection(",
  ".read_variant_product_projection(",
  'boundary_error("read_checkout_product_projection", error)',
], "Commerce checkout Product boundary");

requireAll(aiCargo, [
  'rustok-product-transport = { path = "../rustok-product-transport" }',
  'tokio-stream = { version = "0.1", features = ["net"] }',
  "tonic.workspace = true",
], "AI remote behavior test dependencies");
requireAll(aiBehavior, [
  "mod remote_profile_tests",
  "ProductCatalogReadServiceServer::with_interceptor",
  "ProductCatalogGrpcService::new",
  "GrpcProductCatalogReadProvider::connect",
  "ProductCatalogReadRuntime::external",
  "ProductCatalogReadProfile::External",
  "runtime_with_remote_failure",
  "remote_product_unavailable_degrades_ai_enrichment",
  "remote_product_timeout_degrades_ai_enrichment",
  '"product.remote_unavailable"',
  '"product.remote_timeout"',
  'assert_eq!(metadata["source"], "degraded")',
  'assert_eq!(metadata["catalog_enrichment"], "skipped")',
  'assert_eq!(metadata["errors"][0]["retryable"], true)',
  '"review_required": true',
  '"persistence": "none"',
], "AI remote Product degraded-behavior harness");
forbidAll(aiBehavior, [
  "CatalogService::new",
  "persist_generated_attributes",
  "fallback_to_product_storage",
], "AI remote Product advisory boundary");

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  if (registry.status !== "boundary_ready") {
    failures.push("Product must remain boundary_ready before remote behavior execution evidence");
  }
  if (registry.external_transport?.status !== "runtime_wired_execution_pending") {
    failures.push("Product external transport status must remain runtime_wired_execution_pending");
  }
  const behavior = registry.remote_consumer_behavior ?? {};
  if (behavior.status !== "source_complete_execution_pending") {
    failures.push("remote consumer behavior must remain source_complete_execution_pending");
  }
  if (
    behavior.commerce_test !==
    "crates/rustok-commerce/tests/product_remote_consumer_behavior.rs"
  ) {
    failures.push("remote consumer registry must identify the Commerce harness");
  }
  if (behavior.ai_source_test !== "crates/rustok-ai/src/direct_product_attributes.rs") {
    failures.push("remote consumer registry must identify the AI source harness");
  }
  for (const failureProfile of ["unavailable", "timeout"]) {
    if (!behavior.failure_profiles?.includes(failureProfile)) {
      failures.push(`remote consumer behavior must include ${failureProfile}`);
    }
  }
  for (const assertion of [
    "commerce_hard_dependency_no_cart_snapshot_fallback",
    "commerce_typed_boundary_error_preserved",
    "ai_catalog_enrichment_skipped",
    "ai_typed_degraded_error_preserved",
    "ai_operator_review_required",
    "ai_persistence_none",
  ]) {
    if (!behavior.assertions?.includes(assertion)) {
      failures.push(`remote consumer behavior must assert ${assertion}`);
    }
  }
  if (
    registry.evidence?.remote_consumer_behavior_verifier !==
    "scripts/verify/verify-product-remote-consumer-behavior.mjs"
  ) {
    failures.push("Product registry must link the remote consumer behavior verifier");
  }
}

let aiProductRegistry;
try {
  aiProductRegistry = JSON.parse(aiProductRegistrySource);
} catch (error) {
  failures.push(`AI-product FBA registry is invalid JSON: ${error.message}`);
}
if (aiProductRegistry) {
  if (aiProductRegistry.status !== "boundary_ready") {
    failures.push("AI-product must remain boundary_ready before remote behavior execution evidence");
  }
  const dependency = aiProductRegistry.provider_dependencies?.find(
    (entry) => entry.module === "product",
  );
  if (!dependency?.required_profiles?.includes("grpc_loopback")) {
    failures.push("AI-product Product dependency must include grpc_loopback");
  }
  const behavior = aiProductRegistry.remote_consumer_behavior ?? {};
  if (behavior.status !== "source_complete_execution_pending") {
    failures.push("AI-product remote behavior must remain source_complete_execution_pending");
  }
  if (behavior.profile !== "grpc_loopback") {
    failures.push("AI-product remote behavior must identify grpc_loopback");
  }
  if (behavior.source !== "crates/rustok-ai/src/direct_product_attributes.rs") {
    failures.push("AI-product remote behavior must identify the capability handler source");
  }
  for (const failureProfile of ["unavailable", "timeout"]) {
    if (!behavior.failure_profiles?.includes(failureProfile)) {
      failures.push(`AI-product remote behavior must include ${failureProfile}`);
    }
  }
  for (const assertion of [
    "generate_from_prompt_only",
    "skip_catalog_enrichment",
    "require_operator_review",
    "persistence_none",
    "typed_port_error_preserved",
  ]) {
    if (!behavior.assertions?.includes(assertion)) {
      failures.push(`AI-product remote behavior must assert ${assertion}`);
    }
  }
  if (
    aiProductRegistry.evidence?.remote_consumer_behavior_verifier !==
    "scripts/verify/verify-product-remote-consumer-behavior.mjs"
  ) {
    failures.push("AI-product registry must link the remote consumer behavior verifier");
  }
}

requireAll(plan, [
  "Remote consumer behavior is now source-complete through executable loopback",
  "it never substitutes the",
  "cart line snapshot for current Product authority",
  "both failures skip catalog enrichment",
  "requires operator review",
  "performs no persistence",
  "Product remains `boundary_ready`",
  "Add executable Commerce hard-dependency and AI degraded-behavior gRPC harnesses",
  "Execute the Commerce and AI remote consumer behavior harnesses",
  "cargo test -p rustok-commerce --test product_remote_consumer_behavior",
  "cargo test -p rustok-ai --features server --lib remote_product_",
  "verify-product-remote-consumer-behavior.mjs",
], "Product remote consumer implementation plan");
requireAll(aiProductPlan, [
  "A source-complete gRPC loopback harness now exercises the same product-context",
  "Remote `Unavailable` and `Timeout` errors preserve",
  "review-required and non-persistent",
  "source_complete_execution_pending",
  "Execute the remote Product consumer harness",
  "cargo test -p rustok-ai --features server --lib remote_product_",
  "verify-product-remote-consumer-behavior.mjs",
], "AI-product remote consumer implementation plan");

if (failures.length > 0) {
  console.error("Product remote consumer behavior verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Product remote consumer behavior source verification passed");
