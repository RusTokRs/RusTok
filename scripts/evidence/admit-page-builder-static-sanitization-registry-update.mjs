#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
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
  "crates/rustok-page-builder/contracts/evidence/page-builder-static-sanitization-registry-admission-source.json";
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;

function fail(message) {
  throw new Error(`Page Builder static sanitization registry admission failed: ${message}`);
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

function regularFile(location, label, maximumBytes) {
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
  if (typeof relativePath !== "string" || relativePath.length === 0 || /[\u0000\r\n]/u.test(relativePath)) {
    fail(`${label} path is invalid`);
  }
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

function inputJson(candidate, label, maximumBytes) {
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
  const accepted = new Set(["--receipt", "--workflow-run", "--output"]);
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") {
      console.log(
        "usage: admit-page-builder-static-sanitization-registry-update.mjs --receipt FILE --workflow-run FILE [--output FILE]",
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
  for (const required of ["--receipt", "--workflow-run"]) {
    if (!Object.hasOwn(result, required)) fail(`${required} is required`);
  }
  return result;
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.output?.default_path;
  if (
    typeof candidate !== "string" ||
    candidate.length === 0 ||
    candidate.length > 16_384 ||
    /[\u0000\r\n]/u.test(candidate)
  ) {
    fail("output path is invalid");
  }
  const absolute = path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
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

function pointerValue(document, pointer) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) fail("target JSON Pointer is invalid");
  let current = document;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, token)) {
      fail(`target JSON Pointer does not resolve at ${rawToken}`);
    }
    current = current[token];
  }
  return current;
}

function currentCommit() {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!COMMIT_PATTERN.test(value)) fail("checkout HEAD must be a full lowercase Git SHA");
  return value;
}

function requireCommit(commit) {
  if (!COMMIT_PATTERN.test(commit)) fail("receipt source_commit is invalid");
  const result = spawnSync("git", ["cat-file", "-e", `${commit}^{commit}`], {
    cwd: repoRoot,
    stdio: "ignore",
  });
  if (result.status !== 0) fail("receipt source_commit does not exist in checkout history");
}

function requireAncestor(commit) {
  const result = spawnSync("git", ["merge-base", "--is-ancestor", commit, "HEAD"], {
    cwd: repoRoot,
    stdio: "ignore",
  });
  if (result.status !== 0) fail("receipt source_commit is not an ancestor of checkout HEAD");
}

function fileAtCommit(commit, relativePath) {
  try {
    return execFileSync("git", ["show", `${commit}:${relativePath}`], {
      cwd: repoRoot,
      maxBuffer: MAX_SOURCE_BYTES + 1024,
    });
  } catch {
    fail(`required execution source ${relativePath} is missing at receipt source_commit`);
  }
}

function stringIdentity(value) {
  if (typeof value === "number" && Number.isSafeInteger(value) && value > 0) return String(value);
  if (typeof value === "string" && /^[1-9][0-9]*$/u.test(value)) return value;
  return null;
}

export function evaluateRunMetadata(run, receipt, requirements) {
  const failures = [];
  const provenance = receipt?.provenance ?? {};
  const expectedRunId = stringIdentity(provenance.run_id);
  const expectedRunAttempt = stringIdentity(provenance.run_attempt);
  const actualRunId = stringIdentity(run?.id);
  const actualRunAttempt = stringIdentity(run?.run_attempt);

  const checks = [
    [run?.repository?.full_name === requirements.repository, "workflow repository mismatch"],
    [run?.name === requirements.workflow_name, "workflow name mismatch"],
    [run?.path === requirements.workflow_path, "workflow path mismatch"],
    [run?.event === requirements.event, "workflow event mismatch"],
    [run?.head_branch === requirements.head_branch, "workflow head branch mismatch"],
    [run?.status === requirements.required_status, "workflow status is not completed"],
    [run?.conclusion === requirements.required_conclusion, "workflow conclusion is not success"],
    [actualRunId !== null && actualRunId === expectedRunId, "workflow run id mismatch"],
    [actualRunAttempt !== null && actualRunAttempt === expectedRunAttempt, "workflow run attempt mismatch"],
    [run?.head_sha === receipt?.source_commit, "workflow head SHA mismatch"],
    [provenance.repository === requirements.repository, "receipt repository mismatch"],
    [provenance.workflow === requirements.workflow_name, "receipt workflow name mismatch"],
    [provenance.event_name === requirements.event, "receipt event mismatch"],
    [provenance.github_actions === true, "receipt is not marked as GitHub Actions generated"],
    [provenance.cryptographic_ci_attestation_claimed === false, "receipt overclaims cryptographic attestation"],
  ];
  for (const [valid, message] of checks) {
    if (!valid) failures.push(message);
  }
  return { valid: failures.length === 0, failures };
}

