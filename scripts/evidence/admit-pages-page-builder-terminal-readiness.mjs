#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-readiness-admission-source.json";
const MAX_INPUT_BYTES = 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Pages/Page Builder terminal readiness admission failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function objectValue(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function arrayValue(value, label) {
  if (!Array.isArray(value)) fail(`${label} must be an array`);
  return value;
}

function canonicalCommit(value, label) {
  if (typeof value !== "string" || !COMMIT_PATTERN.test(value)) {
    fail(`${label} must be a lowercase 40-character Git SHA`);
  }
  return value;
}

function canonicalSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    fail(`${label} must be 64 lowercase hex characters`);
  }
  return value;
}

function canonicalRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !REPO_DIGEST_PATTERN.test(value)) {
    fail(`${label} must be REPOSITORY@sha256:<64 lowercase hex>`);
  }
  return value;
}

function canonicalIso(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    fail(`${label} must be a bounded timestamp`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be canonical ISO-8601 UTC`);
  }
  return { value, milliseconds };
}

function currentCommit() {
  return canonicalCommit(
    execFileSync("git", ["rev-parse", "HEAD"], { cwd: repoRoot, encoding: "utf8" }).trim(),
    "checkout HEAD",
  );
}

function parseArguments(argv) {
  const options = {};
  const accepted = new Set([
    "--execution",
    "--promotion-review",
    "--accessibility",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") {
      console.log(
        "usage: admit-pages-page-builder-terminal-readiness.mjs " +
          "--execution FILE --promotion-review FILE --accessibility FILE [--output FILE]",
      );
      process.exit(0);
    }
    if (!accepted.has(token)) fail(`unknown argument ${token}`);
    if (Object.hasOwn(options, token)) fail(`${token} may be supplied only once`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${token} requires a value`);
    options[token] = value;
    index += 1;
  }
  for (const required of ["--execution", "--promotion-review", "--accessibility"]) {
    if (!Object.hasOwn(options, required)) fail(`${required} is required`);
  }
  return options;
}

function resolveInput(candidate, label) {
  if (
    typeof candidate !== "string" ||
    candidate.length === 0 ||
    candidate.length > 16_384 ||
    /[\u0000\r\n]/u.test(candidate)
  ) {
    fail(`${label} path is invalid`);
  }
  return path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(process.cwd(), candidate);
}

function regularFile(location, label, maximumBytes = MAX_INPUT_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  return { bytes, size, sha256: sha256(bytes) };
}

