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
const contractPath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
);
const gatePath = path.join(
  repoRoot,
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
);
const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_CAPTURE_BYTES = 8 * 1024 * 1024;
const MAX_SOURCE_FILES = 128;
const MAX_COMMANDS = 32;
const MAX_ARGS = 48;
const MAX_ARG_BYTES = 4096;
const commitPattern = /^[0-9a-f]{40}$/u;
const sha256Pattern = /^[0-9a-f]{64}$/u;
const deploymentDigestPattern = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;

function fail(message) {
  throw new Error(`Pages reference-consumer gate evidence failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
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
  const value = result.stdout.trim();
  if (!commitPattern.test(value)) fail("git HEAD is not a full lowercase SHA");
  return value;
}

function object(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function regularFile(location, label, maximum = MAX_INPUT_BYTES) {
  if (typeof location !== "string" || location.length === 0 || location.length > 16_384) {
    fail(`${label} path is invalid`);
  }
  const absolute = path.isAbsolute(location)
    ? path.resolve(location)
    : path.resolve(repoRoot, location);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > maximum) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { absolute, bytes, size: stats.size, sha256: sha256(bytes) };
}

function readJsonInput(location, label) {
  const record = regularFile(location, label);
  let document;
  try {
    document = JSON.parse(record.bytes.toString("utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
  object(document, label);
  return { record, document };
}

function requireEnvironment(name) {
  if (typeof name !== "string" || !/^[A-Z0-9_]{1,128}$/u.test(name)) {
    fail("contract contains an invalid evidence environment name");
  }
  const value = process.env[name];
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 16_384 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${name} must contain a bounded evidence file path`);
  }
  return value;
}

function requireDeploymentDigest(value, label) {
  if (typeof value !== "string" || !deploymentDigestPattern.test(value)) {
    fail(`${label} must be an immutable image RepoDigest`);
  }
  return value;
}

function requireSha256(value, label) {
  if (typeof value !== "string" || !sha256Pattern.test(value)) {
    fail(`${label} must be a lowercase SHA-256`);
  }
  return value;
}

function validateArtifactHttp(input, contract, head) {
  const document = input.document;
  const specification = contract.inputs?.artifact_http;
  if (
    document.format !== specification?.format ||
    document.status !== specification?.status ||
    document.source_commit !== head
  ) {
    fail("artifact/HTTP evidence identity, status, or source commit drifted");
  }
  const http = object(document.http, "artifact/HTTP http section");
  const docker = object(document.docker, "artifact/HTTP docker section");
  const digest = requireDeploymentDigest(
    http.deployment_image_digest,
    "artifact/HTTP deployment image digest",
  );
  if (!Array.isArray(docker.repo_digests) || !docker.repo_digests.includes(digest)) {
    fail("artifact/HTTP deployment digest is absent from Docker RepoDigests");
  }
  const boundaries = object(document.boundaries, "artifact/HTTP boundaries");
  if (
    boundaries.browser_edit_save_replay_expiry_executed !== false ||
    boundaries.tenant_rollout_executed !== false ||
    boundaries.ffa_promoted !== false ||
    boundaries.fba_promoted !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("artifact/HTTP packet overclaims browser, rollout, promotion, or source mutation");
  }
  const anonymous = object(document.anonymous_artifact, "artifact/HTTP anonymous artifact");
  if (!Array.isArray(anonymous.findings) || anonymous.findings.length !== 0) {
    fail("artifact/HTTP anonymous artifact evidence contains findings");
  }
  return digest;
}

function zeroFailureCounters(value, label) {
  const counters = object(value, label);
  for (const key of ["console_errors", "page_errors", "critical_request_failures"]) {
    if (counters[key] !== 0) fail(`${label}.${key} must be zero`);
  }
}

