#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution-contract.json";
const sourceContractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-source.json";
const runnerPath =
  "scripts/evidence/capture-social-graph-index-raw-poison-postgres-iggy.mjs";
const evidencePath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-execution.json";
const expectedVerifier =
  "scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy-retained.mjs";
const expectedCases = [
  "raw_poison_persists_published_before_source_acknowledgement",
  "published_redelivery_is_acknowledgement_only_without_republication",
];
const expectedCommandTemplate = {
  program: "cargo",
  args_before_case: [
    "test",
    "-p",
    "rustok-social-graph",
    "--features",
    "index-consumer",
    "--test",
    "index_raw_poison_postgres_iggy",
    "--",
  ],
  args_after_case: ["--exact", "--nocapture", "--test-threads=1"],
};

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const sourceContract = JSON.parse(
  readFileSync(resolve(repoRoot, sourceContractPath), "utf8"),
);
const runner = readFileSync(resolve(repoRoot, runnerPath), "utf8");
const failures = [];

function fail(message) {
  failures.push(message);
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function requireText(name, source, marker) {
  if (!source.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
}

function forbidText(name, source, marker) {
  if (source.includes(marker)) fail(`${name} contains forbidden marker: ${marker}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  return sha256(readFileSync(resolve(repoRoot, relativePath)));
}

function currentSourceHashes() {
  return Object.fromEntries(
    contract.source_files.map((relativePath) => [relativePath, fileSha256(relativePath)]),
  );
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    fail("could not read current git commit");
    return null;
  }
  const commit = result.stdout.trim();
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    fail("current git commit is not a full lowercase SHA-1");
    return null;
  }
  return commit;
}

function expectedCommand(caseName) {
  return {
    program: expectedCommandTemplate.program,
    args: [
      ...expectedCommandTemplate.args_before_case,
      caseName,
      ...expectedCommandTemplate.args_after_case,
    ],
  };
}

function boundedLine(value, field) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.length === 0 ||
    value.length > 256 ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    fail(`${field} is outside the retained metadata boundary`);
  }
}

function validTimestamp(value, field) {
  if (typeof value !== "string" || Number.isNaN(Date.parse(value))) {
    fail(`${field} is not a valid timestamp`);
  }
}

function validSha256(value, field) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail(`${field} is not a lowercase SHA-256`);
  }
}

function collectKeys(value, keys = []) {
  if (Array.isArray(value)) {
    for (const entry of value) collectKeys(entry, keys);
    return keys;
  }
  if (value && typeof value === "object") {
    for (const [key, entry] of Object.entries(value)) {
      keys.push(key);
      collectKeys(entry, keys);
    }
  }
  return keys;
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "social-graph" ||
  contract.packet !== "index-raw-poison-postgres-iggy-execution-contract" ||
  contract.status !== "runtime_execution_contract_locked"
) {
  fail("combined poison execution contract identity drift");
}
if (
  contract.source_contract !== sourceContractPath ||
  contract.runner !== runnerPath ||
  contract.verifier !== expectedVerifier ||
  contract.evidence_path !== evidencePath ||
  contract.evidence_status !== "runtime_execution_pending"
) {
  fail("combined poison retained path drift");
}
if (!sameValue(contract.command_template, expectedCommandTemplate)) {
  fail("combined poison command template drift");
}
if (!sameValue(contract.required_cases?.map((entry) => entry.case), expectedCases)) {
  fail("combined poison required case drift");
}
if (sourceContract.execution_status !== "not_run") {
  fail("source contract must remain unexecuted independently of retained packet");
}

for (const marker of [
  "validateDatabaseUrl()",
  "validateIggyAddress()",
  "validateCredentialsPair()",
  "ensureCleanCommit()",
  "sourceHashes()",
  "requirePassedCase(output, requiredCase.case)",
  '"--exact"',
  "running 1 test",
  "reported a skip",
  "finalCommit !== gitCommit",
  "workingTreeStatus().trim()",
  "writeAtomically({",
  "environment_sources",
  "reviewed_artifacts",
  "combined_test_output_sha256",
  "executed_cases",
]) {
  requireText("combined poison evidence runner", runner, marker);
}
for (const marker of [
  "database_url: value",
  "iggy_address: address",
  "username: username",
  "password: password",
  "raw_test_output",
  "payload:",
  "delivery_uuid",
  "source_offset",
  "ack_token",
  "schema_name",
  "stream_name",
]) {
  forbidText("retained packet projection", runner.slice(runner.indexOf("writeAtomically({")), marker);
}

if (failures.length === 0 && !existsSync(resolve(repoRoot, evidencePath))) {
  console.log(
    "Social Graph PostgreSQL/Iggy retained evidence verified: execution contract, clean-commit runner, exact-case gates, source hashing, privacy boundary, and runtime-pending status are locked; canonical execution JSON is absent.",
  );
  process.exit(0);
}

if (existsSync(resolve(repoRoot, evidencePath))) {
  const evidenceText = readFileSync(resolve(repoRoot, evidencePath), "utf8");
  let evidence;
  try {
    evidence = JSON.parse(evidenceText);
  } catch {
    fail("combined poison execution packet is not valid JSON");
  }

  if (evidence) {
    if (
      evidence.schema_version !== 1 ||
      evidence.module !== "social-graph" ||
      evidence.packet !== "index-raw-poison-postgres-iggy-runtime-evidence" ||
      evidence.status !== "postgres_iggy_runtime_executed"
    ) {
      fail("combined poison execution packet identity drift");
    }
    if (
      evidence.generated_from !== contractPath ||
      evidence.runner !== runnerPath ||
      evidence.verifier !== expectedVerifier ||
      evidence.working_tree_clean_before_run !== true
    ) {
      fail("combined poison execution provenance drift");
    }

    const commit = currentCommit();
    if (commit && evidence.git_commit !== commit) {
      fail("combined poison execution packet was generated from another commit");
    }
    validTimestamp(evidence.started_at, "started_at");
    validTimestamp(evidence.completed_at, "completed_at");
    if (
      typeof evidence.started_at === "string" &&
      typeof evidence.completed_at === "string" &&
      Date.parse(evidence.completed_at) < Date.parse(evidence.started_at)
    ) {
      fail("combined poison completion precedes start");
    }

    if (
      !sameValue(evidence.environment_sources, {
        database_url: "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL",
        iggy_address: "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS",
      })
    ) {
      fail("combined poison environment source metadata drift");
    }
    boundedLine(evidence.reviewed_artifacts?.postgresql, "reviewed PostgreSQL artifact");
    boundedLine(evidence.reviewed_artifacts?.iggy_server, "reviewed Iggy artifact");
    boundedLine(evidence.toolchain?.cargo, "cargo toolchain");
    boundedLine(evidence.toolchain?.rustc, "rustc toolchain");

    const hashes = currentSourceHashes();
    if (!sameValue(evidence.source_sha256, hashes)) {
      fail("combined poison execution source hashes are stale");
    }
    for (const [path, digest] of Object.entries(evidence.source_sha256 ?? {})) {
      validSha256(digest, `source hash ${path}`);
    }
    validSha256(evidence.combined_test_output_sha256, "combined test output hash");
    if (
      !Number.isSafeInteger(evidence.combined_test_output_bytes) ||
      evidence.combined_test_output_bytes <= 0
    ) {
      fail("combined test output byte count is invalid");
    }

    if (!Array.isArray(evidence.executed_cases) || evidence.executed_cases.length !== 2) {
      fail("combined poison execution must retain exactly two cases");
    } else {
      for (let index = 0; index < expectedCases.length; index += 1) {
        const retained = evidence.executed_cases[index];
        const required = contract.required_cases[index];
        if (
          retained.case !== expectedCases[index] ||
          retained.result !== "pass" ||
          !sameValue(retained.assertions, required.assertions) ||
          !sameValue(retained.command, expectedCommand(expectedCases[index]))
        ) {
          fail(`combined poison retained case drift: ${expectedCases[index]}`);
        }
        validSha256(retained.test_output_sha256, `${expectedCases[index]} output hash`);
        if (!Number.isSafeInteger(retained.test_output_bytes) || retained.test_output_bytes <= 0) {
          fail(`${expectedCases[index]} output byte count is invalid`);
        }
      }
    }

    const forbiddenKeys = new Set([
      "database_url_value",
      "iggy_address_value",
      "username",
      "password",
      "connection_string",
      "raw_test_output",
      "payload",
      "delivery_uuid",
      "source_offset",
      "ack_token",
      "schema_name",
      "stream_name",
    ]);
    for (const key of collectKeys(evidence)) {
      if (forbiddenKeys.has(key)) fail(`combined poison packet contains forbidden key: ${key}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Social Graph PostgreSQL/Iggy retained evidence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Social Graph PostgreSQL/Iggy retained evidence verified: current clean-commit provenance, reviewed service artifacts, current source hashes, exact commands, two all-pass ordering cases, bounded output digests, and privacy-safe packet projection are locked.",
);
