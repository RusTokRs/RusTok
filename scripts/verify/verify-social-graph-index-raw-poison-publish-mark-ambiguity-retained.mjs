#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution-contract.json";
const sourceContractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-source.json";
const runnerPath =
  "scripts/evidence/capture-social-graph-index-raw-poison-publish-mark-ambiguity.mjs";
const evidencePath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-execution.json";
const expectedVerifier =
  "scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity-retained.mjs";
const expectedCases = [
  "dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate",
  "dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate",
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
    "index_raw_poison_publish_mark_ambiguity",
    "--",
  ],
  args_after_case: ["--exact", "--nocapture", "--test-threads=1"],
};
const expectedTopLevelKeys = [
  "schema_version",
  "module",
  "packet",
  "status",
  "generated_from",
  "runner",
  "verifier",
  "git_commit",
  "working_tree_clean_before_run",
  "started_at",
  "completed_at",
  "lease_reclaim_wait_milliseconds",
  "environment_sources",
  "reviewed_artifacts",
  "toolchain",
  "source_sha256",
  "combined_test_output_sha256",
  "combined_test_output_bytes",
  "executed_scenarios",
].sort();

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

function validSha256(value, field) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    fail(`${field} is not a lowercase SHA-256`);
  }
}

function validTimestamp(value, field) {
  if (typeof value !== "string" || Number.isNaN(Date.parse(value))) {
    fail(`${field} is not a valid timestamp`);
  }
}