function validateBrowser(input, contract, head, artifactInput, deploymentDigest) {
  const document = input.document;
  const specification = contract.inputs?.browser;
  if (
    document.format !== specification?.format ||
    document.status !== specification?.status ||
    document.source_commit !== head
  ) {
    fail("browser evidence identity, status, or source commit drifted");
  }
  const target = object(document.target, "browser target");
  if (target.deployment_image_digest !== deploymentDigest) {
    fail("browser and artifact/HTTP deployment digests differ");
  }
  requireSha256(target.origin_sha256, "browser API origin hash");
  requireSha256(target.standalone_origin_sha256, "browser standalone-admin origin hash");
  const inputs = object(document.inputs, "browser inputs");
  const artifactHttp = object(inputs.artifact_http, "browser artifact/HTTP input");
  if (
    artifactHttp.bytes !== artifactInput.record.size ||
    artifactHttp.sha256 !== artifactInput.record.sha256
  ) {
    fail("browser packet is not bound to the supplied artifact/HTTP packet");
  }
  const authoring = object(document.authoring, "browser authoring evidence");
  zeroFailureCounters(authoring.failures, "browser authoring failures");
  const save = object(document.save, "browser save evidence");
  if (
    save.commit_request_count !== 1 ||
    save.response_status !== 200 ||
    save.replacement_revision_observed !== true ||
    save.replacement_project_hash_observed !== true ||
    save.reload_persistence_observed !== true ||
    save.raw_request_or_response_persisted !== false
  ) {
    fail("browser save evidence does not satisfy the reference gate boundary");
  }
  const replay = object(document.replay, "browser replay evidence");
  if (replay.exact_successful_request_rejected !== true) {
    fail("browser replay rejection evidence is missing");
  }
  for (const key of ["stale", "expiry"]) {
    const scenario = object(document[key], `browser ${key} evidence`);
    if (scenario.partial_write_absent_after_reload !== true) {
      fail(`browser ${key} scenario does not prove absence of a partial write`);
    }
    zeroFailureCounters(scenario.failures, `browser ${key} failures`);
  }
  const boundaries = object(document.boundaries, "browser boundaries");
  if (
    boundaries.tenant_rollout_executed !== false ||
    boundaries.ffa_promoted !== false ||
    boundaries.fba_promoted !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("browser packet overclaims rollout, promotion, or source mutation");
  }
  const privacy = object(document.privacy, "browser privacy");
  for (const key of [
    "storage_state_contents_persisted",
    "authorization_or_cookie_values_persisted",
    "session_ids_grants_or_proofs_persisted",
    "page_ids_component_ids_or_edited_text_persisted",
    "raw_html_persisted",
    "raw_request_or_response_bodies_persisted",
    "console_message_text_persisted",
    "traces_persisted",
    "screenshots_persisted",
    "videos_persisted",
  ]) {
    if (privacy[key] !== false) fail(`browser privacy.${key} must remain false`);
  }
}

function validateBoundedBodyRecord(value, label, expectedStatus = null) {
  const record = object(value, label);
  if (
    !Number.isInteger(record.response_body_bytes) ||
    record.response_body_bytes < 0 ||
    record.response_body_bytes > MAX_INPUT_BYTES
  ) {
    fail(`${label}.response_body_bytes is outside the bound`);
  }
  requireSha256(record.response_body_sha256, `${label}.response_body_sha256`);
  if (expectedStatus !== null && record.status !== expectedStatus) {
    fail(`${label}.status must be ${expectedStatus}`);
  }
  return record;
}

function validateTypedIntentDenial(value, label, capability, intent) {
  const denial = validateBoundedBodyRecord(value, label, 403);
  if (
    denial.code !== "FLY_CAPABILITY_DENIED" ||
    denial.capability !== capability ||
    denial.intent !== intent
  ) {
    fail(`${label} is not the expected typed capability denial`);
  }
}

