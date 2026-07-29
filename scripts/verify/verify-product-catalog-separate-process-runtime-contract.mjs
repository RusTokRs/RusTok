#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = fileURLToPath(new URL("../../", import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : defaultRoot;
const contractPath =
  "crates/rustok-product/contracts/evidence/product-catalog-separate-process-runtime-contract.json";
const evidencePath =
  "crates/rustok-product/contracts/evidence/product-catalog-separate-process-runtime.json";
const runnerPath =
  "scripts/evidence/capture-product-catalog-separate-process-runtime.mjs";
const verifierPath =
  "scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs";
const probePath =
  "crates/rustok-product-transport/examples/product_catalog_runtime_probe.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const providerPath = "crates/rustok-product-catalog-service/src/main.rs";
const consumerDeploymentPath = "apps/server/src/services/product_catalog_deployment.rs";
const expectedCommands = {
  provider: {
    program: "cargo",
    args: ["run", "-p", "rustok-product-catalog-service"],
  },
  probe: {
    program: "cargo",
    args: [
      "run",
      "-p",
      "rustok-product-transport",
      "--example",
      "product_catalog_runtime_probe",
    ],
  },
  consumer: {
    program: "cargo",
    args: [
      "run",
      "-p",
      "rustok-server",
      "--no-default-features",
      "--features",
      "mod-commerce,mod-ai",
    ],
  },
};
const expectedRequiredEnvironment = [
  "RUSTOK_PRODUCT_CATALOG_DATABASE_URL",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_CONSUMER_DATABASE_URL",
  "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
  "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
  "RUSTOK_PRODUCT_CATALOG_SERVICE_BIND",
  "RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_TENANT_ID",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_PRODUCT_ID",
  "RUSTOK_PRODUCT_CATALOG_EVIDENCE_VARIANT_ID",
];
const expectedRemainingGates = [
  "authenticated_separate_process_commerce_business_request_evidence",
  "authenticated_separate_process_ai_business_request_evidence",
  "retained_business_end_to_end_logs_or_ci_artifacts_for_transport_promotion",
];

function fail(message) {
  throw new Error(message);
}