function jsonInput(candidate, label) {
  const location = resolveInput(candidate, label);
  const record = regularFile(location, label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { document, ...record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function repoFile(relativePath, label, maximumBytes = MAX_SOURCE_BYTES) {
  if (typeof relativePath !== "string" || relativePath.length === 0) {
    fail(`${label} path is invalid`);
  }
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`${label} escapes repository root`);
  }
  return regularFile(absolute, label, maximumBytes);
}

function repoJson(relativePath, label) {
  const record = repoFile(relativePath, label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { document, ...record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function sourceHashes(contract) {
  const required = arrayValue(contract.required_source_files, "required_source_files");
  if (required.length === 0) fail("required_source_files is empty");
  return Object.fromEntries(
    required.map((relativePath) => [
      relativePath,
      repoFile(relativePath, `required source ${relativePath}`).sha256,
    ]),
  );
}

function outputPath(contract, requested) {
  const value = requested ?? contract.output?.default_path;
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 16_384 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("admission output must remain inside repository target/");
  }
  return absolute;
}

function writeAtomic(location, document) {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

function requireFalse(record, key, label) {
  if (record[key] !== false) fail(`${label}.${key} must remain false`);
}

function requireTrue(record, key, label) {
  if (record[key] !== true) fail(`${label}.${key} must be true`);
}

function sameArray(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function validateRegistry(contract) {
  const precondition = objectValue(contract.registry_precondition, "registry_precondition");
  const registryRecord = repoFile(precondition.path, "module readiness registry");
  const registry = registryRecord.bytes.toString("utf8");
  for (const [key, value] of [
    ["pages_row_required", precondition.pages_row_required],
    ["page_builder_row_required", precondition.page_builder_row_required],
    ["source_of_truth_rule_required", precondition.source_of_truth_rule_required],
  ]) {
    if (typeof value !== "string" || !registry.includes(value)) {
      fail(`registry precondition ${key} no longer matches current source`);
    }
  }
  const evidenceRule =
    "If status = `parity_verified` or `transport_verified`, the PR must contain verification evidence.";
  if (!registry.includes(evidenceRule)) {
    fail("registry terminal readiness verification-evidence rule is missing");
  }
  return {
    path: precondition.path,
    sha256: registryRecord.sha256,
    pages_ffa_current:
      contract.potential_terminal_targets.pages_ffa.required_current_registry_status,
    page_builder_fba_current:
      contract.potential_terminal_targets.page_builder_fba.required_current_registry_status,
  };
}

function collectPendingEvidence(value, pendingKey, pendingValue, prefix = "$") {
  const results = [];
  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      results.push(...collectPendingEvidence(entry, pendingKey, pendingValue, `${prefix}[${index}]`));
    });
    return results;
  }
  if (value === null || typeof value !== "object") return results;
  for (const [key, nested] of Object.entries(value)) {
    const current = `${prefix}.${key}`;
    if (key === pendingKey && nested === pendingValue) results.push(current);
    results.push(...collectPendingEvidence(nested, pendingKey, pendingValue, current));
  }
  return results;
}

function validateTerminalEvidenceInventoryGuard(contract) {
  const guard = objectValue(
    contract.terminal_evidence_inventory_guard,
    "terminal_evidence_inventory_guard",
  );
  const fbaRegistry = repoJson(guard.page_builder_fba_registry, "Page Builder FBA registry");
  if (fbaRegistry.document.status !== guard.page_builder_fba_required_current_status) {
    fail("Page Builder FBA registry current status no longer matches the admission contract");
  }
  const pendingPaths = collectPendingEvidence(
    fbaRegistry.document,
    guard.page_builder_fba_pending_key,
    guard.page_builder_fba_pending_value,
  ).sort();
  if (guard.page_builder_fba_current_pending_entries_must_be_nonzero === true && pendingPaths.length === 0) {
    fail("Page Builder FBA pending evidence is now empty; terminal inventory source must be actualized");
  }

  const pagesPlanRecord = repoFile(guard.pages_ffa_local_plan, "Pages local implementation plan");
  const pagesPlan = pagesPlanRecord.bytes.toString("utf8");
  const pagesPending = pagesPlan.includes(guard.pages_ffa_current_pending_marker);
  if (guard.pages_ffa_pending_marker_must_be_present === true && !pagesPending) {
    fail("Pages FFA pending marker is gone; terminal inventory source must be actualized");
  }
  if (guard.complete_terminal_evidence_inventory_source_defined !== false) {
    fail("terminal inventory guard must not claim a complete inventory source exists yet");
  }

  return {
    complete: false,
    owner_platform_review_ready: false,
    page_builder_fba: {
      registry_path: guard.page_builder_fba_registry,
      registry_sha256: fbaRegistry.sha256,
      current_status: fbaRegistry.document.status,
      pending_executed_evidence_count: pendingPaths.length,
      pending_executed_evidence_paths: pendingPaths,
      transport_verified_blocked: true,
    },
    pages_ffa: {
      local_plan_path: guard.pages_ffa_local_plan,
      local_plan_sha256: pagesPlanRecord.sha256,
      pending_marker: guard.pages_ffa_current_pending_marker,
      pending_marker_present: true,
      parity_verified_blocked: true,
    },
    required_future_inventory_format: guard.complete_terminal_evidence_inventory_format,
    future_inventory_source_defined: false,
  };
}

function validatePromotionReview(contract, input, head) {
  const document = input.document;
  const specification = objectValue(contract.promotion_review_input, "promotion_review_input");
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("promotion review is not the approved review packet required by readiness prerequisite admission");
  }
  if (canonicalCommit(document.source_commit, "promotion review source_commit") !== head) {
    fail("promotion review source_commit does not equal checkout HEAD");
  }
  const digest = canonicalRepoDigest(
    document.deployment_image_digest,
    "promotion review deployment_image_digest",
  );
  const review = objectValue(document.promotion_review, "promotion review decision");
  if (
    review.decision !== specification.decision_must_equal ||
    !sameArray(review.targets, specification.targets_must_equal)
  ) {
    fail("promotion review decision or targets drifted");
  }
  const boundaries = objectValue(document.boundaries, "promotion review boundaries");
  for (const key of ["ffa_promoted", "fba_promoted", "control_plane_or_rollout_mutated"]) {
    requireFalse(boundaries, key, "promotion review boundaries");
  }
  return { digest };
}

function validateExecution(contract, input, reviewInput, head, expectedDigest) {
  const document = input.document;
  const specification = objectValue(contract.promotion_execution_input, "promotion_execution_input");
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("promotion execution receipt is not a successful readiness-pending receipt");
  }
  if (canonicalCommit(document.source_commit, "promotion execution source_commit") !== head) {
    fail("promotion execution source_commit does not equal checkout HEAD");
  }
  const target = objectValue(document.target, "promotion execution target");
  const digest = canonicalRepoDigest(
    target.deployment_image_digest,
    "promotion execution deployment_image_digest",
  );
  if (digest !== expectedDigest) fail("promotion execution RepoDigest differs from promotion review");

  const retainedReview = objectValue(document.promotion_review, "promotion execution promotion_review");
  if (canonicalSha256(retainedReview.sha256, "retained promotion review sha256") !== reviewInput.sha256) {
    fail("promotion execution does not bind the supplied promotion-review packet");
  }
  if (retainedReview.decision !== "approve_ffa_fba_promotion_review") {
    fail("promotion execution retained review decision is not approved");
  }

  const mutation = objectValue(document.mutation, "promotion execution mutation");
  if (
    mutation.outcome !== "confirmed" ||
    mutation.control_plane_execution_confirmed !== true ||
    mutation.tenant_rollout_mutation_confirmed !== true
  ) {
    fail("promotion execution mutation is not a confirmed control-plane transition");
  }
  canonicalSha256(
    mutation.applied_settings_semantic_sha256,
    "promotion execution applied settings semantic sha256",
  );

  const postcondition = objectValue(document.postcondition, "promotion execution postcondition");
  if (postcondition.passed !== true || postcondition.current_provider_health_asserted !== false) {
    fail("promotion execution postcondition is not a successful non-health-asserting result");
  }

  const rollback = objectValue(document.rollback, "promotion execution rollback");
  if (
    rollback.attempted !== false ||
    rollback.outcome !== "not_required" ||
    rollback.net_target_state_retained !== true
  ) {
    fail("promotion execution must retain the successful target state without rollback");
  }

  const readiness = objectValue(document.readiness, "promotion execution readiness");
  requireFalse(readiness, "ffa_promoted", "promotion execution readiness");
  requireFalse(readiness, "fba_promoted", "promotion execution readiness");
  requireFalse(
    readiness,
    "registry_or_local_plan_status_mutated",
    "promotion execution readiness",
  );
  requireTrue(
    readiness,
    "separate_evidence_backed_governance_change_required",
    "promotion execution readiness",
  );

  const boundaries = objectValue(document.boundaries, "promotion execution boundaries");
  requireTrue(boundaries, "control_plane_change_executed", "promotion execution boundaries");
  requireTrue(boundaries, "tenant_rollout_mutated", "promotion execution boundaries");
  requireFalse(boundaries, "canonical_source_mutated", "promotion execution boundaries");
  requireFalse(boundaries, "readiness_board_mutated", "promotion execution boundaries");
  requireFalse(
    boundaries,
    "cryptographic_origin_to_repo_digest_binding_claimed",
    "promotion execution boundaries",
  );

  const generatedAt = canonicalIso(document.generated_at, "promotion execution generated_at");
  const nextDueAt = canonicalIso(
    retainedReview.observed_wave_next_due_at,
    "promotion execution observed_wave_next_due_at",
  );
  if (generatedAt.milliseconds > nextDueAt.milliseconds) {
    fail("promotion execution receipt was generated after the retained observed-Wave lease expired");
  }
  const reviewObserved = objectValue(
    reviewInput.document.observed_acceptance,
    "promotion review observed_acceptance",
  );
  if (reviewObserved.wave_next_due_at !== nextDueAt.value) {
    fail("promotion execution retained observed-Wave lease differs from the supplied promotion review");
  }

  const retainedSources = objectValue(document.source_sha256, "promotion execution source_sha256");
  const executionSourceContract = contract.promotion_execution_input.source_contract;
  const executionSourceHash = repoFile(
    executionSourceContract,
    "promotion execution source contract",
  ).sha256;
  if (
    canonicalSha256(
      retainedSources[executionSourceContract],
      `promotion execution source_sha256.${executionSourceContract}`,
    ) !== executionSourceHash
  ) {
    fail("promotion execution retained source contract hash does not match checkout");
  }
  return { digest, generatedAt: generatedAt.value, nextDueAt: nextDueAt.value };
}