function validateRolloutProfile(profileId, value) {
  const profile = object(value, `rollout matrix profile ${profileId}`);
  const expected = {
    all_on: {
      flags: [true, true, true, true],
      provider: "unobserved",
      preview: true,
      properties: "enabled",
      publish: "enabled",
      previewDisabled: false,
    },
    publish_off: {
      flags: [true, true, true, false],
      provider: "degraded",
      preview: true,
      properties: "enabled",
      publish: "disabled_or_hidden",
      previewDisabled: false,
    },
    preview_off: {
      flags: [true, false, true, false],
      provider: "degraded",
      preview: false,
      properties: "enabled",
      publish: "disabled_or_hidden",
      previewDisabled: true,
    },
    builder_off: {
      flags: [false, false, false, false],
      provider: "unavailable",
      preview: false,
      properties: "disabled_or_hidden",
      publish: "disabled_or_hidden",
      previewDisabled: true,
    },
  }[profileId];
  if (expected === undefined) fail(`unexpected rollout profile ${profileId}`);

  const flags = object(profile.flags, `${profileId} flags`);
  const actualFlags = [
    flags.builder_enabled,
    flags.preview_enabled,
    flags.properties_enabled,
    flags.publish_enabled,
  ];
  if (JSON.stringify(actualFlags) !== JSON.stringify(expected.flags)) {
    fail(`${profileId} rollout flags drifted`);
  }

  const settingsWrite = validateBoundedBodyRecord(profile.settings_write, `${profileId} settings write`, 200);
  if (settingsWrite.raw_request_or_response_persisted !== false) {
    fail(`${profileId} settings write persisted a raw request or response`);
  }

  const snapshot = validateBoundedBodyRecord(profile.server_snapshot, `${profileId} server snapshot`, 200);
  if (
    snapshot.raw_request_or_response_persisted !== false ||
    snapshot.tenant_match !== true ||
    snapshot.flags_match !== true ||
    snapshot.provider_health_observed !== false
  ) {
    fail(`${profileId} server snapshot does not satisfy rollout ownership`);
  }

  const reads = validateBoundedBodyRecord(profile.pages_owned_reads, `${profileId} Pages reads`, 200);
  if (
    reads.raw_request_or_response_persisted !== false ||
    reads.list_read !== true ||
    reads.document_read !== true
  ) {
    fail(`${profileId} does not preserve Pages-owned reads`);
  }

  const ui = object(profile.ui, `${profileId} UI evidence`);
  const allowedState = (actual, expectedState) =>
    expectedState === "disabled_or_hidden"
      ? actual === "disabled" || actual === "hidden"
      : actual === expectedState;
  if (
    ui.provider_state !== expected.provider ||
    ui.provider_health !== "unobserved" ||
    ui.preview_enabled !== expected.preview ||
    !allowedState(ui.properties, expected.properties) ||
    !allowedState(ui.publish, expected.publish)
  ) {
    fail(`${profileId} UI rollout state drifted`);
  }

  const preview = validateBoundedBodyRecord(profile.preview_ssr, `${profileId} preview SSR`);
  if (preview.capability_disabled !== expected.previewDisabled) {
    fail(`${profileId} preview SSR capability outcome drifted`);
  }
  if (expected.previewDisabled) {
    if (preview.status < 400) fail(`${profileId} disabled preview returned a success status`);
  } else if (preview.status >= 400) {
    fail(`${profileId} enabled preview returned an error status`);
  }

  if (profileId === "all_on") {
    const publish = object(profile.publish_dry, "all_on publish dry evidence");
    if (
      publish.ui_capability_enabled !== true ||
      publish.mutating_save_request_sent !== false
    ) {
      fail("all_on publish dry evidence must remain non-mutating and enabled");
    }
    if (profile.properties_denial !== null) {
      fail("all_on must not retain a properties denial");
    }
  } else {
    validateTypedIntentDenial(profile.publish_dry, `${profileId} publish denial`, "publish", "save");
    if (profileId === "builder_off") {
      validateTypedIntentDenial(
        profile.properties_denial,
        "builder_off properties denial",
        "properties",
        "rename_page",
      );
    } else if (profile.properties_denial !== null) {
      fail(`${profileId} must not retain a properties denial`);
    }
  }
}

