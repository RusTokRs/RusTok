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
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-evidence-inventory-source.json";
const MAX_INPUT_BYTES = 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;

function fail(message) {
  throw new Error(`Pages/Page Builder terminal evidence inventory failed: ${message}`);
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

function currentCommit() {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!COMMIT_PATTERN.test(value)) fail("checkout HEAD must be a full lowercase Git SHA");
  return value;
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

function repoFile(relativePath, label, maximumBytes = MAX_SOURCE_BYTES) {
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`${label} escapes repository root`);
  return regularFile(absolute, label, maximumBytes);
}

function repoJson(relativePath, label) {
  const record = repoFile(relativePath, label);
  let document;
  try {
    document = JSON.parse(record.bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
  objectValue(document, label);
  return { document, ...record };
}

function inputJson(candidate, label, maximumBytes = MAX_INPUT_BYTES) {
  if (
    typeof candidate !== "string" ||
    candidate.length === 0 ||
    candidate.length > 16_384 ||
    /[\u0000\r\n]/u.test(candidate)
  ) {
    fail(`${label} path is invalid`);
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(process.cwd(), candidate);
  const record = regularFile(absolute, label, maximumBytes);
  let document;
  try {
    document = JSON.parse(record.bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
  objectValue(document, label);
  return { document, ...record };
}

function parseArguments(argv) {
  const result = {};
  const accepted = new Set(["--prerequisite-admission", "--output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") {
      console.log(
        "usage: inventory-pages-page-builder-terminal-readiness.mjs --prerequisite-admission FILE [--output FILE]",
      );
      process.exit(0);
    }
    if (!accepted.has(token)) fail(`unknown argument ${token}`);
    if (Object.hasOwn(result, token)) fail(`${token} may be supplied only once`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${token} requires a value`);
    result[token] = value;
    index += 1;
  }
  if (!Object.hasOwn(result, "--prerequisite-admission")) {
    fail("--prerequisite-admission is required");
  }
  return result;
}

function outputPath(contract, requested) {
  const value = requested ?? contract.output?.default_path;
  if (typeof value !== "string" || value.length === 0 || value.length > 16_384) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("inventory output must remain inside repository target/");
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

function pointerToken(value) {
  return String(value).replaceAll("~", "~0").replaceAll("/", "~1");
}

export function collectPendingEvidence(value, pendingKey = "executed_evidence", pendingValue = "pending", prefix = "") {
  const results = [];
  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      results.push(
        ...collectPendingEvidence(entry, pendingKey, pendingValue, `${prefix}/${pointerToken(index)}`),
      );
    });
    return results;
  }
  if (value === null || typeof value !== "object") return results;
  for (const [key, nested] of Object.entries(value)) {
    const current = `${prefix}/${pointerToken(key)}`;
    if (key === pendingKey && nested === pendingValue) results.push(current);
    results.push(...collectPendingEvidence(nested, pendingKey, pendingValue, current));
  }
  return results;
}

export function evaluateInventory({ prerequisiteValid, pendingEvidencePaths, pagesRolloutPending }) {
  const pageBuilderFbaComplete = pendingEvidencePaths.length === 0;
  const pagesFfaComplete = !pagesRolloutPending;
  const complete = prerequisiteValid && pageBuilderFbaComplete && pagesFfaComplete;
  return {
    complete,
    owner_platform_review_ready: complete,
    page_builder_fba_complete: pageBuilderFbaComplete,
    pages_ffa_complete: pagesFfaComplete,
  };
}

function validateRegistry(contract) {
  const source = repoFile(contract.authorities.central_registry, "central module readiness registry");
  const text = source.bytes.toString("utf8");
  const precondition = objectValue(contract.readiness_precondition, "readiness_precondition");
  for (const marker of [
    precondition.pages_row_required,
    precondition.page_builder_row_required,
    precondition.source_of_truth_rule_required,
    "If status = `parity_verified` or `transport_verified`, the PR must contain verification evidence.",
  ]) {
    if (typeof marker !== "string" || !text.includes(marker)) {
      fail(`central readiness precondition drifted: ${marker}`);
    }
  }
  return { path: contract.authorities.central_registry, sha256: source.sha256 };
}

function validatePrerequisite(contract, input, head) {
  const specification = objectValue(contract.predecessor, "predecessor");
  const document = input.document;
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("prerequisite admission packet identity drifted");
  }
  if (document.source_commit !== head || !COMMIT_PATTERN.test(document.source_commit)) {
    fail("prerequisite admission source_commit does not equal checkout HEAD");
  }

  const retainedSources = objectValue(document.source_sha256, "prerequisite source_sha256");
  const admissionSourcePath = contract.authorities.prerequisite_admission_source;
  const admissionSourceHash = repoFile(admissionSourcePath, "prerequisite admission source").sha256;
  if (retainedSources[admissionSourcePath] !== admissionSourceHash) {
    fail("prerequisite retained admission-source hash does not match checkout");
  }
  const inventorySourceHash = repoFile(contractPath, "terminal evidence inventory source").sha256;
  if (retainedSources[contractPath] !== inventorySourceHash) {
    fail("prerequisite retained inventory-source hash does not match checkout");
  }

  const governance = objectValue(document.governance, "prerequisite governance");
  if (
    governance.terminal_evidence_inventory_complete !== false ||
    governance.owner_platform_review_ready !== false ||
    governance.source_mutation_performed !== false ||
    governance.pages_ffa_promoted !== false ||
    governance.page_builder_fba_promoted !== false
  ) {
    fail("prerequisite admission contains a terminal-readiness overclaim");
  }
  const inventory = objectValue(document.terminal_evidence_inventory, "prerequisite terminal_evidence_inventory");
  if (inventory.complete !== false || inventory.owner_platform_review_ready !== false) {
    fail("prerequisite packet must retain incomplete terminal inventory state");
  }
  if (
    inventory.future_inventory_source_defined !== true ||
    inventory.future_inventory_source_path !== contractPath ||
    inventory.future_inventory_source_sha256 !== inventorySourceHash
  ) {
    fail("prerequisite terminal-inventory source binding does not match checkout");
  }
  return inventory;
}

function sourceHashes(contract) {
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("required_source_files must be non-empty");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => [
      relativePath,
      repoFile(relativePath, `required source ${relativePath}`).sha256,
    ]),
  );
}

function buildCanonicalInventory(contract, predecessorInventory) {
  const fba = repoJson(contract.authorities.page_builder_fba_registry, "Page Builder FBA registry");
  if (fba.document.status !== contract.page_builder_fba_inventory.required_registry_status_before_governance) {
    fail("Page Builder FBA registry status changed before terminal governance review");
  }
  const pendingEvidencePaths = collectPendingEvidence(
    fba.document,
    contract.page_builder_fba_inventory.recursive_blocker_key,
    contract.page_builder_fba_inventory.recursive_blocker_value,
  ).sort();
  if (pendingEvidencePaths.length > contract.page_builder_fba_inventory.maximum_blocker_paths) {
    fail("Page Builder FBA blocker inventory exceeds the configured bound");
  }

  const pagesPlan = repoFile(contract.authorities.pages_local_plan, "Pages local plan");
  const pagesPlanText = pagesPlan.bytes.toString("utf8");
  const pagesRolloutPending = pagesPlanText.includes(contract.pages_ffa_inventory.blocking_status_marker);

  const predecessorCount = predecessorInventory.page_builder_fba?.pending_executed_evidence_count;
  if (!Number.isSafeInteger(predecessorCount) || predecessorCount !== pendingEvidencePaths.length) {
    fail("prerequisite Page Builder FBA blocker count does not match same-source canonical registry");
  }
  if (predecessorInventory.pages_ffa?.pending_marker_present !== pagesRolloutPending) {
    fail("prerequisite Pages rollout blocker fact does not match same-source canonical plan");
  }

  const evaluation = evaluateInventory({
    prerequisiteValid: true,
    pendingEvidencePaths,
    pagesRolloutPending,
  });
  return {
    evaluation,
    pageBuilderFba: {
      registry_path: contract.authorities.page_builder_fba_registry,
      registry_sha256: fba.sha256,
      current_status: fba.document.status,
      pending_executed_evidence_count: pendingEvidencePaths.length,
      pending_executed_evidence_paths: pendingEvidencePaths,
      complete: evaluation.page_builder_fba_complete,
      transport_verified_blocked: !evaluation.page_builder_fba_complete,
    },
    pagesFfa: {
      local_plan_path: contract.authorities.pages_local_plan,
      local_plan_sha256: pagesPlan.sha256,
      blocking_marker: contract.pages_ffa_inventory.blocking_status_marker,
      blocking_marker_present: pagesRolloutPending,
      complete: evaluation.pages_ffa_complete,
      parity_verified_blocked: !evaluation.pages_ffa_complete,
    },
  };
}

function main() {
  const args = parseArguments(process.argv.slice(2));
  const contractRecord = repoJson(contractPath, "terminal evidence inventory source contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "pages_page_builder_terminal_evidence_inventory_source_v1" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("terminal evidence inventory source contract identity drifted");
  }

  const head = currentCommit();
  const prerequisite = inputJson(
    args["--prerequisite-admission"],
    "terminal readiness prerequisite admission",
    contract.predecessor.maximum_bytes,
  );
  const predecessorInventory = validatePrerequisite(contract, prerequisite, head);
  const registry = validateRegistry(contract);
  const canonical = buildCanonicalInventory(contract, predecessorInventory);
  const hashes = sourceHashes(contract);
  const status = canonical.evaluation.complete
    ? contract.completion.complete_status
    : contract.completion.incomplete_status;

  const output = outputPath(contract, args["--output"]);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: contract.output.format,
    status,
    generated_at: new Date().toISOString(),
    source_commit: head,
    predecessor: {
      bytes: prerequisite.size,
      sha256: prerequisite.sha256,
      raw_path_retained: false,
    },
    source_sha256: hashes,
    registry_precondition: registry,
    page_builder_fba: canonical.pageBuilderFba,
    pages_ffa: canonical.pagesFfa,
    complete: canonical.evaluation.complete,
    owner_platform_review_ready: canonical.evaluation.owner_platform_review_ready,
    governance: {
      inventory_is_not_owner_approval: true,
      inventory_is_not_platform_approval: true,
      source_mutation_performed: false,
      registry_mutated: false,
      local_plan_mutated: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
      local_plan_and_registry_same_pr_sync_required_after_approval: true,
      verification_evidence_required_in_terminal_change_pr: true,
    },
    privacy: {
      raw_predecessor_path_retained: false,
      tenant_identity_retained: false,
      api_origin_retained: false,
      raw_settings_retained: false,
      raw_graphql_or_browser_payload_retained: false,
      credentials_or_cookies_retained: false,
    },
  });

  console.log(
    `[inventory-pages-page-builder-terminal-readiness] PASS source=${head} fba_pending=${canonical.pageBuilderFba.pending_executed_evidence_count} pages_rollout_pending=${canonical.pagesFfa.blocking_marker_present} complete=${canonical.evaluation.complete} owner_platform_review_ready=${canonical.evaluation.owner_platform_review_ready}`,
  );
}

const invokedDirectly =
  typeof process.argv[1] === "string" && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