export function evaluateReceiptBoundary(receipt, executionSource, target) {
  const failures = [];
  const execution = receipt?.execution ?? {};
  const governance = receipt?.governance ?? {};
  const receiptTarget = receipt?.target ?? {};
  const expectedCommands = executionSource?.execution?.test_commands;
  const expectedFragments = executionSource?.execution?.required_test_name_fragments;
  const checks = [
    [receipt?.format === executionSource?.output?.format, "receipt format mismatch"],
    [receipt?.status === executionSource?.output?.success_status, "receipt status mismatch"],
    [COMMIT_PATTERN.test(receipt?.source_commit ?? ""), "receipt source_commit is invalid"],
    [execution?.all_commands_passed === true, "receipt does not assert all commands passed"],
    [execution?.packet_generated_only_after_test_steps === true, "receipt packet ordering assertion is missing"],
    [execution?.test_list_command === executionSource?.execution?.test_list_command, "test-list command mismatch"],
    [JSON.stringify(execution?.test_commands) === JSON.stringify(expectedCommands), "test command set mismatch"],
    [JSON.stringify(execution?.required_test_name_fragments) === JSON.stringify(expectedFragments), "required test identity set mismatch"],
    [execution?.network_runtime_under_test === false, "receipt unexpectedly claims network runtime"],
    [execution?.database_used === false, "receipt unexpectedly claims database use"],
    [execution?.browser_used === false, "receipt unexpectedly claims browser use"],
    [receiptTarget?.fba_registry === target.fba_registry, "receipt registry path mismatch"],
    [receiptTarget?.registry_status_before === target.registry_required_status, "receipt registry status mismatch"],
    [receiptTarget?.executed_evidence_json_pointer === target.executed_evidence_json_pointer, "receipt target pointer mismatch"],
    [receiptTarget?.executed_evidence_before === target.required_before_value, "receipt target before-value mismatch"],
    [governance?.registry_mutated === false, "receipt claims registry mutation"],
    [governance?.executed_evidence_cleared === false, "receipt claims executed evidence was already cleared"],
    [governance?.terminal_inventory_complete_claimed === false, "receipt claims terminal inventory completion"],
    [governance?.owner_approval_claimed === false, "receipt claims owner approval"],
    [governance?.platform_approval_claimed === false, "receipt claims platform approval"],
    [governance?.page_builder_fba_promoted === false, "receipt claims FBA promotion"],
    [governance?.later_evidence_containing_registry_pr_required === true, "receipt does not require later evidence-containing PR"],
  ];
  for (const [valid, message] of checks) {
    if (!valid) failures.push(message);
  }
  return { valid: failures.length === 0, failures };
}

function validateSourceLineage(receipt, executionSource) {
  requireCommit(receipt.source_commit);
  requireAncestor(receipt.source_commit);
  const sourceHashes = objectValue(receipt.source_sha256, "receipt source_sha256");
  const requiredFiles = executionSource.required_source_files;
  if (!Array.isArray(requiredFiles) || requiredFiles.length === 0) {
    fail("execution source required_source_files must be non-empty");
  }
  const actualKeys = Object.keys(sourceHashes).sort();
  const expectedKeys = [...requiredFiles].sort();
  if (JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys)) {
    fail("receipt source_sha256 does not cover the exact execution required-source set");
  }

  const currentHashes = {};
  for (const relativePath of requiredFiles) {
    const expectedHash = sourceHashes[relativePath];
    if (typeof expectedHash !== "string" || !SHA256_PATTERN.test(expectedHash)) {
      fail(`receipt source hash is invalid for ${relativePath}`);
    }
    const executedHash = sha256(fileAtCommit(receipt.source_commit, relativePath));
    if (executedHash !== expectedHash) {
      fail(`receipt source hash does not match executed commit for ${relativePath}`);
    }
    const current = repoFile(relativePath, `current execution source ${relativePath}`);
    if (current.sha256 !== expectedHash) {
      fail(`execution source drift requires new execution: ${relativePath}`);
    }
    currentHashes[relativePath] = current.sha256;
  }
  return currentHashes;
}

