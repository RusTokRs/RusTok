#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-execution-contract.json";
const sourceContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-runtime-source.json";
const runnerPath =
  "scripts/evidence/capture-iggy-dlq-duplicate-fair-window-external-scan.mjs";
const evidencePath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-execution.json";
const expectedVerifier =
  "scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs";
const expectedCase =
  "fair_window_scans_each_partition_and_differs_from_global_budget";

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const sourceContract = JSON.parse(
  readFileSync(resolve(repoRoot, sourceContractPath), "utf8"),
);
const runner = readFileSync(resolve(repoRoot, runnerPath), "utf8");
const failures = [];

const expectedCommand = {
  program: "cargo",
  args: [
    "test",
    "-p",
    "rustok-iggy",
    "--features",
    "iggy",
    "--test",
    "dlq_duplicate_fair_window_external_scan",
    "--",
    expectedCase,
    "--exact",
    "--nocapture",
    "--test-threads=1",
  ],
};
const expectedFairSummary = {
  total_messages: 4,
  unique_message_ids: 2,
  duplicate_messages: 2,
  duplicate_groups: 2,
  conflicting_payload_groups: 1,
  max_copies_per_message_id: 2,
  has_physical_duplicates: true,
  has_identity_conflicts: true,
  requires_manual_review: true,
};
const expectedGlobalSummary = {
  total_messages: 4,
  unique_message_ids: 3,
  duplicate_messages: 1,
  duplicate_groups: 1,
  conflicting_payload_groups: 0,
  max_copies_per_message_id: 2,
  has_physical_duplicates: true,
  has_identity_conflicts: false,
  requires_manual_review: false,
};
const expectedComparison = {
  fair_summary_repeated_equal: true,
  global_summary_differs_from_fair: true,
};
const expectedOffsets = {
  partitions_checked: 2,
  before_fixture_publication_stored_offset_count: 0,
  after_first_fair_window_stored_offset_count: 0,
  after_global_request_stored_offset_count: 0,
  after_second_fair_window_stored_offset_count: 0,
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
  "environment_sources",
  "reviewed_artifacts",
  "reviewed_configuration",
  "toolchain",
  "source_sha256",
  "executed_case",
].sort();
const expectedCaseKeys = [
  "name",
  "result",
  "command",
  "required_fair_summary",
  "required_global_summary",
  "required_comparison",
  "required_offset_observations",
  "test_output_sha256",
  "test_output_bytes",
].sort();

function fail(message) {
  failures.push(message);
}

