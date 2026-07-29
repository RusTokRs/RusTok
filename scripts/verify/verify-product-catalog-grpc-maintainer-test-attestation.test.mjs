#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.mjs",
);
const evidencePath =
  "crates/rustok-product/contracts/evidence/product-catalog-grpc-maintainer-test-attestation.json";
const verifierPath =
  "scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.mjs";
const commands = [
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
const remainingGates = [
  "standalone_provider_postgresql_schema_preflight_runtime_evidence",
  "authenticated_separate_process_commerce_end_to_end_evidence",
  "authenticated_separate_process_ai_end_to_end_evidence",
  "retained_runtime_logs_or_ci_artifacts_for_transport_promotion",
];

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-grpc-attestation-"));
  const evidenceCommands = commands.map((command, index) => ({
    command,
    result: options.failedCommand && index === 0 ? "failed" : "passed",
    evidence_quality: "maintainer_attested_no_raw_log",
  }));
  if (options.missingCommand) evidenceCommands.pop();
  const gates = options.missingRemainingGate ? remainingGates.slice(0, -1) : remainingGates;
  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: "product",
      packet: "product-catalog-grpc-maintainer-test-attestation",
      status: "maintainer_attested_passed",
      attestation: {
        source: "repository_maintainer_chat_confirmation",
        attested_at: "2026-07-29T16:33:00+03:00",
        statement: "tests passed",
        agent_independently_verified: options.falseIndependentClaim ?? false,
        raw_logs_retained: options.falseRawLogClaim ?? false,
        exact_local_git_commit: null,
        repository_head_observed_after_attestation:
          "6655bda2068911bf010be2d950638a4473f953f1",
      },
      cargo_lock: {
        path: "Cargo.lock",
        package: "rustok-product-catalog-service",
        entry_present_in_observed_head: true,
      },
      commands: evidenceCommands,
      closed_gates: [],
      remaining_gates: gates,
      promotion: {
        product_status: options.falsePromotion ? "transport_verified" : "boundary_ready",
        transport_verified_claimed: options.falsePromotion ?? false,
        reason: "bounded maintainer attestation",
      },
    }),
  );
  write(
    root,
    "Cargo.lock",
    options.missingLock
      ? "version = 4"
      : 'version = 4\n\n[[package]]\nname = "rustok-product-catalog-service"\nversion = "0.1.0"',
  );
  write(
    root,
    "crates/rustok-product/contracts/product-fba-registry.json",
    JSON.stringify({
      status: options.falsePromotion ? "transport_verified" : "boundary_ready",
      evidence: {
        grpc_maintainer_test_attestation: evidencePath,
        grpc_maintainer_test_attestation_verifier: verifierPath,
      },
      maintainer_test_attestation: {
        status: "passed_no_raw_logs_end_to_end_pending",
        executed_command_count: commands.length,
        remaining_gates: remainingGates,
      },
    }),
  );
  write(
    root,
    "crates/rustok-ai-product/contracts/ai-product-fba-registry.json",
    JSON.stringify({
      status: "boundary_ready",
      evidence: {
        product_grpc_maintainer_test_attestation: options.missingAiLink
          ? "missing.json"
          : evidencePath,
      },
      remote_consumer_behavior: {
        maintainer_harness_status: "passed_no_raw_logs_end_to_end_pending",
        separate_process_end_to_end_status: "pending",
      },
    }),
  );
  const productPlan = options.missingPlan
    ? "Product plan"
    : `maintainer-attested raw logs were not retained separate-process standalone PostgreSQL schema-preflight runtime evidence remains open Product remains \`boundary_ready\` rather than \`transport_verified\`
- [x] Execute the Product catalog service-host unit, authentication, and loopback conformance test suites.
- [x] Execute the Commerce and AI remote consumer behavior harnesses.
- [ ] Execute the standalone PostgreSQL schema preflight and retain runtime logs.
- [ ] Retain authenticated separate-process Commerce and AI end-to-end evidence.`;
  write(root, "crates/rustok-product/docs/implementation-plan.md", productPlan);
  write(
    root,
    "crates/rustok-ai-product/docs/implementation-plan.md",
    "maintainer-attested raw logs were not retained separate-process",
  );
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected attestation mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("attestation guard accepts canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("guard rejects a failed command", () => {
  reject({ failedCommand: true }, /result must be passed/);
});

test("guard rejects an incomplete command set", () => {
  reject({ missingCommand: true }, /command set or order drift/);
});

test("guard rejects an independent-agent verification claim", () => {
  reject({ falseIndependentClaim: true }, /independent agent verification/);
});

test("guard rejects a retained-raw-log claim", () => {
  reject({ falseRawLogClaim: true }, /retained raw logs/);
});

test("guard rejects a removed promotion gate", () => {
  reject({ missingRemainingGate: true }, /remaining promotion gates drift/);
});

test("guard rejects premature Product promotion", () => {
  reject({ falsePromotion: true }, /boundary_ready|transport_verified/);
});

test("guard rejects a missing Cargo.lock package entry", () => {
  reject({ missingLock: true }, /Cargo.lock Product service package entry/);
});

test("guard rejects missing AI-product evidence linkage", () => {
  reject({ missingAiLink: true }, /AI-product registry must link/);
});

test("guard rejects missing implementation-plan handoff", () => {
  reject({ missingPlan: true }, /Product implementation plan|Product verification checklist/);
});
