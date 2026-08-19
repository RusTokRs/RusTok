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
const admissionContractPath =
  "crates/rustok-pages/contracts/evidence/pages-consumer-properties-admission-source.json";
const MAX_INPUT_BYTES = 8 * 1024 * 1024;
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;
const RUN_ID_PATTERN = /^[1-9][0-9]*$/u;
const REVIEWER_PATTERN = /^[A-Za-z0-9._-]{1,64}$/u;

function fail(message) {
  throw new Error(`Pages consumer-properties admission failed: ${message}`);
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
  if (result.error) fail(`git HEAD lookup failed: ${result.error.message}`);
  if (result.status !== 0) fail("git HEAD lookup returned a non-zero status");
  const commit = result.stdout.trim();
  if (!COMMIT_PATTERN.test(commit)) fail("checkout HEAD is not a full lowercase Git SHA");
  return commit;
}

function requireCommitAncestor(ancestor, descendant) {
  const result = spawnSync("git", ["merge-base", "--is-ancestor", ancestor, descendant], {
    cwd: repoRoot,
    encoding: "utf8",
    shell: false,
    maxBuffer: 1024 * 1024,
  });
  if (result.error) fail(`source receipt ancestry lookup failed: ${result.error.message}`);
  if (result.status !== 0) {
    fail(
      "source execution receipt source_commit is not a locally verifiable ancestor of checkout HEAD",
    );
  }
}

