#!/usr/bin/env node

import { spawnSync } from "node:child_process";
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
const admissionContractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-consumer-properties-provider-admission-source.json",
);
const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const INTEGER_PATTERN = /^[1-9][0-9]*$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Pages consumer-properties provider admission failed: ${message}`);
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

function canonicalJson(value) {
  const normalize = (input) => {
    if (Array.isArray(input)) return input.map(normalize);
    if (input !== null && typeof input === "object") {
      return Object.fromEntries(
        Object.entries(input)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, nested]) => [key, normalize(nested)]),
      );
    }
    return input;
  };
  return JSON.stringify(normalize(value));
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error || result.status !== 0) fail("git HEAD lookup failed");
  return requireCommit(result.stdout.trim(), "git HEAD");
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !COMMIT_PATTERN.test(value)) {
    fail(`${label} must be a full lowercase Git SHA`);
  }
  return value;
}

function requireSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    fail(`${label} must be a lowercase SHA-256`);
  }
  return value;
}

function requirePositiveIntegerString(value, label) {
  if (typeof value !== "string" || !INTEGER_PATTERN.test(value)) {
    fail(`${label} must be a positive integer string`);
  }
}

function requireRepoDigest(value, label) {
  if (typeof value !== "string" || value.length > 1024 || !REPO_DIGEST_PATTERN.test(value)) {
    fail(`${label} must be an immutable REPOSITORY@sha256:<digest>`);
  }
  return value;
}

function requireCanonicalIso(value, label) {
  if (typeof value !== "string") fail(`${label} must be a canonical ISO timestamp`);
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp) || new Date(timestamp).toISOString() !== value) {
    fail(`${label} must be a canonical ISO timestamp`);
  }
}

function commitExists(commit, label) {
  const result = spawnSync("git", ["cat-file", "-e", `${commit}^{commit}`], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    fail(`${label} is not available in checkout history`);
  }
}

function requireAncestor(ancestor, descendant, label) {
  commitExists(ancestor, `${label} ancestor`);
  commitExists(descendant, `${label} descendant`);
  const result = spawnSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error || result.status > 1) fail(`${label} ancestry check failed`);
  if (result.status !== 0) fail(`${label} is not ancestor-bound`);
}

function parseArguments(argv) {
  const options = {};
  const allowed = new Set([
    "--rust-receipt",
    "--browser-evidence",
    "--deployment-identity",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: admit-pages-consumer-properties-provider.mjs " +
          "--rust-receipt FILE --browser-evidence FILE --deployment-identity FILE [--output FILE]",
      );
      process.exit(0);
    }
    if (!allowed.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    const key = argument
      .slice(2)
      .replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
    options[key] = value;
    index += 1;
  }
  for (const key of ["rustReceipt", "browserEvidence", "deploymentIdentity"]) {
    if (!options[key]) {
      fail(`--${key.replace(/[A-Z]/gu, (letter) => `-${letter.toLowerCase()}`)} is required`);
    }
  }
  return options;
}

function resolveInput(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 16_384 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} path is invalid`);
  }
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function regularFile(location, label, maximumBytes = MAX_INPUT_BYTES) {
  if (!existsSync(location)) fail(`${label} is missing`);
  const metadata = lstatSync(location);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(location).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(location);
  return { bytes, size, sha256: sha256(bytes) };
}