function same(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function requireText(name, text, marker) {
  if (!text.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fileSha256(relativePath) {
  const absolutePath = resolve(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    fail(`source file is missing: ${relativePath}`);
    return null;
  }
  return sha256(readFileSync(absolutePath));
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

function boundedLine(value, field, maximumLength = 256) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    value.trim() !== value ||
    /[\r\n\u0000-\u001f\u007f]/u.test(value)
  ) {
    fail(`${field} is not a bounded one-line value`);
  }
}

function boundedArtifact(value, field) {
  boundedLine(value, field, 256);
  if (
    typeof value === "string" &&
    (value.includes("://") ||
      value.includes("@") ||
      /^\[[0-9a-fA-F:]+\]:\d+$/u.test(value) ||
      /^[A-Za-z0-9._-]+:\d+$/u.test(value))
  ) {
    fail(`${field} is endpoint-shaped rather than an artifact label`);
  }
}

function collectKeys(value, keys = []) {
  if (Array.isArray(value)) {
    for (const entry of value) collectKeys(entry, keys);
  } else if (value && typeof value === "object") {
    for (const [key, entry] of Object.entries(value)) {
      keys.push(key);
      collectKeys(entry, keys);
    }
  }
  return keys;
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "iggy" ||
  contract.packet !==
    "dlq-duplicate-fair-window-external-scan-execution-contract" ||
  contract.status !== "runtime_execution_contract_locked" ||
  contract.source_contract !== sourceContractPath ||
  contract.runner !== runnerPath ||
  contract.verifier !== expectedVerifier ||
  contract.evidence_path !== evidencePath ||
  contract.evidence_status !== "runtime_execution_pending" ||
  contract.case !== expectedCase
) {
  fail("fair-window retained execution contract identity or path drift");
}
if (!same(contract.command, expectedCommand)) {
  fail("fair-window retained exact command drift");
}
if (!same(contract.required_fair_summary, expectedFairSummary)) {
  fail("fair-window retained fair summary drift");
}
if (!same(contract.required_global_summary, expectedGlobalSummary)) {
  fail("fair-window retained global summary drift");
}
if (!same(contract.required_comparison, expectedComparison)) {
  fail("fair-window retained comparison drift");
}
if (!same(contract.required_offset_observations, expectedOffsets)) {
  fail("fair-window retained aggregate absent-offset drift");
}
if (
  sourceContract.status !== "source_complete_runtime_pending" ||
  sourceContract.execution_status !== "not_run" ||
  sourceContract.test !==
    "crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs" ||
  sourceContract.case !== expectedCase ||
  sourceContract.retained_execution?.contract !== contractPath ||
  sourceContract.retained_execution?.runner !== runnerPath ||
  sourceContract.retained_execution?.verifier !== expectedVerifier ||
  sourceContract.retained_execution?.evidence_path !== evidencePath ||
  sourceContract.retained_execution?.canonical_packet_present !== false ||
  sourceContract.retained_execution?.no_clobber_write !== true
) {
  fail("fair-window runtime source contract retained relationship drift");
}
if (
  contract.reviewed_configuration?.section !== "system.message_deduplication" ||
  contract.reviewed_configuration?.required_enabled !== false ||
  !same(contract.reviewed_configuration?.retain_only, [
    "section",
    "enabled",
    "canonical_sha256",
  ]) ||
  contract.reviewed_configuration?.config_path_outside_repository !== true ||
  contract.reviewed_configuration?.full_content_retained !== false ||
  contract.reviewed_configuration?.full_file_sha256_retained !== false
) {
  fail("fair-window reviewed configuration boundary drift");
}

for (const marker of [
  "function strictOneLine(value, field, maximumLength = 256)",
  "function commandOutputLine(value, field, maximumLength = 256)",
  "validateAddress(process.env[addressEnvironment]",
  "validateCredentialsPair();",
  "externalConfigPath(",
  "must point outside the repository",
  "reviewedArtifact(",
  "system.message_deduplication",
  'enabledRaw !== "false"',
  "canonical_sha256: sha256(JSON.stringify(canonical))",
  "ensureCleanCommit()",
  "sourceHashes()",
  'runChecked("cargo", ["--version"])',
  'runChecked("rustc", ["--version"])',
  "running 1 test",
  "exact case reported a skip",
  "finalCommit !== gitCommit",
  "workingTreeStatus().trim()",
  "writeNoClobber({",
  'flag: "wx"',
  "linkSync(temporaryPath, outputPath)",
  "required_fair_summary: contract.required_fair_summary",
  "required_global_summary: contract.required_global_summary",
  "required_comparison: contract.required_comparison",
  "required_offset_observations: contract.required_offset_observations",
  "test_output_sha256: sha256(output)",
]) {
  requireText("fair-window retained runner", runner, marker);
}

if (failures.length === 0 && !existsSync(resolve(repoRoot, evidencePath))) {
  console.log(
    "Iggy fair-window retained evidence verified: clean-commit exact-case capture, reviewed dedup-disabled configuration projection, current source hashes, fair/global assertions, two-partition absent-offset aggregates, privacy exclusions, and no-clobber publication are locked; canonical execution JSON is absent.",
  );
  process.exit(0);
}

if (existsSync(resolve(repoRoot, evidencePath))) {
  let evidence;
  try {
    evidence = JSON.parse(readFileSync(resolve(repoRoot, evidencePath), "utf8"));
  } catch {
    fail("fair-window execution packet is not valid JSON");
  }

  if (evidence) {
    if (!same(Object.keys(evidence).sort(), expectedTopLevelKeys)) {
      fail("fair-window execution packet top-level keys drifted");
    }
    if (
      evidence.schema_version !== 1 ||
      evidence.module !== "iggy" ||
      evidence.packet !==
        "dlq-duplicate-fair-window-external-scan-runtime-evidence" ||
      evidence.status !==
        "external_iggy_fair_window_duplicate_scan_runtime_executed" ||
      evidence.generated_from !== contractPath ||
      evidence.runner !== runnerPath ||
      evidence.verifier !== expectedVerifier ||
      evidence.working_tree_clean_before_run !== true
    ) {
      fail("fair-window execution packet identity or provenance drift");
    }

    const commit = currentCommit();
    if (commit && evidence.git_commit !== commit) {
      fail("fair-window execution packet was generated from another commit");
    }
    validTimestamp(evidence.started_at, "started_at");
    validTimestamp(evidence.completed_at, "completed_at");
    if (
      typeof evidence.started_at === "string" &&
      typeof evidence.completed_at === "string" &&
      Date.parse(evidence.completed_at) < Date.parse(evidence.started_at)
    ) {
      fail("fair-window completion precedes start");
    }

    if (
      !same(evidence.environment_sources, {
        address_environment: "RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_ADDRESS",
        configuration_path_environment:
          "RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_CONFIG_PATH",
        server_artifact_environment:
          "RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_SERVER_ARTIFACT",
        username_environment: "RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_USERNAME",
        password_environment: "RUSTOK_IGGY_FAIR_WINDOW_SCAN_TEST_PASSWORD",
      })
    ) {
      fail("fair-window environment source metadata drift");
    }
    if (!same(Object.keys(evidence.reviewed_artifacts ?? {}), ["iggy_server"])) {
      fail("fair-window reviewed artifact keys drift");
    }
    boundedArtifact(evidence.reviewed_artifacts?.iggy_server, "reviewed Iggy artifact");
    boundedLine(evidence.toolchain?.cargo, "cargo toolchain");
    boundedLine(evidence.toolchain?.rustc, "rustc toolchain");

    if (
      !same(Object.keys(evidence.reviewed_configuration ?? {}).sort(), [
        "canonical_sha256",
        "enabled",
        "section",
      ]) ||
      evidence.reviewed_configuration?.section !==
        "system.message_deduplication" ||
      evidence.reviewed_configuration?.enabled !== false
    ) {
      fail("fair-window reviewed configuration shape drift");
    } else {
      const canonical = {
        section: evidence.reviewed_configuration.section,
        enabled: evidence.reviewed_configuration.enabled,
      };
      validSha256(
        evidence.reviewed_configuration.canonical_sha256,
        "reviewed configuration canonical hash",
      );
      if (
        evidence.reviewed_configuration.canonical_sha256 !==
        sha256(JSON.stringify(canonical))
      ) {
        fail("fair-window canonical configuration digest mismatch");
      }
    }

    const hashes = currentSourceHashes();
    if (!same(evidence.source_sha256, hashes)) {
      fail("fair-window execution source hashes are stale");
    }
    for (const [path, digest] of Object.entries(evidence.source_sha256 ?? {})) {
      validSha256(digest, `source hash ${path}`);
    }

    if (!same(Object.keys(evidence.executed_case ?? {}).sort(), expectedCaseKeys)) {
      fail("fair-window executed case keys drifted");
    }
    if (
      evidence.executed_case?.name !== expectedCase ||
      evidence.executed_case?.result !== "pass" ||
      !same(evidence.executed_case?.command, expectedCommand) ||
      !same(evidence.executed_case?.required_fair_summary, expectedFairSummary) ||
      !same(evidence.executed_case?.required_global_summary, expectedGlobalSummary) ||
      !same(evidence.executed_case?.required_comparison, expectedComparison) ||
      !same(evidence.executed_case?.required_offset_observations, expectedOffsets)
    ) {
      fail("fair-window executed case assertions or result drift");
    }
    validSha256(
      evidence.executed_case?.test_output_sha256,
      "fair-window test output hash",
    );
    if (
      !Number.isSafeInteger(evidence.executed_case?.test_output_bytes) ||
      evidence.executed_case.test_output_bytes <= 0
    ) {
      fail("fair-window test output byte count is invalid");
    }

    const forbiddenKeys = new Set(contract.privacy_exclusions ?? []);
    for (const key of collectKeys(evidence)) {
      if (forbiddenKeys.has(key)) {
        fail(`fair-window execution packet contains forbidden key: ${key}`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error("Iggy fair-window retained verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy fair-window retained evidence verified: current commit/source hashes, reviewed dedup-disabled configuration digest, bounded Iggy/toolchain labels, exact all-pass case, fair/global summary assertions, four aggregate absent-offset checkpoints over two partitions, output digest, no-clobber provenance, and identifier-free packet projection are locked.",
);