function parseArguments(argv) {
  const options = {};
  const allowed = new Set([
    "--source-receipt",
    "--browser-evidence",
    "--deployment-provenance",
    "--output",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: admit-pages-consumer-properties.mjs " +
          "--source-receipt FILE --browser-evidence FILE --deployment-provenance FILE [--output FILE]",
      );
      process.exit(0);
    }
    if (!allowed.has(argument)) fail(`unknown argument ${argument}`);
    if (Object.hasOwn(options, argument)) fail(`${argument} may be supplied only once`);
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) fail(`${argument} requires a value`);
    options[argument] = value;
    index += 1;
  }
  for (const required of ["--source-receipt", "--browser-evidence", "--deployment-provenance"]) {
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
  const record = regularFile(resolveInput(candidate, label), label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { document, ...record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function repoFile(relativePath, label, maximumBytes = MAX_SOURCE_BYTES) {
  if (
    typeof relativePath !== "string" ||
    relativePath.length === 0 ||
    relativePath.length > 4096 ||
    /[\u0000\r\n]/u.test(relativePath)
  ) {
    fail(`${label} path is invalid`);
  }
  const absolute = path.resolve(repoRoot, relativePath);
  const relative = path.relative(repoRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) fail(`${label} escapes repository root`);
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

function canonicalRunId(value, label) {
  const normalized = typeof value === "number" && Number.isSafeInteger(value) ? String(value) : value;
  if (typeof normalized !== "string" || !RUN_ID_PATTERN.test(normalized)) {
    fail(`${label} must be a positive GitHub Actions run id`);
  }
  return normalized;
}

function canonicalIso(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    fail(`${label} must be a bounded canonical timestamp`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be canonical ISO-8601 UTC`);
  }
  return value;
}

function pointerValue(document, pointer, label) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) fail(`${label} JSON Pointer is invalid`);
  let current = document;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, token)) {
      fail(`${label} JSON Pointer does not resolve`);
    }
    current = current[token];
  }
  return current;
}

function expectedSourceFiles(contract, label) {
  const files = contract.required_source_files;
  if (!Array.isArray(files) || files.length === 0 || files.length > 128) {
    fail(`${label} required_source_files is invalid`);
  }
  if (new Set(files).size !== files.length) fail(`${label} required_source_files contains duplicates`);
  return [...files].sort();
}

function sourceHash(relativePath) {
  return repoFile(relativePath, `source file ${relativePath}`).sha256;
}

function verifyRetainedSourceHashes(document, sourceContract, field, label) {
  const retained = objectValue(document[field], `${label}.${field}`);
  const expectedNames = expectedSourceFiles(sourceContract, label);
  const actualNames = Object.keys(retained).sort();
  if (canonicalJson(actualNames) !== canonicalJson(expectedNames)) {
    fail(`${label} source hash set differs from its source contract`);
  }
  for (const relativePath of expectedNames) {
    const retainedHash = canonicalSha256(retained[relativePath], `${label} source hash ${relativePath}`);
    if (retainedHash !== sourceHash(relativePath)) {
      fail(`${label} source hash for ${relativePath} does not match checkout`);
    }
  }
}

function sourceHashes(contract) {
  return Object.fromEntries(
    expectedSourceFiles(contract, "admission contract").map((relativePath) => [
      relativePath,
      sourceHash(relativePath),
    ]),
  );
}

function requirePacketRecord(record, label) {
  const value = objectValue(record, label);
  if (!Number.isSafeInteger(value.bytes) || value.bytes <= 0 || value.bytes > MAX_INPUT_BYTES) {
    fail(`${label}.bytes is invalid`);
  }
  canonicalSha256(value.sha256, `${label}.sha256`);
}

function validateTargetPreconditions(contract) {
  const consumerSpec = objectValue(contract.target_preconditions?.consumer_contract, "consumer precondition");
  const consumer = repoJson(consumerSpec.path, "consumer properties contract");
  if (
    consumer.document.format !== consumerSpec.required_format ||
    consumer.document.status !== consumerSpec.required_status ||
    pointerValue(
      consumer.document,
      consumerSpec.executed_evidence_json_pointer,
      "consumer properties contract",
    ) !== consumerSpec.required_before_value
  ) {
    fail("consumer properties contract is no longer in the pending admission state");
  }

  const registrySpec = objectValue(contract.target_preconditions?.fba_registry, "FBA precondition");
  const registry = repoJson(registrySpec.path, "Page Builder FBA registry");
  if (
    registry.document.status !== registrySpec.required_status ||
    pointerValue(registry.document, registrySpec.executed_evidence_json_pointer, "FBA registry") !==
      registrySpec.required_before_value
  ) {
    fail("Page Builder FBA consumer-properties evidence is no longer pending");
  }

  return {
    consumer: { path: consumerSpec.path, sha256: consumer.sha256 },
    registry: { path: registrySpec.path, sha256: registry.sha256 },
  };
}

function validateSourceReceipt(input, contract, sourceContract, head, targets) {
  const document = input.document;
  const specification = objectValue(contract.source_execution_input, "source execution input");
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("source execution receipt format/status drifted");
  }
  const sourceCommit = canonicalCommit(document.source_commit, "source receipt source_commit");
  requireCommitAncestor(sourceCommit, head);

  const provenance = objectValue(document.provenance, "source receipt provenance");
  if (
    provenance.repository !== specification.repository_must_equal ||
    provenance.workflow !== specification.workflow_must_equal ||
    !["push", "workflow_dispatch"].includes(provenance.event_name) ||
    provenance.head_branch !== "main" ||
    provenance.github_actions !== true ||
    provenance.cryptographic_ci_attestation_claimed !== false
  ) {
    fail("source execution receipt workflow provenance drifted");
  }
  const runId = canonicalRunId(provenance.run_id, "source workflow run id");

  verifyRetainedSourceHashes(document, sourceContract, "source_sha256", "source execution receipt");

  const receiptTargets = objectValue(document.targets, "source receipt targets");
  const consumerTarget = objectValue(receiptTargets.consumer_contract, "source receipt consumer target");
  const registryTarget = objectValue(receiptTargets.fba_registry, "source receipt registry target");
  const consumerSpec = contract.target_preconditions.consumer_contract;
  const registrySpec = contract.target_preconditions.fba_registry;
  if (
    consumerTarget.path !== consumerSpec.path ||
    consumerTarget.status_before !== consumerSpec.required_status ||
    consumerTarget.sha256 !== targets.consumer.sha256 ||
    consumerTarget.json_pointer !== consumerSpec.executed_evidence_json_pointer ||
    consumerTarget.before !== consumerSpec.required_before_value
  ) {
    fail("source receipt consumer target does not match current pending checkout");
  }
  if (
    registryTarget.path !== registrySpec.path ||
    registryTarget.status_before !== registrySpec.required_status ||
    registryTarget.sha256 !== targets.registry.sha256 ||
    registryTarget.json_pointer !== registrySpec.executed_evidence_json_pointer ||
    registryTarget.before !== registrySpec.required_before_value
  ) {
    fail("source receipt FBA target does not match current pending checkout");
  }

  const execution = objectValue(document.execution, "source receipt execution");
  if (
    execution.all_commands_passed !== true ||
    execution.packet_generated_only_after_test_and_check_steps !== true ||
    execution.network_runtime_under_test !== false ||
    execution.database_used !== false ||
    execution.browser_used !== false ||
    execution.browser_evidence_pending !== true
  ) {
    fail("source receipt execution boundary drifted");
  }
  const governance = objectValue(document.governance, "source receipt governance");
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
    if (governance[key] !== false) fail(`source receipt governance ${key} must remain false`);
  }
  if (governance.later_admission_must_bind_rust_browser_and_source_lineage !== true) {
    fail("source receipt no longer requires later source-lineage admission");
  }
  return { runId, sourceCommit };
}

function expectedPublishedFacts() {
  return [
    "registered_surface_visible",
    "published_only_admission",
    "fly_canvas_unmounted",
    "document_authoring_unmounted",
    "registered_runtime_present",
    "owner_port_persistence_declared",
    "registered_property_panel_ready",
    "save_action_available_without_mutation",
  ];
}

function expectedHiddenFacts() {
  return ["registered_published_surface_absent", "metadata_surface_error_absent"];
}

function validateBrowserPacket(input, contract, browserContract, head) {
  const document = input.document;
  const specification = objectValue(contract.browser_input, "browser input");
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("browser packet format/status drifted");
  }
  if (canonicalCommit(document.source_commit, "browser source_commit") !== head) {
    fail("browser packet source_commit does not equal checkout HEAD");
  }
  const deploymentDigest = canonicalRepoDigest(document.deployment_digest, "browser deployment digest");
  canonicalIso(document.executed_at, "browser executed_at");
  verifyRetainedSourceHashes(document, browserContract, "source_files", "browser packet");

  const expectedProfiles = specification.required_profiles;
  if (
    canonicalJson(browserContract.profiles) !== canonicalJson(expectedProfiles) ||
    canonicalJson(Object.keys(objectValue(document.observations, "browser observations")).sort()) !==
      canonicalJson([...expectedProfiles].sort())
  ) {
    fail("browser profile set drifted");
  }
  const observations = document.observations;
  for (const profile of expectedProfiles) {
    const observation = objectValue(observations[profile], `browser ${profile} observation`);
    if (observation.passed !== true || observation.criticalFailures !== 0) {
      fail(`browser ${profile} observation did not pass cleanly`);
    }
    const facts = objectValue(observation.facts, `browser ${profile} facts`);
    const requiredFacts = profile === "published" ? expectedPublishedFacts() : expectedHiddenFacts();
    for (const fact of requiredFacts) {
      if (facts[fact] !== true) fail(`browser ${profile} fact ${fact} must be true`);
    }
  }

  if (
    document.retained_secrets !== false ||
    document.metadata_values_retained !== false ||
    document.browser_execution_only !== true ||
    document.consumer_properties_admission_pending !== true
  ) {
    fail("browser retention/admission boundary drifted");
  }

  const inputRecords = objectValue(document.input_records, "browser input_records");
  requirePacketRecord(inputRecords.editor_storage_state, "browser editor storage record");
  const routeHashes = objectValue(inputRecords.profile_url_sha256, "browser profile URL hashes");
  if (canonicalJson(Object.keys(routeHashes).sort()) !== canonicalJson([...expectedProfiles].sort())) {
    fail("browser profile URL hash set drifted");
  }
  for (const profile of expectedProfiles) {
    canonicalSha256(routeHashes[profile], `browser ${profile} URL hash`);
  }
  return { deploymentDigest, routeHashes };
}

function validateDeploymentProvenance(
  input,
  contract,
  head,
  sourceReceipt,
  browser,
  sourceInput,
  browserInput,
) {
  const document = input.document;
  const specification = objectValue(
    contract.deployment_provenance_input,
    "deployment provenance input",
  );
  if (document.format !== specification.format || document.status !== specification.required_status) {
    fail("deployment provenance format/status drifted");
  }
  if (canonicalCommit(document.source_commit, "deployment provenance source_commit") !== head) {
    fail("deployment provenance source_commit does not equal checkout HEAD");
  }
  if (
    canonicalRepoDigest(document.deployment_image_digest, "deployment provenance image digest") !==
    browser.deploymentDigest
  ) {
    fail("deployment provenance RepoDigest differs from browser packet");
  }
  canonicalIso(document.reviewed_at, "deployment provenance reviewed_at");

  const packetHashes = objectValue(
    document.input_packet_sha256,
    "deployment provenance input packet hashes",
  );
  if (
    canonicalSha256(packetHashes.source_receipt, "reviewed source receipt sha256") !== sourceInput.sha256 ||
    canonicalSha256(packetHashes.browser_evidence, "reviewed browser evidence sha256") !== browserInput.sha256
  ) {
    fail("deployment provenance packet hashes differ from supplied inputs");
  }

  const review = objectValue(document.review, "deployment provenance review");
  if (
    typeof review.reviewer_id !== "string" ||
    !REVIEWER_PATTERN.test(review.reviewer_id) ||
    review.classification !== specification.origin_to_repo_digest_binding_classification ||
    review.source_commit_reviewed !== true ||
    review.deployment_image_digest_reviewed !== true ||
    review.browser_profile_route_hashes_reviewed !== true ||
    review.source_workflow_index_reviewed !== true ||
    review.browser_workflow_index_reviewed !== true ||
    review.cryptographic_signature_present !== false
  ) {
    fail("deployment provenance maintainer review boundary drifted");
  }

  const workflow = objectValue(document.workflow_evidence, "deployment provenance workflow_evidence");
  const sourceWorkflow = objectValue(workflow.source, "deployment provenance source workflow");
  const browserWorkflow = objectValue(workflow.browser, "deployment provenance browser workflow");
  if (
    sourceWorkflow.context !== specification.source_workflow_index_context ||
    canonicalRunId(sourceWorkflow.run_id, "reviewed source workflow run id") !== sourceReceipt.runId ||
    canonicalCommit(sourceWorkflow.source_commit, "reviewed source workflow source_commit") !==
      sourceReceipt.sourceCommit ||
    sourceWorkflow.status !== "success" ||
    browserWorkflow.context !== specification.browser_workflow_index_context ||
    !canonicalRunId(browserWorkflow.run_id, "reviewed browser workflow run id") ||
    canonicalCommit(browserWorkflow.source_commit, "reviewed browser workflow source_commit") !== head ||
    browserWorkflow.status !== "success" ||
    workflow.exact_bound_commit_statuses_reviewed !== true
  ) {
    fail("deployment provenance workflow index review drifted");
  }

  const routeHashes = objectValue(document.profile_url_sha256, "deployment provenance profile URL hashes");
  if (canonicalJson(routeHashes) !== canonicalJson(browser.routeHashes)) {
    fail("deployment provenance route hashes differ from browser packet");
  }

  const binding = objectValue(document.binding, "deployment provenance binding");
  if (
    binding.origin_to_repo_digest !== specification.origin_to_repo_digest_binding_classification ||
    binding.cryptographic_origin_to_repo_digest_binding !== false ||
    binding.raw_profile_urls_retained !== false ||
    binding.credentials_retained !== false
  ) {
    fail("deployment provenance external-fact boundary drifted");
  }

  return {
    reviewerId: review.reviewer_id,
    browserRunId: canonicalRunId(browserWorkflow.run_id, "reviewed browser workflow run id"),
  };
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
  if (relative.startsWith("..") || path.isAbsolute(relative) || relative.length === 0) {
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

function main() {
  const options = parseArguments(process.argv.slice(2));
  const admissionRecord = repoJson(admissionContractPath, "admission source contract");
  const contract = admissionRecord.document;
  if (
    contract.format !== "pages_consumer_properties_admission_source_v1" ||
    contract.status !== "source_ready_maintainer_evidence_pending"
  ) {
    fail("admission source contract identity drifted");
  }
  const head = currentCommit();
  const sourceContract = repoJson(
    contract.source_execution_input.source_contract,
    "source execution source contract",
  ).document;
  const browserContract = repoJson(
    contract.browser_input.source_contract,
    "browser execution source contract",
  ).document;
  const targets = validateTargetPreconditions(contract);

  const sourceInput = jsonInput(options["--source-receipt"], "source execution receipt");
  const browserInput = jsonInput(options["--browser-evidence"], "browser evidence packet");
  const deploymentInput = jsonInput(
    options["--deployment-provenance"],
    "deployment provenance packet",
  );

  const sourceReceipt = validateSourceReceipt(sourceInput, contract, sourceContract, head, targets);
  const browser = validateBrowserPacket(browserInput, contract, browserContract, head);
  const deployment = validateDeploymentProvenance(
    deploymentInput,
    contract,
    head,
    sourceReceipt,
    browser,
    sourceInput,
    browserInput,
  );

  const output = {
    format: contract.output.format,
    status: contract.output.status,
    admitted_at: new Date().toISOString(),
    source_commit: head,
    source_receipt_commit: sourceReceipt.sourceCommit,
    browser_deployment_source_commit: head,
    deployment_image_digest: browser.deploymentDigest,
    workflow_evidence: {
      source: {
        context: contract.source_execution_input.run_index_context,
        run_id: sourceReceipt.runId,
        source_commit: sourceReceipt.sourceCommit,
        reviewed_status: "success",
      },
      browser: {
        context: contract.browser_input.run_index_context,
        run_id: deployment.browserRunId,
        source_commit: head,
        reviewed_status: "success",
      },
      review_classification: "maintainer_reviewed_external_fact",
      cryptographic_ci_attestation_claimed: false,
    },
    profile_url_sha256: browser.routeHashes,
    input_records: {
      source_receipt: { bytes: sourceInput.size, sha256: sourceInput.sha256 },
      browser_evidence: { bytes: browserInput.size, sha256: browserInput.sha256 },
      deployment_provenance: { bytes: deploymentInput.size, sha256: deploymentInput.sha256 },
    },
    source_files: sourceHashes(contract),
    admission: {
      source_receipt_ancestor_lineage_bound: true,
      source_receipt_required_sources_equal_current_checkout: true,
      browser_and_deployment_exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
      source_receipt_bound: true,
      browser_packet_bound: true,
      deployment_provenance_bound: true,
      source_and_browser_indexes_reviewed_on_bound_commits: true,
      no_source_drift_at_admission: true,
      registry_update_ready_for_later_evidence_containing_pr: true,
    },
    review: {
      reviewer_id: deployment.reviewerId,
      classification: "maintainer_reviewed_external_fact",
      cryptographic_signature_present: false,
    },
    boundaries: {
      network_requests_performed: false,
      browser_execution_performed: false,
      cargo_execution_performed: false,
      workflow_statuses_queried_by_runner: false,
      cryptographic_ci_attestation_claimed: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
      consumer_contract_mutated: false,
      fba_registry_mutated: false,
      executed_evidence_verified: false,
      terminal_inventory_complete_claimed: false,
      pages_ffa_promoted: false,
      page_builder_fba_promoted: false,
    },
  };

  writeAtomic(outputPath(contract, options["--output"]), output);
  console.log(
    `[admit-pages-consumer-properties] PASS checkout=${head} source_receipt=${sourceReceipt.sourceCommit} source_run=${sourceReceipt.runId} browser_run=${deployment.browserRunId} registry_update=pending`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