function validateRolloutMatrix(input, contract, gate, head, browserInput, deploymentDigest) {
  const document = input.document;
  const specification = contract.inputs?.rollout_matrix;
  if (
    document.format !== specification?.format ||
    document.status !== specification?.status ||
    document.source_commit !== head
  ) {
    fail("rollout matrix identity, status, or source commit drifted");
  }

  const inputs = object(document.inputs, "rollout matrix inputs");
  const predecessor = object(inputs.browser_predecessor, "rollout matrix browser predecessor");
  if (
    predecessor.bytes !== browserInput.record.size ||
    predecessor.sha256 !== browserInput.record.sha256
  ) {
    fail("rollout matrix is not bound to the supplied browser predecessor packet");
  }

  const browserTarget = object(browserInput.document.target, "browser target");
  const target = object(document.target, "rollout matrix target");
  if (target.deployment_image_digest !== deploymentDigest) {
    fail("rollout matrix API deployment digest differs from the artifact/browser chain");
  }
  if (
    requireSha256(target.api_origin_sha256, "rollout matrix API origin hash") !==
      browserTarget.origin_sha256 ||
    requireSha256(target.admin_origin_sha256, "rollout matrix admin origin hash") !==
      browserTarget.standalone_origin_sha256
  ) {
    fail("rollout matrix origins differ from the supplied browser predecessor");
  }

  const originalSettings = object(document.original_settings, "rollout matrix original settings");
  requireSha256(originalSettings.semantic_sha256, "rollout matrix original settings hash");
  if (
    originalSettings.raw_settings_persisted !== false ||
    originalSettings.restored !== true ||
    originalSettings.restore_verified !== true
  ) {
    fail("rollout matrix does not prove restoration of the original Pages settings");
  }

  const profiles = object(document.profiles, "rollout matrix profiles");
  const requiredProfiles = gate.gate?.required_profiles;
  if (
    !Array.isArray(requiredProfiles) ||
    JSON.stringify(requiredProfiles) !==
      JSON.stringify(["all_on", "publish_off", "preview_off", "builder_off"])
  ) {
    fail("source gate required rollout profiles drifted");
  }
  const actualProfiles = Object.keys(profiles).sort();
  const expectedProfiles = [...requiredProfiles].sort();
  if (JSON.stringify(actualProfiles) !== JSON.stringify(expectedProfiles)) {
    fail("rollout matrix profile set is incomplete or contains unexpected profiles");
  }
  for (const profileId of requiredProfiles) {
    validateRolloutProfile(profileId, profiles[profileId]);
  }

  const boundaries = object(document.boundaries, "rollout matrix boundaries");
  if (
    boundaries.runtime_matrix_executed !== true ||
    boundaries.owner_review_pending !== true ||
    boundaries.gate_accepted !== false ||
    boundaries.forum_wave_accepted !== false ||
    boundaries.provider_health_observed !== false ||
    boundaries.ffa_promoted !== false ||
    boundaries.fba_promoted !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("rollout matrix execution/promotion boundaries drifted");
  }

  const privacy = object(document.privacy, "rollout matrix privacy");
  for (const key of [
    "tenant_slug_or_id_persisted",
    "page_id_or_admin_route_persisted",
    "authorization_or_cookie_values_persisted",
    "storage_state_contents_persisted",
    "tokens_or_session_ids_persisted",
    "raw_module_settings_persisted",
    "raw_graphql_bodies_persisted",
    "raw_preview_request_or_response_persisted",
    "raw_browser_intent_response_persisted",
    "raw_html_persisted",
    "traces_persisted",
    "screenshots_persisted",
    "videos_persisted",
  ]) {
    if (privacy[key] !== false) fail(`rollout matrix privacy.${key} must remain false`);
  }
}

function sourceHashes(contract) {
  const files = contract.required_source_files;
  if (!Array.isArray(files) || files.length === 0 || files.length > MAX_SOURCE_FILES) {
    fail("required source-file set is outside the bounded contract");
  }
  const entries = files.map((relativePath) => {
    if (
      typeof relativePath !== "string" ||
      relativePath.length === 0 ||
      relativePath.length > 4096 ||
      relativePath.includes("\0")
    ) {
      fail("required source-file path is invalid");
    }
    const absolute = path.resolve(repoRoot, relativePath);
    const relative = path.relative(repoRoot, absolute);
    if (relative.startsWith("..") || path.isAbsolute(relative)) {
      fail(`required source file escapes repository: ${relativePath}`);
    }
    const record = regularFile(absolute, `source file ${relativePath}`, MAX_CAPTURE_BYTES);
    return [relativePath, record.sha256];
  });
  if (new Set(entries.map(([relativePath]) => relativePath)).size !== entries.length) {
    fail("required source-file set contains duplicates");
  }
  return Object.fromEntries(entries);
}

function commandKey(command) {
  return `${command.program}\u0000${command.args.join("\u0000")}`;
}