function jsonInput(value, label) {
  const record = regularFile(resolveInput(value, label), label);
  try {
    return { document: objectValue(JSON.parse(record.bytes.toString("utf8")), label), record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function jsonSource(relativePath, label) {
  const location = path.resolve(repoRoot, relativePath);
  const record = regularFile(location, label, MAX_SOURCE_BYTES);
  try {
    return objectValue(JSON.parse(record.bytes.toString("utf8")), label);
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function sourceHash(relativePath) {
  if (
    typeof relativePath !== "string" ||
    relativePath.length === 0 ||
    relativePath.length > 4096 ||
    relativePath.includes("\0")
  ) {
    fail("source path is invalid");
  }
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`source path escapes repository: ${relativePath}`);
  }
  return sha256(regularFile(absolute, `source file ${relativePath}`, MAX_SOURCE_BYTES).bytes);
}

function expectedSourceFiles(contract, label) {
  const files = contract.required_source_files;
  if (!Array.isArray(files) || files.length === 0 || files.length > 128) {
    fail(`${label} required_source_files is invalid`);
  }
  if (new Set(files).size !== files.length) fail(`${label} required_source_files contains duplicates`);
  return [...files].sort();
}

function verifyRetainedSourceHashes(document, contract, field, label) {
  const retained = objectValue(document[field], `${label}.${field}`);
  const expectedNames = expectedSourceFiles(contract, label);
  const actualNames = Object.keys(retained).sort();
  if (canonicalJson(actualNames) !== canonicalJson(expectedNames)) {
    fail(`${label} source hash set differs from its source contract`);
  }
  for (const relativePath of expectedNames) {
    if (
      requireSha256(retained[relativePath], `${label} hash ${relativePath}`) !==
      sourceHash(relativePath)
    ) {
      fail(`${label} source hash for ${relativePath} does not match checkout`);
    }
  }
}

function sourceHashes(contract) {
  return Object.fromEntries(
    expectedSourceFiles(contract, "provider admission contract").map((relativePath) => [
      relativePath,
      sourceHash(relativePath),
    ]),
  );
}

function pointerValue(document, pointer) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) {
    fail("target JSON Pointer is invalid");
  }
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

function validateCurrentTargets(admissionContract) {
  const targets = {};
  for (const [name, specification] of Object.entries(admissionContract.targets ?? {})) {
    const record = regularFile(
      path.resolve(repoRoot, specification.path),
      `${name} target`,
      MAX_SOURCE_BYTES,
    );
    const document = objectValue(JSON.parse(record.bytes.toString("utf8")), `${name} target`);
    const before = pointerValue(document, specification.json_pointer);
    if (before !== specification.required_before) {
      fail(`${name} target is no longer ${specification.required_before}`);
    }
    targets[name] = {
      path: specification.path,
      json_pointer: specification.json_pointer,
      before,
      admitted_after: specification.admitted_after,
      sha256: record.sha256,
    };
  }
  return targets;
}

function validateRust(input, admissionContract, rustContract, head) {
  const document = input.document;
  const specification = admissionContract.rust_receipt_input;
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("Rust receipt format/status drifted");
  }
  requireCanonicalIso(document.generated_at, "Rust receipt generated_at");
  const sourceCommit = requireCommit(document.source_commit, "Rust receipt source_commit");
  requireAncestor(sourceCommit, head, "Rust receipt to checkout");

  const provenance = objectValue(document.provenance, "Rust receipt provenance");
  if (
    provenance.repository !== specification.canonical_repository ||
    provenance.workflow !== specification.canonical_workflow ||
    provenance.event_name !== specification.required_event ||
    provenance.head_branch !== specification.required_branch ||
    provenance.github_actions !== true ||
    provenance.cryptographic_ci_attestation_claimed !== false
  ) {
    fail("Rust receipt GitHub Actions provenance drifted");
  }
  requirePositiveIntegerString(provenance.run_id, "Rust receipt run_id");
  requirePositiveIntegerString(provenance.run_attempt, "Rust receipt run_attempt");

  const execution = objectValue(document.execution, "Rust receipt execution");
  if (
    execution.all_commands_passed !== true ||
    execution.packet_generated_only_after_test_and_check_steps !== true ||
    execution.browser_used !== false ||
    execution.browser_evidence_pending !== true
  ) {
    fail("Rust receipt execution boundary drifted");
  }

  const targets = objectValue(document.targets, "Rust receipt targets");
  for (const [key, contractTarget, expectedStatus] of [
    ["consumer_contract", rustContract.consumer_contract, rustContract.consumer_contract.required_status],
    ["fba_registry", rustContract.fba_registry, rustContract.fba_registry.required_status],
  ]) {
    const target = objectValue(targets[key], `Rust receipt ${key}`);
    if (
      target.path !== contractTarget.path ||
      target.status_before !== expectedStatus ||
      target.json_pointer !== contractTarget.executed_evidence_json_pointer ||
      target.before !== contractTarget.required_before_value ||
      requireSha256(target.sha256, `Rust receipt ${key} sha256`) !== sourceHash(contractTarget.path)
    ) {
      fail(`Rust receipt ${key} target pre-state drifted`);
    }
  }
  if (
    execution.test_list_command !== rustContract.execution.test_list_command ||
    canonicalJson(execution.required_test_name_fragments) !==
      canonicalJson(rustContract.execution.required_test_name_fragments) ||
    canonicalJson(execution.verifier_commands) !== canonicalJson(rustContract.execution.verifier_commands) ||
    canonicalJson(execution.test_commands) !== canonicalJson(rustContract.execution.test_commands) ||
    execution.check_command !== rustContract.execution.check_command
  ) {
    fail("Rust receipt command set drifted");
  }
  verifyRetainedSourceHashes(document, rustContract, "source_sha256", "Rust receipt");

  const governance = objectValue(document.governance, "Rust receipt governance");
  for (const key of [
    "consumer_contract_mutated",
    "fba_registry_mutated",
    "executed_evidence_cleared",
    "browser_execution_claimed",
    "deployment_provenance_verified",
    "terminal_inventory_complete_claimed",
    "owner_approval_claimed",
    "platform_approval_claimed",
    "pages_ffa_promoted",
    "page_builder_fba_promoted",
  ]) {
    if (governance[key] !== false) fail(`Rust receipt governance ${key} must remain false`);
  }
  if (governance.later_admission_must_bind_rust_browser_and_source_lineage !== true) {
    fail("Rust receipt later-admission boundary drifted");
  }
  return sourceCommit;
}

function requireFacts(observation, requiredFacts, label) {
  const record = objectValue(observation, `${label} observation`);
  if (record.passed !== true || record.criticalFailures !== 0) {
    fail(`${label} browser observation did not pass cleanly`);
  }
  const facts = objectValue(record.facts, `${label} browser facts`);
  for (const fact of requiredFacts) {
    if (facts[fact] !== true) fail(`${label} browser fact ${fact} must be true`);
  }
}

function validateBrowser(input, admissionContract, browserContract, head) {
  const document = input.document;
  const specification = admissionContract.browser_input;
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("browser packet format/status drifted");
  }
  requireCanonicalIso(document.executed_at, "browser packet executed_at");
  const sourceCommit = requireCommit(document.source_commit, "browser source_commit");
  requireAncestor(sourceCommit, head, "browser packet to checkout");
  const deploymentDigest = requireRepoDigest(document.deployment_digest, "browser deployment digest");
  verifyRetainedSourceHashes(document, browserContract, "source_files", "browser packet");

  if (
    document.retained_secrets !== false ||
    document.metadata_values_retained !== false ||
    document.browser_execution_only !== true ||
    document.consumer_properties_admission_pending !== true
  ) {
    fail("browser packet retention/admission boundary drifted");
  }
  const expectedProfiles = [...browserContract.profiles].sort();
  const inputRecords = objectValue(document.input_records, "browser input records");
  const editorStorage = objectValue(inputRecords.editor_storage_state, "browser editor storage record");
  if (!Number.isSafeInteger(editorStorage.bytes) || editorStorage.bytes <= 0) {
    fail("browser editor storage byte count is invalid");
  }
  requireSha256(editorStorage.sha256, "browser editor storage hash");
  const profileHashes = objectValue(inputRecords.profile_url_sha256, "browser profile URL hashes");
  if (canonicalJson(Object.keys(profileHashes).sort()) !== canonicalJson(expectedProfiles)) {
    fail("browser profile URL hash set drifted");
  }
  for (const profile of expectedProfiles) {
    requireSha256(profileHashes[profile], `browser ${profile} URL hash`);
  }
  const observations = objectValue(document.observations, "browser observations");
  if (canonicalJson(Object.keys(observations).sort()) !== canonicalJson(expectedProfiles)) {
    fail("browser profile set drifted");
  }
  requireFacts(
    observations.published,
    [
      "registered_surface_visible",
      "published_only_admission",
      "fly_canvas_unmounted",
      "document_authoring_unmounted",
      "registered_runtime_present",
      "owner_port_persistence_declared",
      "registered_property_panel_ready",
      "save_action_available_without_mutation",
    ],
    "published",
  );
  for (const profile of ["draft", "archived", "missing"]) {
    requireFacts(
      observations[profile],
      ["registered_published_surface_absent", "metadata_surface_error_absent"],
      profile,
    );
  }
  return { sourceCommit, deploymentDigest };
}

