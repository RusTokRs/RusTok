#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const verifier = resolve(
  "scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs",
);
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
const consumerPath = "apps/server/src/services/product_catalog_deployment.rs";
const commands = {
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
const requiredEnvironment = [
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
const remainingGates = [
  "authenticated_separate_process_commerce_business_request_evidence",
  "authenticated_separate_process_ai_business_request_evidence",
  "retained_business_end_to_end_logs_or_ci_artifacts_for_transport_promotion",
];

function write(root, path, content) {
  const absolute = join(root, path);
  mkdirSync(dirname(absolute), { recursive: true });
  writeFileSync(absolute, content);
}

function contract(options = {}) {
  const contractCommands = structuredClone(commands);
  if (options.arbitraryCommand) {
    contractCommands.probe = { program: "sh", args: ["-c", "echo unsafe"] };
  }
  const forced = {
    RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK: options.weakLoopback
      ? "false"
      : "true",
    RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK: "true",
    RUSTOK_PRODUCT_CATALOG_PROVIDER: "grpc",
    RUSTOK_ENV: "development",
    RUSTOK_LOG_FORMAT: "json",
    OTEL_ENABLED: "false",
  };
  return {
    schema_version: 1,
    module: "product",
    packet: "product-catalog-separate-process-runtime-contract",
    status: "runtime_execution_contract_locked",
    runner: runnerPath,
    verifier: verifierPath,
    evidence_path: evidencePath,
    evidence_status: "runtime_execution_pending",
    execution_scope: "provider_schema_preflight_authenticated_rpc_and_consumer_remote_startup",
    promotion_gate:
      "does_not_close_commerce_or_ai_business_end_to_end_without_separate_requests",
    commands: contractCommands,
    required_environment: requiredEnvironment,
    optional_environment: [],
    forced_environment: forced,
    readiness_markers: {
      provider: [
        "Product catalog database schema preflight passed",
        "Product catalog gRPC service listening",
      ],
      probe: [
        "PRODUCT_CATALOG_RUNTIME_PROBE_OK operations=3 product_projection=matched variant_projection=matched published_list=nonempty",
      ],
      consumer: [
        "Product catalog deployment provider initialized",
        "RusTok Axum host listening",
      ],
    },
    source_files: [runnerPath, probePath, providerPath, consumerPath],
    required_metadata: [],
    privacy_boundary: {
      forbidden_persisted_values: [
        "database_url",
        "consumer_database_url",
        "bearer_token",
        "tenant_id",
        "product_id",
        "variant_id",
        "raw_process_output",
        "tls_private_key",
        "authorization_metadata",
      ],
      persist_environment_names_only: true,
      persist_output_hashes_only: true,
    },
  };
}

const runnerMarkers = [
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
  ...remainingGates.map((gate) => `"${gate}"`),
];
const probeMarkers = [
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
];
const providerMarkers = [
  "verify_required_schema(&database).await?",
  '"Product catalog database schema preflight passed"',
  '"Product catalog gRPC service listening"',
  "ProductCatalogGrpcBearerInterceptor::from_bearer_token",
];
const consumerMarkers = [
  "GrpcProductCatalogReadConnectionConfig::new",
  ".with_authentication(authentication)",
  "ProductCatalogReadRuntime::external",
  '"Product catalog deployment provider initialized"',
];
const planMarkers = [
  "separate-process runtime capture contract is source-complete",
  "raw database URLs, bearer credentials, tenant IDs, product IDs, variant IDs, and process logs are not retained",
  "Commerce and AI business requests through the separate consumer process remain open",
  "Product remains `boundary_ready` rather than `transport_verified`",
  "- [x] Lock a reproducible separate-process Product runtime evidence capture contract.",
  "- [ ] Execute the separate-process Product runtime capture and retain its sanitized evidence packet.",
  "- [ ] Retain authenticated separate-process Commerce and AI business-request evidence.",
];

function fixture(options = {}) {
  const root = mkdtempSync(join(tmpdir(), "rustok-product-runtime-contract-"));
  write(root, contractPath, JSON.stringify(contract(options)));
  const runner = [...runnerMarkers];
  if (options.missingRedaction) {
    runner.splice(runner.indexOf("rejectSecretLeaks("), 1);
  }
  write(root, runnerPath, runner.join("\n"));
  write(
    root,
    probePath,
    [...probeMarkers, ...(options.ownerBypass ? ["CatalogService"] : [])].join("\n"),
  );
  write(root, providerPath, providerMarkers.join("\n"));
  write(root, consumerPath, consumerMarkers.join("\n"));
  write(
    root,
    registryPath,
    JSON.stringify({
      status: options.prematurePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        separate_process_runtime_contract: contractPath,
        separate_process_runtime_contract_verifier: verifierPath,
        separate_process_runtime_capture_runner: runnerPath,
      },
      separate_process_runtime: {
        status: "source_complete_execution_pending",
        execution_scope: "provider_schema_preflight_authenticated_rpc_and_consumer_remote_startup",
        evidence_path: evidencePath,
        remaining_gates: options.removedGate ? remainingGates.slice(0, -1) : remainingGates,
      },
    }),
  );
  write(root, planPath, (options.missingPlan ? planMarkers.slice(1) : planMarkers).join("\n"));
  if (options.forbiddenEvidence) {
    write(
      root,
      evidencePath,
      JSON.stringify({
        schema_version: 1,
        module: "product",
        packet: "product-catalog-separate-process-runtime-evidence",
        status: "separate_process_runtime_executed",
        generated_from: contractPath,
        runner: runnerPath,
        verifier: verifierPath,
        tenant_id: "forbidden",
        remaining_gates: remainingGates,
        promotion: {
          product_status: "boundary_ready",
          transport_verified_claimed: false,
        },
      }),
    );
  }
  return root;
}

function run(root) {
  return spawnSync("node", [verifier], {
    cwd: resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected mutated runtime contract to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("separate-process runtime contract accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("guard rejects arbitrary probe commands", () => {
  reject({ arbitraryCommand: true }, /command allowlist drift/);
});

test("guard rejects weakened loopback flags", () => {
  reject({ weakLoopback: true }, /fail-closed environment drift/);
});

test("guard rejects missing output redaction", () => {
  reject({ missingRedaction: true }, /rejectSecretLeaks/);
});

test("guard rejects owner-service bypasses in the probe", () => {
  reject({ ownerBypass: true }, /CatalogService/);
});

test("guard rejects removed business end-to-end gates", () => {
  reject({ removedGate: true }, /runtime status drift/);
});

test("guard rejects premature Product promotion", () => {
  reject({ prematurePromotion: true }, /boundary_ready/);
});

test("guard rejects missing implementation-plan handoff", () => {
  reject({ missingPlan: true }, /source-complete/);
});

test("guard rejects forbidden retained fixture identifiers", () => {
  reject({ forbiddenEvidence: true }, /forbidden field: tenant_id/);
});