function validateCommands(commands, label) {
  if (!Array.isArray(commands) || commands.length === 0 || commands.length > MAX_COMMANDS) {
    fail(`${label} command set is outside the bounded contract`);
  }
  const ids = new Set();
  const commandKeys = new Set();
  return commands.map((command) => {
    object(command, `${label} command`);
    if (
      typeof command.id !== "string" ||
      !/^[a-z0-9_]{1,64}$/u.test(command.id) ||
      ids.has(command.id)
    ) {
      fail(`${label} command id is invalid or duplicated`);
    }
    ids.add(command.id);
    if (!Array.isArray(command.args) || command.args.length === 0 || command.args.length > MAX_ARGS) {
      fail(`${label} command ${command.id} has invalid argv`);
    }
    for (const arg of command.args) {
      if (
        typeof arg !== "string" ||
        arg.length === 0 ||
        Buffer.byteLength(arg, "utf8") > MAX_ARG_BYTES ||
        /[\u0000\r\n]/u.test(arg)
      ) {
        fail(`${label} command ${command.id} contains an invalid argument`);
      }
    }
    if (command.program === "cargo") {
      if (command.args[0] !== "test") {
        fail(`${label} command ${command.id} must remain a cargo test command`);
      }
    } else if (command.program === "node") {
      const script = command.args[0];
      if (!script.endsWith(".mjs")) {
        fail(`${label} command ${command.id} must execute an .mjs verifier`);
      }
      const absolute = path.resolve(repoRoot, script);
      const relative = path.relative(repoRoot, absolute);
      if (relative.startsWith("..") || path.isAbsolute(relative)) {
        fail(`${label} command ${command.id} verifier escapes repository`);
      }
      regularFile(absolute, `${label} verifier ${script}`, MAX_CAPTURE_BYTES);
    } else {
      fail(`${label} command ${command.id} uses a non-allowlisted program`);
    }
    const key = commandKey(command);
    if (commandKeys.has(key)) fail(`${label} command argv is duplicated`);
    commandKeys.add(key);
    return { id: command.id, program: command.program, args: [...command.args] };
  });
}

function validateGateCommandParity(contract, gate) {
  const guardScripts = new Set(
    validateCommands(contract.source_guards, "source guard")
      .map((command) => (command.program === "node" ? command.args[0] : null))
      .filter(Boolean),
  );
  const required = gate.gate?.required_source_guards;
  if (!Array.isArray(required) || required.length === 0) {
    fail("source gate required_source_guards is missing");
  }
  for (const script of required) {
    if (!guardScripts.has(script)) {
      fail(`execution contract source guards are missing gate requirement ${script}`);
    }
  }
}

function boundedCapture(value, label) {
  const buffer = Buffer.isBuffer(value) ? value : Buffer.from(value ?? "");
  if (buffer.byteLength > MAX_CAPTURE_BYTES) fail(`${label} exceeds the capture bound`);
  return { bytes: buffer.byteLength, sha256: sha256(buffer) };
}

function execute(command) {
  const result = spawnSync(command.program, command.args, {
    cwd: repoRoot,
    shell: false,
    encoding: null,
    maxBuffer: MAX_CAPTURE_BYTES,
    env: process.env,
  });
  if (result.error) fail(`${command.id} could not execute: ${result.error.message}`);
  if (!Number.isInteger(result.status)) fail(`${command.id} returned no integer exit status`);
  const observation = {
    id: command.id,
    program: command.program,
    args: command.args,
    status: result.status,
    stdout: boundedCapture(result.stdout, `${command.id} stdout`),
    stderr: boundedCapture(result.stderr, `${command.id} stderr`),
  };
  if (result.status !== 0) {
    throw new Error(`Pages reference-consumer gate command ${command.id} failed with status ${result.status}`);
  }
  return observation;
}