function validateDeployment(input, admissionContract, deploymentContract, head, browser) {
  const document = input.document;
  const specification = admissionContract.deployment_identity_input;
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("deployment identity format/status drifted");
  }
  requireCanonicalIso(document.captured_at, "deployment identity captured_at");
  const deployment = objectValue(document.deployment, "deployment identity deployment");
  const sourceCommit = requireCommit(deployment.source_commit, "deployment source_commit");
  if (sourceCommit !== browser.sourceCommit) {
    fail("deployment source_commit differs from browser source_commit");
  }
  requireAncestor(sourceCommit, head, "deployment identity to checkout");
  const deploymentDigest = requireRepoDigest(
    deployment.deployment_image_digest,
    "deployment image digest",
  );
  if (deploymentDigest !== browser.deploymentDigest) {
    fail("deployment image digest differs from browser packet");
  }
  if (
    deployment.inventory_complete !== true ||
    deployment.origin_to_repo_digest_binding !== specification.origin_to_repo_digest_binding ||
    deployment.cryptographic_origin_to_repo_digest_binding !== false
  ) {
    fail("deployment identity provenance boundary drifted");
  }
  if (
    !Number.isSafeInteger(deployment.expected_target_count) ||
    deployment.expected_target_count <= 0 ||
    deployment.expected_target_count > 64 ||
    deployment.verified_target_count !== deployment.expected_target_count
  ) {
    fail("deployment identity target counts are incomplete");
  }
  const targets = document.expected_targets;
  if (!Array.isArray(targets) || targets.length !== deployment.expected_target_count) {
    fail("deployment identity expected target set is incomplete");
  }
  const ids = new Set();
  for (const [index, targetValue] of targets.entries()) {
    const target = objectValue(targetValue, `deployment target ${index}`);
    if (
      typeof target.target_id !== "string" ||
      target.target_id.length === 0 ||
      ids.has(target.target_id)
    ) {
      fail("deployment target identifiers are invalid or duplicated");
    }
    ids.add(target.target_id);
    if (
      target.status !== 200 ||
      target.reported_source_commit !== sourceCommit ||
      target.source_commit_verified_equal_checkout !== true ||
      target.raw_metrics_url_persisted !== false ||
      target.raw_response_persisted !== false
    ) {
      fail(`deployment target ${target.target_id} source verification drifted`);
    }
    requireSha256(target.metrics_url_sha256, `deployment target ${target.target_id} URL hash`);
    requireSha256(target.response_sha256, `deployment target ${target.target_id} response hash`);
  }
  verifyRetainedSourceHashes(document, deploymentContract, "source_files", "deployment identity");

  const credentials = objectValue(document.credentials, "deployment identity credentials");
  if (!Array.isArray(credentials.environment_names) || credentials.values_persisted !== false) {
    fail("deployment identity credential retention boundary drifted");
  }
  const privacy = objectValue(document.privacy, "deployment identity privacy");
  for (const key of [
    "raw_metrics_urls_persisted",
    "raw_metrics_responses_persisted",
    "credential_values_persisted",
    "tenant_page_revision_or_correlation_ids_persisted",
  ]) {
    if (privacy[key] !== false) fail(`deployment identity privacy ${key} must remain false`);
  }
  for (const key of [
    "provider_health_snapshot_evaluated",
    "pages_provider_health_observed",
    "pages_reference_consumer_gate_accepted",
    "forum_wave_accepted",
    "ffa_promoted",
    "fba_promoted",
  ]) {
    if (document[key] !== false) fail(`deployment identity boundary ${key} must remain false`);
  }
  return {
    sourceCommit,
    deploymentId: deployment.deployment_id,
    deploymentDigest,
    expectedTargetCount: deployment.expected_target_count,
  };
}