function validateAccessibility(contract, input, head, expectedDigest) {
  const document = input.document;
  const specification = objectValue(contract.accessibility_input, "accessibility_input");
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("accessibility input is not a verified deployed browser packet");
  }
  if (canonicalCommit(document.source_commit, "accessibility source_commit") !== head) {
    fail("accessibility source_commit does not equal checkout HEAD");
  }
  const digest = canonicalRepoDigest(document.deployment_digest, "accessibility deployment_digest");
  if (digest !== expectedDigest) {
    fail("accessibility deployment RepoDigest differs from promotion execution");
  }
  const profiles = objectValue(document.profiles, "accessibility profiles");
  for (const profile of specification.required_profiles) {
    const record = objectValue(profiles[profile], `accessibility profile ${profile}`);
    if (record.passed !== true || record.critical_failures !== 0) {
      fail(`accessibility profile ${profile} did not pass without critical failures`);
    }
    if (!Number.isSafeInteger(record.page_count) || record.page_count < 2) {
      fail(`accessibility profile ${profile} page_count is below the required minimum`);
    }
  }
  requireTrue(document, "owner_review_required", "accessibility verification");
  requireTrue(document, "screen_reader_execution_pending", "accessibility verification");
  requireTrue(document, "wcag_conformance_not_claimed", "accessibility verification");
  requireTrue(document, "tenant_rollout_not_claimed", "accessibility verification");
  requireFalse(
    document,
    "cryptographic_origin_to_repo_digest_binding_claimed",
    "accessibility verification",
  );
  return { digest };
}