function validateCurrentTarget(contract, receipt) {
  const registry = repoJson(contract.target.fba_registry, "current Page Builder FBA registry");
  if (registry.document.status !== contract.target.registry_required_status) {
    fail("current Page Builder FBA registry status drifted");
  }
  const currentValue = pointerValue(registry.document, contract.target.executed_evidence_json_pointer);
  if (currentValue !== contract.target.required_before_value) {
    fail("current static sanitization executed-evidence node is no longer pending");
  }
  if (registry.sha256 !== receipt.target?.registry_sha256) {
    fail("current FBA registry does not match the registry hashed by the execution receipt");
  }
  return registry.sha256;
}

function main() {
  const args = parseArguments(process.argv.slice(2));
  const contractRecord = repoJson(contractPath, "registry admission source contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "page_builder_static_sanitization_registry_admission_source" ||
    contract.status !== "source_ready_execution_receipt_pending"
  ) {
    fail("registry admission source contract identity drifted");
  }

  const executionSourceRecord = repoJson(contract.predecessor.source_contract, "execution source contract");
  const executionSource = executionSourceRecord.document;
  if (
    executionSource.format !== contract.predecessor.source_format ||
    executionSource.status !== contract.predecessor.source_status
  ) {
    fail("execution source contract identity drifted");
  }

  const receiptRecord = inputJson(
    args["--receipt"],
    "static sanitization execution receipt",
    contract.predecessor.maximum_receipt_bytes,
  );
  const runRecord = inputJson(
    args["--workflow-run"],
    "GitHub workflow run metadata",
    contract.predecessor.maximum_run_metadata_bytes,
  );
  const receipt = receiptRecord.document;

  const receiptBoundary = evaluateReceiptBoundary(receipt, executionSource, contract.target);
  if (!receiptBoundary.valid) fail(receiptBoundary.failures.join("; "));
  const runBoundary = evaluateRunMetadata(runRecord.document, receipt, contract.github_run);
  if (!runBoundary.valid) fail(runBoundary.failures.join("; "));

  const currentSourceHashes = validateSourceLineage(receipt, executionSource);
  const registrySha256 = validateCurrentTarget(contract, receipt);
  const head = currentCommit();

  const output = outputPath(contract, args["--output"]);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.success_status,
    generated_at: new Date().toISOString(),
    checkout_head: head,
    execution_source_commit: receipt.source_commit,
    evidence: {
      receipt_bytes: receiptRecord.size,
      receipt_sha256: receiptRecord.sha256,
      workflow_run_metadata_bytes: runRecord.size,
      workflow_run_metadata_sha256: runRecord.sha256,
      raw_receipt_path_retained: false,
      raw_run_metadata_path_retained: false,
    },
    github_run: {
      repository: runRecord.document.repository.full_name,
      workflow: runRecord.document.name,
      workflow_path: runRecord.document.path,
      run_id: String(runRecord.document.id),
      run_attempt: String(runRecord.document.run_attempt),
      event: runRecord.document.event,
      head_branch: runRecord.document.head_branch,
      head_sha: runRecord.document.head_sha,
      status: runRecord.document.status,
      conclusion: runRecord.document.conclusion,
      cryptographic_ci_attestation_claimed: false,
      maintainer_external_github_review_required: true,
    },
    target: {
      fba_registry: contract.target.fba_registry,
      registry_sha256_before: registrySha256,
      executed_evidence_json_pointer: contract.target.executed_evidence_json_pointer,
      executed_evidence_before: contract.target.required_before_value,
      admitted_after_value: contract.target.admitted_after_value,
    },
    execution_source_sha256: currentSourceHashes,
    governance: {
      registry_mutated: false,
      executed_evidence_cleared: false,
      terminal_inventory_recomputed: false,
      owner_approval_claimed: false,
      platform_approval_claimed: false,
      page_builder_fba_promoted: false,
      separate_evidence_containing_registry_pr_required: true,
      terminal_inventory_recompute_required_after_registry_change: true,
    },
    privacy: {
      raw_receipt_path_retained: false,
      raw_run_metadata_path_retained: false,
      raw_logs_retained: false,
      credentials_or_cookies_retained: false,
      tenant_identity_retained: false,
    },
  });

  console.log(
    `[admit-page-builder-static-sanitization-registry-update] PASS execution_source=${receipt.source_commit} checkout_head=${head} run_id=${String(runRecord.document.id)} registry_update=pending`,
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