function outputPath(contract) {
  const environment = contract.output?.environment;
  const defaultPath = contract.output?.default_path;
  if (typeof environment !== "string" || typeof defaultPath !== "string") {
    fail("output contract is invalid");
  }
  const requested = process.env[environment] ?? defaultPath;
  if (
    typeof requested !== "string" ||
    requested.length === 0 ||
    requested.length > 16_384 ||
    /[\u0000\r\n]/u.test(requested)
  ) {
    fail(`${environment} is outside the bounded output path input`);
  }
  const absolute = path.isAbsolute(requested)
    ? path.resolve(requested)
    : path.resolve(repoRoot, requested);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("candidate output must remain inside repository target/");
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
  const contract = JSON.parse(readFileSync(contractPath, "utf8"));
  const gate = JSON.parse(readFileSync(gatePath, "utf8"));
  if (
    contract.schema_version !== 1 ||
    contract.module !== "pages" ||
    contract.packet !== "pages-reference-consumer-gate-candidate" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("execution contract identity drifted");
  }
  if (
    gate.artifact !== "pages_reference_consumer_gate_source" ||
    gate.mode !== "source_ready" ||
    gate.accepted !== false ||
    gate.current_boundary?.execution_gate !== "pending" ||
    gate.current_boundary?.provider_health !== "unobserved"
  ) {
    fail("source gate must remain source-ready, unaccepted, pending, and health-unobserved");
  }
  if (
    contract.output?.format !== "pages_reference_consumer_gate_candidate_v1" ||
    contract.output?.status !== "component_execution_passed_owner_review_pending" ||
    contract.output?.automatic_source_mutation !== false ||
    contract.output?.automatic_gate_acceptance !== false ||
    contract.output?.automatic_ffa_fba_promotion !== false
  ) {
    fail("candidate output boundary drifted");
  }

  validateGateCommandParity(contract, gate);
  const sourceGuardCommands = validateCommands(contract.source_guards, "source guard");
  const focusedTestCommands = validateCommands(contract.focused_tests, "focused test");
  if (focusedTestCommands.some((command) => command.program !== "cargo")) {
    fail("focused test command set must contain only cargo test commands");
  }

  const head = currentCommit();
  const artifactInput = readJsonInput(
    requireEnvironment(contract.inputs.artifact_http.environment),
    "artifact/HTTP evidence",
  );
  const browserInput = readJsonInput(
    requireEnvironment(contract.inputs.browser.environment),
    "browser evidence",
  );
  const rolloutMatrixInput = readJsonInput(
    requireEnvironment(contract.inputs.rollout_matrix.environment),
    "rollout matrix evidence",
  );
  const deploymentDigest = validateArtifactHttp(artifactInput, contract, head);
  validateBrowser(browserInput, contract, head, artifactInput, deploymentDigest);
  validateRolloutMatrix(
    rolloutMatrixInput,
    contract,
    gate,
    head,
    browserInput,
    deploymentDigest,
  );

  const output = outputPath(contract);
  rmSync(output, { force: true });
  const sources = sourceHashes(contract);
  const sourceGuards = sourceGuardCommands.map(execute);
  const focusedTests = focusedTestCommands.map(execute);

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    source_commit: head,
    deployment_image_digest: deploymentDigest,
    generated_at: new Date().toISOString(),
    inputs: {
      artifact_http: {
        bytes: artifactInput.record.size,
        sha256: artifactInput.record.sha256,
      },
      browser: {
        bytes: browserInput.record.size,
        sha256: browserInput.record.sha256,
      },
      rollout_matrix: {
        bytes: rolloutMatrixInput.record.size,
        sha256: rolloutMatrixInput.record.sha256,
      },
    },
    source_sha256: sources,
    source_guards: sourceGuards,
    focused_tests: focusedTests,
    candidate: {
      all_source_guards_passed: true,
      all_focused_tests_passed: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
      artifact_http_browser_chain_bound: true,
      rollout_matrix_browser_chain_bound: true,
      rollout_matrix_profiles_passed: true,
      rollout_matrix_settings_restored: true,
      provider_health: "unobserved",
      owner_signoff: "pending",
      rollback_decision: "pending",
      gate_acceptance: "pending",
    },
    boundaries: {
      canonical_source_mutated: false,
      gate_accepted: false,
      forum_wave_accepted: false,
      ffa_promoted: false,
      fba_promoted: false,
    },
    privacy: {
      raw_input_packets_persisted: false,
      raw_command_output_persisted: false,
      tenant_or_actor_ids_persisted: false,
      credentials_sessions_grants_or_proofs_persisted: false,
      raw_html_or_http_bodies_persisted: false,
      raw_rollout_settings_persisted: false,
      database_urls_persisted: false,
    },
  });
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