function main() {
  const args = parseArguments(process.argv.slice(2));
  const contractRecord = repoFile(contractPath, "terminal readiness admission contract");
  const contract = JSON.parse(contractRecord.bytes.toString("utf8"));
  if (
    contract.format !== "pages_page_builder_terminal_readiness_admission_source_v1" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("terminal readiness admission contract identity drifted");
  }
  if (
    contract.potential_terminal_targets?.pages_ffa?.potential_terminal_status !== "parity_verified" ||
    contract.potential_terminal_targets?.page_builder_fba?.potential_terminal_status !==
      "transport_verified" ||
    contract.potential_terminal_targets?.pages_ffa?.terminal_candidate_ready !== false ||
    contract.potential_terminal_targets?.page_builder_fba?.terminal_candidate_ready !== false
  ) {
    fail("potential terminal readiness target mapping drifted");
  }

  const head = currentCommit();
  const executionInput = jsonInput(args["--execution"], "promotion execution receipt");
  const reviewInput = jsonInput(args["--promotion-review"], "approved promotion review");
  const accessibilityInput = jsonInput(args["--accessibility"], "accessibility verification packet");

  const review = validatePromotionReview(contract, reviewInput, head);
  const execution = validateExecution(contract, executionInput, reviewInput, head, review.digest);
  validateAccessibility(contract, accessibilityInput, head, execution.digest);
  const registry = validateRegistry(contract);
  const terminalEvidenceInventory = validateTerminalEvidenceInventoryGuard(contract);
  const hashes = sourceHashes(contract);

  const output = outputPath(contract, args["--output"]);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    admitted_at: new Date().toISOString(),
    source_commit: head,
    deployment_image_digest: execution.digest,
    input_packets: {
      promotion_review: {
        bytes: reviewInput.size,
        sha256: reviewInput.sha256,
        raw_path_retained: false,
      },
      promotion_execution: {
        bytes: executionInput.size,
        sha256: executionInput.sha256,
        executed_at: execution.generatedAt,
        observed_wave_next_due_at: execution.nextDueAt,
        raw_path_retained: false,
      },
      accessibility_verification: {
        bytes: accessibilityInput.size,
        sha256: accessibilityInput.sha256,
        raw_path_retained: false,
      },
    },
    source_sha256: hashes,
    registry_precondition: registry,
    potential_terminal_targets: contract.potential_terminal_targets,
    non_targets: contract.non_targets,
    prerequisite_evidence: {
      promotion_review_approved: true,
      control_plane_execution_confirmed: true,
      rollout_postcondition_passed: true,
      successful_target_state_retained_without_rollback: true,
      accessibility_full_profile_verified: true,
      accessibility_read_only_profile_verified: true,
      accessibility_owner_review_still_required: true,
      screen_reader_execution_pending: true,
      wcag_conformance_not_claimed: true,
    },
    terminal_evidence_inventory: terminalEvidenceInventory,
    governance: {
      terminal_evidence_inventory_complete: false,
      owner_platform_review_ready: false,
      owner_review_required_after_complete_inventory: true,
      platform_review_required_after_complete_inventory: true,
      local_plan_and_registry_same_pr_sync_required: true,
      terminal_verification_evidence_required_in_change_pr: true,
      admission_is_not_terminal_evidence_completion: true,
      admission_is_not_approval: true,
      source_mutation_performed: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
    },
    privacy: {
      raw_input_paths_retained: false,
      tenant_identity_retained: false,
      api_origin_retained: false,
      raw_settings_retained: false,
      raw_graphql_or_browser_payload_retained: false,
      credentials_or_cookies_retained: false,
    },
  });

  console.log(
    `[admit-pages-page-builder-terminal-readiness] PASS source=${head} prerequisites=admitted terminal_inventory=pending owner_platform_review=blocked`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