function boundedArtifact(value, field) {
  if (
    typeof value !== "string" ||
    value.trim() !== value ||
    value.length === 0 ||
    value.length > 256 ||
    /[\r\n\u0000-\u001f\u007f]/u.test(value) ||
    value.includes("://") ||
    value.includes("@") ||
    /^\[[0-9a-fA-F:]+\]:\d+$/u.test(value) ||
    /^[A-Za-z0-9._-]+:\d+$/u.test(value)
  ) {
    fail(`${field} is not a bounded reviewed artifact label`);
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
  contract.packet !== "index-raw-poison-publish-mark-ambiguity-execution-contract" ||
  contract.status !== "runtime_execution_contract_locked" ||
  contract.source_contract !== sourceContractPath ||
  contract.runner !== runnerPath ||
  contract.verifier !== expectedVerifier ||
  contract.evidence_path !== evidencePath ||
  contract.evidence_status !== "runtime_execution_pending"
) {
  fail("publish/mark ambiguity execution contract identity or path drift");
}
if (!sameValue(contract.command_template, expectedCommandTemplate)) {
  fail("publish/mark ambiguity command template drift");
}
if (!sameValue(contract.scenarios?.map((scenario) => scenario.case), expectedCases)) {
  fail("publish/mark ambiguity exact case order drift");
}
if (contract.lease_reclaim_wait_milliseconds !== 1500) {
  fail("publish/mark ambiguity lease reclaim wait drift");
}
if (
  sourceContract.status !== "source_complete_runtime_pending" ||
  sourceContract.execution_status !== "not_run" ||
  sourceContract.verifier !==
    "scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity.mjs"
) {
  fail("publish/mark ambiguity source contract must remain source-complete and unexecuted");
}

for (const marker of [
  "function oneLine(value, field, maximumLength = 256)",
  "value.trim() !== value",
  "/[\\r\\n]/u.test(value)",
  "process.env[contract.database_environment] ??",
  "validateDatabaseUrl();",
  "validateCredentialsPair();",
  "validateAddress(process.env[scenario.address_env]",
  "externalConfigPath(",
  "must point outside the repository",
  "reviewedArtifact(",
  "system.message_deduplication",
  "configuration.expiry_milliseconds <= contract.lease_reclaim_wait_milliseconds",
  "ambiguity execution requires two distinct broker addresses",
  "ambiguity execution requires two distinct reviewed config files",
  "ensureCleanCommit()",
  "sourceHashes()",
  '"--exact"',
  "running 1 test",
  "reported a skip",
  "finalCommit !== gitCommit",
  "workingTreeStatus().trim()",
  "writeAtomically({",
  "environment_sources",
  "reviewed_artifacts",
  "combined_test_output_sha256",
  "executed_scenarios",
]) {
  requireText("publish/mark ambiguity retained runner", runner, marker);
}

if (failures.length === 0 && !existsSync(resolve(repoRoot, evidencePath))) {
  console.log(
    "Social Graph publish/mark ambiguity retained evidence verified: execution contract, reviewed dedup configuration gates, strict one-line metadata, clean-commit runner, exact-case execution, current source hashing, privacy projection, and runtime-pending status are locked; canonical execution JSON is absent.",
  );
  process.exit(0);
}

if (existsSync(resolve(repoRoot, evidencePath))) {
  let evidence;
  try {
    evidence = JSON.parse(readFileSync(resolve(repoRoot, evidencePath), "utf8"));
  } catch {
    fail("publish/mark ambiguity execution packet is not valid JSON");
  }

  if (evidence) {
    if (!sameValue(Object.keys(evidence).sort(), expectedTopLevelKeys)) {
      fail("publish/mark ambiguity execution packet top-level keys drifted");
    }
    if (
      evidence.schema_version !== 1 ||
      evidence.module !== "social-graph" ||
      evidence.packet !== "index-raw-poison-publish-mark-ambiguity-runtime-evidence" ||
      evidence.status !== "postgres_iggy_ambiguity_runtime_executed" ||
      evidence.generated_from !== contractPath ||
      evidence.runner !== runnerPath ||
      evidence.verifier !== expectedVerifier ||
      evidence.working_tree_clean_before_run !== true ||
      evidence.lease_reclaim_wait_milliseconds !== 1500
    ) {
      fail("publish/mark ambiguity execution packet identity or provenance drift");
    }

    const commit = currentCommit();
    if (commit && evidence.git_commit !== commit) {
      fail("publish/mark ambiguity execution packet was generated from another commit");
    }
    validTimestamp(evidence.started_at, "started_at");
    validTimestamp(evidence.completed_at, "completed_at");
    if (
      typeof evidence.started_at === "string" &&
      typeof evidence.completed_at === "string" &&
      Date.parse(evidence.completed_at) < Date.parse(evidence.started_at)
    ) {
      fail("publish/mark ambiguity completion precedes start");
    }

    if (
      !sameValue(evidence.environment_sources, {
        database_url: "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL",
        postgresql_artifact:
          "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_POSTGRES_ARTIFACT",
      })
    ) {
      fail("publish/mark ambiguity environment source metadata drift");
    }
    if (!sameValue(Object.keys(evidence.reviewed_artifacts ?? {}), ["postgresql"])) {
      fail("publish/mark ambiguity reviewed artifact keys drift");
    }
    boundedArtifact(evidence.reviewed_artifacts?.postgresql, "reviewed PostgreSQL artifact");
    boundedArtifact(evidence.toolchain?.cargo, "cargo toolchain");
    boundedArtifact(evidence.toolchain?.rustc, "rustc toolchain");

    const hashes = currentSourceHashes();
    if (!sameValue(evidence.source_sha256, hashes)) {
      fail("publish/mark ambiguity execution source hashes are stale");
    }
    for (const [path, digest] of Object.entries(evidence.source_sha256 ?? {})) {
      validSha256(digest, `source hash ${path}`);
    }
    validSha256(evidence.combined_test_output_sha256, "combined test output hash");
    if (
      !Number.isSafeInteger(evidence.combined_test_output_bytes) ||
      evidence.combined_test_output_bytes <= 0
    ) {
      fail("publish/mark ambiguity combined output byte count is invalid");
    }

    if (
      !Array.isArray(evidence.executed_scenarios) ||
      evidence.executed_scenarios.length !== 2
    ) {
      fail("publish/mark ambiguity execution must retain exactly two scenarios");
    } else {
      for (let index = 0; index < expectedCases.length; index += 1) {
        const retained = evidence.executed_scenarios[index];
        const required = contract.scenarios[index];
        if (
          retained.case !== expectedCases[index] ||
          retained.result !== "pass" ||
          retained.address_source_env !== required.address_env ||
          retained.configuration_source_env !== required.config_path_env ||
          retained.server_artifact_source_env !== required.server_artifact_env ||
          !sameValue(
            retained.expected_partition_message_counts,
            required.expected_partition_message_counts,
          ) ||
          !sameValue(retained.command, expectedCommand(expectedCases[index]))
        ) {
          fail(`publish/mark ambiguity retained scenario drift: ${expectedCases[index]}`);
        }

        boundedArtifact(retained.server_artifact, `${expectedCases[index]} server artifact`);
        const reviewed = retained.reviewed_configuration;
        if (
          !sameValue(Object.keys(reviewed ?? {}).sort(), [
            "canonical_sha256",
            "enabled",
            "expiry",
            "expiry_milliseconds",
            "max_entries",
            "section",
          ]) ||
          reviewed.section !== "system.message_deduplication"
        ) {
          fail(`${expectedCases[index]} reviewed configuration shape drift`);
        } else {
          const canonical = {
            section: reviewed.section,
            enabled: reviewed.enabled,
            max_entries: reviewed.max_entries,
            expiry: reviewed.expiry,
            expiry_milliseconds: reviewed.expiry_milliseconds,
          };
          validSha256(reviewed.canonical_sha256, `${expectedCases[index]} config hash`);
          if (reviewed.canonical_sha256 !== sha256(JSON.stringify(canonical))) {
            fail(`${expectedCases[index]} canonical configuration digest mismatch`);
          }
          if (index === 0) {
            if (
              reviewed.enabled !== true ||
              !Number.isSafeInteger(reviewed.max_entries) ||
              reviewed.max_entries < 1 ||
              !Number.isSafeInteger(reviewed.expiry_milliseconds) ||
              reviewed.expiry_milliseconds <= 1500
            ) {
              fail("dedup-enabled reviewed configuration does not cover the recovery wait");
            }
          } else if (reviewed.enabled !== false) {
            fail("dedup-disabled reviewed configuration must retain enabled=false");
          }
        }

        validSha256(retained.test_output_sha256, `${expectedCases[index]} output hash`);
        if (!Number.isSafeInteger(retained.test_output_bytes) || retained.test_output_bytes <= 0) {
          fail(`${expectedCases[index]} output byte count is invalid`);
        }
      }
    }

    const forbiddenKeys = new Set([
      "database_url_value",
      "broker_address",
      "address",
      "config_path",
      "username",
      "password",
      "connection_string",
      "full_config_content",
      "full_config_sha256",
      "raw_test_output",
      "payload",
      "source_offset",
      "delivery_uuid",
      "ack_token",
      "schema_name",
      "stream_name",
    ]);
    for (const key of collectKeys(evidence)) {
      if (forbiddenKeys.has(key)) {
        fail(`publish/mark ambiguity packet contains forbidden key: ${key}`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error("Social Graph publish/mark ambiguity retained verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Social Graph publish/mark ambiguity retained evidence verified: current commit/source hashes, reviewed PostgreSQL and Iggy artifacts, enabled expiry coverage, disabled duplicate behavior, two exact all-pass cases, bounded output digests, and privacy-safe packet projection are locked.",
);