function readText(relativePath) {
  return readFileSync(resolve(repoRoot, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function sameSet(actual, expected) {
  return (
    Array.isArray(actual) &&
    actual.length === expected.length &&
    expected.every((value) => actual.includes(value))
  );
}

function requireText(label, source, marker) {
  if (!source.includes(marker)) fail(`${label} is missing required marker: ${marker}`);
}

function forbidText(label, source, marker) {
  if (source.includes(marker)) fail(`${label} contains forbidden marker: ${marker}`);
}

function verifyContract() {
  const contract = readJson(contractPath);
  if (contract.schema_version !== 1 || contract.module !== "product") {
    fail("Product separate-process contract identity drift");
  }
  if (
    contract.packet !== "product-catalog-separate-process-runtime-contract" ||
    contract.status !== "runtime_execution_contract_locked"
  ) {
    fail("Product separate-process contract status drift");
  }
  if (
    contract.runner !== runnerPath ||
    contract.verifier !== verifierPath ||
    contract.evidence_path !== evidencePath ||
    contract.evidence_status !== "runtime_execution_pending"
  ) {
    fail("Product separate-process tooling boundary drift");
  }
  if (!sameValue(contract.commands, expectedCommands)) {
    fail("Product separate-process command allowlist drift");
  }
  if (!sameValue(contract.required_environment, expectedRequiredEnvironment)) {
    fail("Product separate-process required environment drift");
  }
  if (
    contract.execution_scope !==
      "provider_schema_preflight_authenticated_rpc_and_consumer_remote_startup" ||
    contract.promotion_gate !==
      "does_not_close_commerce_or_ai_business_end_to_end_without_separate_requests"
  ) {
    fail("Product separate-process promotion boundary drift");
  }
  if (
    contract.forced_environment?.RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK !==
      "true" ||
    contract.forced_environment?.RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK !==
      "true" ||
    contract.forced_environment?.RUSTOK_PRODUCT_CATALOG_PROVIDER !== "grpc"
  ) {
    fail("Product separate-process fail-closed environment drift");
  }
  if (
    !sameValue(contract.readiness_markers?.provider, [
      "Product catalog database schema preflight passed",
      "Product catalog gRPC service listening",
    ]) ||
    !sameValue(contract.readiness_markers?.probe, [
      "PRODUCT_CATALOG_RUNTIME_PROBE_OK operations=3 product_projection=matched variant_projection=matched published_list=nonempty",
    ]) ||
    !sameValue(contract.readiness_markers?.consumer, [
      "Product catalog deployment provider initialized",
      "RusTok Axum host listening",
    ])
  ) {
    fail("Product separate-process readiness marker drift");
  }
  if (
    !sameSet(contract.privacy_boundary?.forbidden_persisted_values, [
      "database_url",
      "consumer_database_url",
      "bearer_token",
      "tenant_id",
      "product_id",
      "variant_id",
      "raw_process_output",
      "tls_private_key",
      "authorization_metadata",
    ]) ||
    contract.privacy_boundary?.persist_environment_names_only !== true ||
    contract.privacy_boundary?.persist_output_hashes_only !== true
  ) {
    fail("Product separate-process privacy boundary drift");
  }
  for (const sourcePath of contract.source_files ?? []) {
    if (!existsSync(resolve(repoRoot, sourcePath))) {
      fail(`Product separate-process source file is missing: ${sourcePath}`);
    }
  }
}

function verifyRunner() {
  const source = readText(runnerPath);
  for (const marker of [
    "validateContractBoundary()",
    "validateNoTlsOverrides()",
    "validateLoopbackBinding(",
    "validateLoopbackEndpoint(",
    "ensureCleanCommit()",
    'runChecked("git", ["status", "--porcelain=v1", "--untracked-files=all"])',
    'runChecked("git", ["rev-parse", "HEAD"])',
    "sourceHashes()",
    "rejectSecretLeaks(",
    "writeAtomically({",
    "renameSync(temporaryPath, outputPath)",
    'record.child.kill("SIGTERM")',
    'record.child.kill("SIGKILL")',
    "await providerProcess.waitForMarkers(",
    "await consumerProcess.waitForMarkers(",
    "authenticated Product catalog runtime probe did not report its success marker",
    'product_status: "boundary_ready"',
    "transport_verified_claimed: false",
    ...expectedRemainingGates.map((gate) => `"${gate}"`),
  ]) {
    requireText("Product separate-process runner", source, marker);
  }
  for (const marker of [
    "shell: true",
    "execSync(",
    "console.log(providerProcess.output",
    "console.log(consumerProcess.output",
    "raw_process_output:",
    'product_status: "transport_verified"',
    "transport_verified_claimed: true",
  ]) {
    forbidText("Product separate-process runner", source, marker);
  }
}

function verifyProbe() {
  const source = readText(probePath);
  for (const marker of [
    "GrpcProductCatalogReadConnectionConfig::new",
    "ProductCatalogGrpcBearerToken::new",
    ".with_authentication(authentication)",
    ".read_product_projection(",
    ".read_variant_product_projection(",
    ".list_published_products(",
    "product.id != config.product_id",
    "product.tenant_id != config.tenant_id",
    "variant.id == config.variant_id",
    "published.total == 0",
    "PRODUCT_CATALOG_RUNTIME_PROBE_OK operations=3",
  ]) {
    requireText("Product authenticated runtime probe", source, marker);
  }
  for (const marker of [
    "println!(config.bearer_token",
    "dbg!(config",
    "CatalogService",
    "Database::connect",
    "Entity::find",
  ]) {
    forbidText("Product authenticated runtime probe", source, marker);
  }
}

function verifyCanonicalSources() {
  const provider = readText(providerPath);
  const consumer = readText(consumerDeploymentPath);
  for (const marker of [
    "verify_required_schema(&database).await?",
    '"Product catalog database schema preflight passed"',
    '"Product catalog gRPC service listening"',
    "ProductCatalogGrpcBearerInterceptor::from_bearer_token",
  ]) {
    requireText("Product provider host", provider, marker);
  }
  for (const marker of [
    "GrpcProductCatalogReadConnectionConfig::new",
    ".with_authentication(authentication)",
    "ProductCatalogReadRuntime::external",
    '"Product catalog deployment provider initialized"',
  ]) {
    requireText("Product consumer deployment", consumer, marker);
  }
}

function verifyRegistryAndPlan() {
  const registry = readJson(registryPath);
  const plan = readText(planPath);
  if (registry.status !== "boundary_ready") {
    fail("Product must remain boundary_ready before retained business end-to-end evidence");
  }
  if (
    registry.evidence?.separate_process_runtime_contract !== contractPath ||
    registry.evidence?.separate_process_runtime_contract_verifier !== verifierPath ||
    registry.evidence?.separate_process_runtime_capture_runner !== runnerPath
  ) {
    fail("Product registry separate-process evidence linkage drift");
  }
  const runtime = registry.separate_process_runtime;
  if (
    runtime?.status !== "source_complete_execution_pending" ||
    runtime?.execution_scope !==
      "provider_schema_preflight_authenticated_rpc_and_consumer_remote_startup" ||
    runtime?.evidence_path !== evidencePath ||
    !sameValue(runtime?.remaining_gates, expectedRemainingGates)
  ) {
    fail("Product registry separate-process runtime status drift");
  }
  for (const marker of [
    "separate-process runtime capture contract is source-complete",
    "raw database URLs, bearer credentials, tenant IDs, product IDs, variant IDs, and process logs are not retained",
    "Commerce and AI business requests through the separate consumer process remain open",
    "Product remains `boundary_ready` rather than `transport_verified`",
    "- [x] Lock a reproducible separate-process Product runtime evidence capture contract.",
    "- [ ] Execute the separate-process Product runtime capture and retain its sanitized evidence packet.",
    "- [ ] Retain authenticated separate-process Commerce and AI business-request evidence.",
  ]) {
    requireText("Product implementation plan", plan, marker);
  }
}

function verifyRetainedEvidenceIfPresent() {
  const absoluteEvidencePath = resolve(repoRoot, evidencePath);
  if (!existsSync(absoluteEvidencePath)) return;
  const evidence = readJson(evidencePath);
  if (
    evidence.schema_version !== 1 ||
    evidence.module !== "product" ||
    evidence.packet !== "product-catalog-separate-process-runtime-evidence" ||
    evidence.status !== "separate_process_runtime_executed"
  ) {
    fail("retained Product separate-process evidence identity drift");
  }
  if (
    evidence.generated_from !== contractPath ||
    evidence.runner !== runnerPath ||
    evidence.verifier !== verifierPath ||
    evidence.promotion?.product_status !== "boundary_ready" ||
    evidence.promotion?.transport_verified_claimed !== false
  ) {
    fail("retained Product separate-process evidence promotion boundary drift");
  }
  if (!sameValue(evidence.remaining_gates, expectedRemainingGates)) {
    fail("retained Product separate-process remaining gates drift");
  }
  const serialized = JSON.stringify(evidence);
  for (const forbiddenField of [
    "database_url",
    "consumer_database_url",
    "bearer_token",
    "tenant_id",
    "product_id",
    "variant_id",
    "raw_process_output",
    "authorization_metadata",
  ]) {
    if (serialized.includes(`"${forbiddenField}"`)) {
      fail(`retained Product separate-process evidence contains forbidden field: ${forbiddenField}`);
    }
  }
}

try {
  verifyContract();
  verifyRunner();
  verifyProbe();
  verifyCanonicalSources();
  verifyRegistryAndPlan();
  verifyRetainedEvidenceIfPresent();
  console.log(
    "[verify-product-catalog-separate-process-runtime-contract] Product runtime capture contract is source-complete and execution-pending",
  );
} catch (error) {
  console.error(
    `[verify-product-catalog-separate-process-runtime-contract] ${error.message}`,
  );
  process.exit(1);
}