function packetRecord(input) {
  return { bytes: input.record.size, sha256: input.record.sha256 };
}

function outputPath(contract, requested) {
  const candidate = requested ?? contract.output.default_path;
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
    fail("provider admission output must remain inside repository target/");
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

function main() {
  const options = parseArguments(process.argv.slice(2));
  const admissionContract = jsonSource(
    path.relative(repoRoot, admissionContractPath),
    "provider admission contract",
  );
  if (
    admissionContract.format !== "pages_consumer_properties_provider_admission_source_v1" ||
    admissionContract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("provider admission source identity drifted");
  }

  const rustContract = jsonSource(
    admissionContract.rust_receipt_input.source_contract,
    "Rust source execution contract",
  );
  const browserContract = jsonSource(
    admissionContract.browser_input.source_contract,
    "browser execution contract",
  );
  const deploymentContract = jsonSource(
    admissionContract.deployment_identity_input.source_contract,
    "deployment identity source contract",
  );

  const rust = jsonInput(options.rustReceipt, "Rust receipt");
  const browser = jsonInput(options.browserEvidence, "browser evidence");
  const deployment = jsonInput(options.deploymentIdentity, "deployment identity");
  const head = currentCommit();

  const rustSourceCommit = validateRust(rust, admissionContract, rustContract, head);
  const browserLineage = validateBrowser(browser, admissionContract, browserContract, head);
  const deploymentLineage = validateDeployment(
    deployment,
    admissionContract,
    deploymentContract,
    head,
    browserLineage,
  );
  requireAncestor(
    rustSourceCommit,
    browserLineage.sourceCommit,
    "Rust receipt to browser/deployment source",
  );

  const targets = validateCurrentTargets(admissionContract);
  const output = outputPath(admissionContract, options.output);
  rmSync(output, { force: true });
  writeAtomic(output, {
    format: admissionContract.output.format,
    status: admissionContract.output.status,
    admitted_at: new Date().toISOString(),
    checkout_commit: head,
    lineage: {
      rust_source_commit: rustSourceCommit,
      browser_and_deployment_source_commit: browserLineage.sourceCommit,
      rust_source_is_ancestor_of_browser_source: true,
      browser_and_deployment_source_match: true,
      all_retained_source_sets_match_checkout: true,
      required_source_drift_detected: false,
    },
    deployment: {
      deployment_id: deploymentLineage.deploymentId,
      deployment_image_digest: deploymentLineage.deploymentDigest,
      expected_target_count: deploymentLineage.expectedTargetCount,
      source_commit_verified_on_all_expected_targets: true,
      origin_to_repo_digest_binding: "maintainer_reviewed_external_fact",
      cryptographic_origin_to_repo_digest_binding: false,
    },
    input_packets: {
      rust_receipt: packetRecord(rust),
      browser_evidence: packetRecord(browser),
      deployment_identity: packetRecord(deployment),
    },
    targets,
    source_files: sourceHashes(admissionContract),
    boundaries: {
      consumer_contract_mutated: false,
      fba_registry_mutated: false,
      executed_evidence_changed: false,
      terminal_inventory_recomputed: false,
      pages_execution_rollout_marker_changed: false,
      terminal_inventory_complete_claimed: false,
      owner_approval_claimed: false,
      platform_approval_claimed: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
      separate_evidence_containing_update_required: true,
    },
  });
  console.log(
    `[admit-pages-consumer-properties-provider] PASS checkout=${head} rust=${rustSourceCommit} deployed=${browserLineage.sourceCommit} registry_update=pending`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
