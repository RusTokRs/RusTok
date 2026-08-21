#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
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
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json",
);
const MAX_INPUT_BYTES = 1024 * 1024;
const MAX_SETTINGS_BYTES = 512 * 1024;
const MAX_RESPONSE_BYTES = 1024 * 1024;
const REQUEST_TIMEOUT_MS = 20_000;
const CLOCK_SKEW_MS = 5 * 60 * 1000;
const COMMIT_PATTERN = /^[0-9a-f]{40}$/u;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REPO_DIGEST_PATTERN = /^[^@\s]+@sha256:[0-9a-f]{64}$/u;
const CONFLICT_CODE = "MODULE_SETTINGS_SNAPSHOT_CONFLICT";

const tenantModulesQuery =
  "query FfaFbaPromotionTenantModules($limit: Int) { tenantModules(limit: $limit) { moduleSlug enabled settings revision } }";
const casMutation =
  "mutation FfaFbaPromotionSettings($moduleSlug: String!, $expectedEnabled: Boolean!, $expectedSettings: String!, $settings: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) { compareAndSwapModuleSettings(moduleSlug: $moduleSlug, expectedEnabled: $expectedEnabled, expectedSettings: $expectedSettings, settings: $settings, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) { moduleSlug enabled settings revision } }";
const rolloutSnapshotQuery =
  "query FfaFbaPromotionRolloutSnapshot { pageBuilderRolloutSnapshot { tenantSlug builderEnabled previewEnabled propertiesEnabled publishEnabled providerHealthObserved } }";

class SnapshotConflictError extends Error {
  constructor(message, response) {
    super(message);
    this.name = "SnapshotConflictError";
    this.response = response;
  }
}

class MutationOutcomeAmbiguousError extends Error {
  constructor(message, response = null) {
    super(message);
    this.name = "MutationOutcomeAmbiguousError";
    this.response = response;
  }
}

function fail(message) {
  throw new Error(`Forum FFA/FBA promotion execution failed: ${message}`);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, canonicalize(nested)]),
    );
  }
  return value;
}

function canonicalJson(value) {
  return JSON.stringify(canonicalize(value));
}

function objectValue(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function parseArguments(argv) {
  const options = {};
  const accepted = new Set(["--promotion-review", "--output"]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: execute-forum-page-builder-ffa-fba-promotion.mjs " +
          "--promotion-review FILE [--output FILE]\n" +
          "target/auth inputs are read from the execution contract environment variables",
      );
      process.exit(0);
    }
    if (!accepted.has(argument)) fail(`unknown argument ${argument}`);
    const value = argv[index + 1];
    if (!value) fail(`${argument} requires a value`);
    options[argument.slice(2).replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase())] = value;
    index += 1;
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
  return path.isAbsolute(candidate) ? path.resolve(candidate) : path.resolve(repoRoot, candidate);
}

function repositoryPath(relativePath, label) {
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
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail(`${label} path escapes the repository root`);
  }
  return absolute;
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

function jsonInput(candidate, label) {
  const location = resolveInput(candidate, label);
  const record = regularFile(location, label);
  try {
    const document = JSON.parse(record.bytes.toString("utf8"));
    objectValue(document, label);
    return { location, document, ...record };
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
}

function requiredEnvironment(name, maximumLength = 16_384) {
  const value = process.env[name];
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${name} must be a bounded non-empty environment value`);
  }
  return value;
}

function currentCommit() {
  const result = spawnSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) fail("git rev-parse HEAD failed");
  const commit = result.stdout.trim().toLowerCase();
  if (!COMMIT_PATTERN.test(commit)) fail("checkout HEAD is not a canonical Git commit");
  return commit;
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
    fail(`${label} is invalid`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be canonical ISO-8601 UTC`);
  }
  return { value, milliseconds };
}

function requireFalse(record, key, label) {
  if (record[key] !== false) fail(`${label}.${key} must remain false`);
}

function isLocalHost(hostname) {
  const normalized = hostname.toLowerCase();
  return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(normalized);
}

function canonicalOrigin(value, label) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} must be an absolute HTTP(S) origin`);
  }
  if (
    !["http:", "https:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !["", "/"].includes(parsed.pathname)
  ) {
    fail(`${label} must be credential-free and contain no path/query/fragment`);
  }
  if (parsed.protocol === "http:" && !isLocalHost(parsed.hostname)) {
    fail(`${label} must use HTTPS unless the target is localhost/loopback`);
  }
  return parsed.origin;
}

function tenantSlug(value) {
  if (
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 128 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail("promotion tenant slug must be a bounded header-safe value");
  }
  return value;
}

function authorizationHeader(value) {
  if (value.length > 8192) fail("promotion auth token is too large");
  return /^Bearer\s+/iu.test(value) ? value : `Bearer ${value}`;
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
    fail("promotion execution output must remain inside repository target/");
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

function parseSettings(raw, label) {
  if (typeof raw !== "string" || Buffer.byteLength(raw, "utf8") > MAX_SETTINGS_BYTES) {
    fail(`${label} must be a bounded settings JSON string`);
  }
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    fail(`${label} is invalid JSON: ${error.message}`);
  }
  return objectValue(parsed, label);
}

function withAllOn(original) {
  const cloned = JSON.parse(JSON.stringify(original));
  const builder =
    cloned.builder !== null && typeof cloned.builder === "object" && !Array.isArray(cloned.builder)
      ? { ...cloned.builder }
      : {};
  const nested = (value) =>
    value !== null && typeof value === "object" && !Array.isArray(value) ? { ...value } : {};
  builder.enabled = true;
  builder.preview = { ...nested(builder.preview), enabled: true };
  builder.properties = { ...nested(builder.properties), enabled: true };
  builder.publish = { ...nested(builder.publish), enabled: true };
  cloned.builder = builder;
  return cloned;
}

function hasAllOnFlags(settings) {
  const builder = settings.builder;
  if (builder === null || typeof builder !== "object" || Array.isArray(builder)) return false;
  const enabled = (value) =>
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    value.enabled === true;
  return (
    builder.enabled === true &&
    enabled(builder.preview) &&
    enabled(builder.properties) &&
    enabled(builder.publish)
  );
}

function sourceHashes(contract) {
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("promotion execution contract has no required source files");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const location = repositoryPath(relativePath, `source file ${relativePath}`);
      const record = regularFile(location, `source file ${relativePath}`, 4 * 1024 * 1024);
      return [relativePath, record.sha256];
    }),
  );
}

function validateReview(contract, review, head, deploymentDigest) {
  const document = review.document;
  if (
    document.format !== contract.predecessor?.format ||
    document.status !== contract.predecessor?.required_status
  ) {
    fail("promotion execution requires an approved FFA/FBA promotion-review packet");
  }
  if (canonicalCommit(document.source_commit, "promotion review source_commit") !== head) {
    fail("promotion review source_commit does not equal checkout HEAD");
  }
  const packetDigest = canonicalRepoDigest(
    document.deployment_image_digest,
    "promotion review deployment_image_digest",
  );
  if (packetDigest !== deploymentDigest) {
    fail("execution deployment RepoDigest does not equal approved promotion review");
  }
  const reviewedAt = canonicalIso(document.reviewed_at, "promotion review reviewed_at");
  const now = Date.now();
  if (reviewedAt.milliseconds > now + CLOCK_SKEW_MS) {
    fail("promotion review reviewed_at is implausibly in the future");
  }

  const observed = objectValue(document.observed_acceptance, "promotion review observed_acceptance");
  canonicalSha256(observed.sha256, "promotion review observed_acceptance.sha256");
  const nextDueAt = canonicalIso(
    observed.wave_next_due_at,
    "promotion review observed_acceptance.wave_next_due_at",
  );
  if (nextDueAt.milliseconds <= now) {
    fail("approved promotion review is stale because the retained observed Wave lease expired");
  }
  if (
    observed.prior_owner_decision !== "accept_observed_wave_evidence" ||
    observed.freshness_verifier_passed_at_prior_review !== true ||
    observed.admission_lineage_verifier_passed_at_prior_review !== true
  ) {
    fail("promotion review no longer retains accepted observed-Wave freshness/lineage evidence");
  }

  const promotion = objectValue(document.promotion_review, "promotion review decision");
  if (
    promotion.decision !== contract.predecessor?.promotion_decision_must_equal ||
    JSON.stringify(promotion.targets) !== JSON.stringify(contract.predecessor?.targets_must_equal) ||
    promotion.identity_is_operator_assertion !== true ||
    promotion.cryptographic_signature_verified !== false
  ) {
    fail("promotion review decision/target boundary drifted");
  }

  const boundaries = objectValue(document.boundaries, "promotion review boundaries");
  if (
    boundaries.review_only !== true ||
    boundaries.approval_is_not_control_plane_execution !== true ||
    boundaries.separate_control_plane_execution_required !== true
  ) {
    fail("promotion review does not authorize a separate execution step");
  }
  for (const key of [
    "control_plane_or_rollout_mutated",
    "pages_or_forum_persistence_mutated",
    "current_provider_health_asserted",
    "cryptographic_origin_to_repo_digest_binding_claimed",
    "forum_wave_promoted",
    "ffa_promoted",
    "fba_promoted",
  ]) requireFalse(boundaries, key, "promotion review boundaries");

  const privacy = objectValue(document.privacy, "promotion review privacy");
  for (const key of [
    "raw_input_path_persisted",
    "raw_metrics_or_trace_values_persisted",
    "forum_content_persisted",
    "tenant_or_actor_identifiers_persisted",
    "free_text_reason_persisted",
  ]) requireFalse(privacy, key, "promotion review privacy");

  return { reviewedAt: reviewedAt.value, nextDueAt: nextDueAt.value, packetDigest };
}

async function readBoundedBody(response, label) {
  if (response.body === null) return Buffer.alloc(0);
  const reader = response.body.getReader();
  const chunks = [];
  let total = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      fail(`${label} response exceeded ${MAX_RESPONSE_BYTES} bytes`);
    }
    chunks.push(Buffer.from(value));
  }
  return Buffer.concat(chunks, total);
}

function responseRecord(status, body) {
  return {
    status,
    response_body_bytes: body.length,
    response_body_sha256: sha256(body),
    raw_request_or_response_persisted: false,
  };
}

async function graphqlRequest(target, query, variables, label, mutation = false) {
  const body = Buffer.from(JSON.stringify({ query, variables }), "utf8");
  if (body.length > MAX_INPUT_BYTES) fail(`${label} request is outside the bounded size`);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  let response;
  try {
    response = await fetch(`${target.apiOrigin}${target.graphqlPath}`, {
      method: "POST",
      headers: {
        accept: "application/json",
        authorization: target.authorization,
        "content-type": "application/json",
        "x-tenant-slug": target.tenantSlug,
      },
      body,
      signal: controller.signal,
    });
  } catch (error) {
    if (mutation) {
      throw new MutationOutcomeAmbiguousError(`${label} transport failed: ${error.message}`);
    }
    fail(`${label} transport failed: ${error.message}`);
  } finally {
    clearTimeout(timer);
  }

  let responseBody;
  try {
    responseBody = await readBoundedBody(response, label);
  } catch (error) {
    if (mutation) {
      throw new MutationOutcomeAmbiguousError(`${label} response could not be bounded: ${error.message}`);
    }
    throw error;
  }
  const record = responseRecord(response.status, responseBody);
  if (response.status !== 200) {
    if (mutation) {
      throw new MutationOutcomeAmbiguousError(`${label} returned HTTP ${response.status}`, record);
    }
    fail(`${label} returned HTTP ${response.status}`);
  }

  let envelope;
  try {
    envelope = JSON.parse(responseBody.toString("utf8"));
  } catch (error) {
    if (mutation) {
      throw new MutationOutcomeAmbiguousError(`${label} returned invalid JSON: ${error.message}`, record);
    }
    fail(`${label} returned invalid JSON: ${error.message}`);
  }
  envelope = objectValue(envelope, `${label} response`);
  const errors = Array.isArray(envelope.errors) ? envelope.errors : [];
  if (errors.length > 0) {
    if (mutation) {
      const conflict = errors.find(
        (entry) =>
          entry !== null &&
          typeof entry === "object" &&
          entry.extensions !== null &&
          typeof entry.extensions === "object" &&
          entry.extensions.code === CONFLICT_CODE &&
          entry.extensions.requires_rereview === true,
      );
      if (conflict !== undefined) {
        throw new SnapshotConflictError(`${label} rejected the stale settings snapshot`, record);
      }
      throw new MutationOutcomeAmbiguousError(`${label} returned non-conflict GraphQL errors`, record);
    }
    fail(`${label} returned GraphQL errors`);
  }
  return {
    data: objectValue(envelope.data, `${label} data`),
    response: record,
  };
}

async function loadPagesModule(target) {
  const result = await graphqlRequest(
    target,
    tenantModulesQuery,
    { limit: 100 },
    "tenantModules promotion snapshot",
  );
  const modules = result.data.tenantModules;
  if (!Array.isArray(modules)) fail("tenantModules did not return an array");
  const pages = modules.find(
    (entry) =>
      entry !== null &&
      typeof entry === "object" &&
      entry.moduleSlug === "pages",
  );
  if (pages === undefined || pages.enabled !== true) {
    fail("Pages module must be enabled for FFA/FBA control-plane promotion");
  }
  if (!Number.isSafeInteger(pages.revision) || pages.revision < 0) {
    fail("Pages module must return a non-negative lifecycle revision");
  }
  return {
    settings: parseSettings(pages.settings, "Pages module settings"),
    revision: pages.revision,
    response: result.response,
  };
}

async function compareAndSwapPagesSettings(
  target,
  expectedRevision,
  expectedSettings,
  settings,
  label,
) {
  const result = await graphqlRequest(
    target,
    casMutation,
    {
      moduleSlug: "pages",
      expectedEnabled: true,
      expectedSettings: JSON.stringify(expectedSettings),
      settings: JSON.stringify(settings),
      expectedRevision,
      idempotencyKey: randomUUID(),
    },
    label,
    true,
  );
  const module = objectValue(result.data.compareAndSwapModuleSettings, `${label} result`);
  if (module.moduleSlug !== "pages" || module.enabled !== true) {
    throw new MutationOutcomeAmbiguousError(`${label} returned the wrong module identity`, result.response);
  }
  if (!Number.isSafeInteger(module.revision) || module.revision < 0) {
    throw new MutationOutcomeAmbiguousError(`${label} returned an invalid lifecycle revision`, result.response);
  }
  let returnedSettings;
  try {
    returnedSettings = parseSettings(module.settings, `${label} returned settings`);
  } catch (error) {
    throw new MutationOutcomeAmbiguousError(error.message, result.response);
  }
  return { settings: returnedSettings, revision: module.revision, response: result.response };
}

async function verifyPromotedPostcondition(target, appliedSettings) {
  const module = await loadPagesModule(target);
  if (canonicalJson(module.settings) !== canonicalJson(appliedSettings)) {
    fail("postcondition tenantModules settings differ from confirmed applied settings");
  }
  if (!hasAllOnFlags(module.settings)) {
    fail("postcondition tenantModules settings are not the all_on profile");
  }
  const snapshot = await graphqlRequest(
    target,
    rolloutSnapshotQuery,
    {},
    "pageBuilderRolloutSnapshot promotion postcondition",
  );
  const value = objectValue(snapshot.data.pageBuilderRolloutSnapshot, "Page Builder rollout snapshot");
  if (
    value.tenantSlug !== target.tenantSlug ||
    value.builderEnabled !== true ||
    value.previewEnabled !== true ||
    value.propertiesEnabled !== true ||
    value.publishEnabled !== true
  ) {
    fail("pageBuilderRolloutSnapshot does not confirm the all_on promotion profile");
  }
  return {
    tenant_modules: module.response,
    rollout_snapshot: snapshot.response,
    provider_health_observed: value.providerHealthObserved === true,
  };
}

function receiptBase(contract, head, hashes, review, reviewValidation, target, before, requested) {
  return {
    format: contract.output.format,
    generated_at: new Date().toISOString(),
    source_commit: head,
    source_sha256: hashes,
    promotion_review: {
      bytes: review.size,
      sha256: review.sha256,
      reviewed_at: reviewValidation.reviewedAt,
      observed_wave_next_due_at: reviewValidation.nextDueAt,
      decision: "approve_ffa_fba_promotion_review",
      raw_input_path_persisted: false,
    },
    target: {
      api_origin_sha256: sha256(target.apiOrigin),
      deployment_image_digest: reviewValidation.packetDigest,
      tenant_slug_sha256: sha256(target.tenantSlug),
      graphql_path: target.graphqlPath,
      raw_origin_or_tenant_persisted: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
    },
    before: {
      enabled: true,
      settings_semantic_sha256: sha256(canonicalJson(before.settings)),
      tenant_modules_read: before.response,
      raw_settings_persisted: false,
    },
    requested: {
      profile: "all_on",
      settings_semantic_sha256: sha256(canonicalJson(requested)),
      preserve_non_builder_settings: true,
      raw_settings_persisted: false,
    },
    privacy: {
      authorization_or_cookie_values_persisted: false,
      tenant_slug_or_id_persisted: false,
      api_origin_persisted: false,
      raw_module_settings_persisted: false,
      raw_graphql_request_or_response_persisted: false,
      raw_forum_content_persisted: false,
      raw_metrics_or_traces_persisted: false,
    },
  };
}

function readinessBoundary() {
  return {
    ffa_promoted: false,
    fba_promoted: false,
    registry_or_local_plan_status_mutated: false,
    separate_evidence_backed_governance_change_required: true,
  };
}

function writeFailureReceipt(output, base, status, details) {
  writeAtomic(output, {
    ...base,
    status,
    ...details,
    readiness: readinessBoundary(),
  });
}

function mutationReceipt(applied, appliedHash) {
  return {
    outcome: "confirmed",
    response: applied.response,
    control_plane_execution_confirmed: true,
    tenant_rollout_mutation_confirmed: true,
    applied_settings_semantic_sha256: appliedHash,
  };
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (!options.promotionReview) fail("missing required promotionReview input");

  const contractRecord = jsonInput(contractPath, "Forum FFA/FBA promotion execution source contract");
  const contract = contractRecord.document;
  if (
    contract.format !== "forum_page_builder_ffa_fba_promotion_execution_source_v1" ||
    contract.status !== "source_ready_maintainer_execution_pending" ||
    contract.target?.write_operation !== "compareAndSwapModuleSettings" ||
    contract.target?.promotion_profile !== "all_on" ||
    JSON.stringify(contract.target?.required_permissions) !==
      JSON.stringify(["modules:manage", "pages:read"])
  ) {
    fail("promotion execution source contract identity drifted");
  }
  if (contract.mutation?.cas_conflict_code !== CONFLICT_CODE) {
    fail("promotion execution CAS conflict contract drifted");
  }

  const head = currentCommit();
  const deploymentDigest = canonicalRepoDigest(
    requiredEnvironment(contract.target.deployment_image_digest_environment, 1024),
    contract.target.deployment_image_digest_environment,
  );
  const apiOrigin = canonicalOrigin(
    requiredEnvironment(contract.target.api_origin_environment, 4096),
    contract.target.api_origin_environment,
  );
  const routedTenantSlug = tenantSlug(
    requiredEnvironment(contract.target.tenant_slug_environment, 128),
  );
  const authorization = authorizationHeader(
    requiredEnvironment(contract.target.auth_token_environment, 8192),
  );
  const review = jsonInput(options.promotionReview, "Forum FFA/FBA approved promotion review");
  const reviewValidation = validateReview(contract, review, head, deploymentDigest);
  const output = outputPath(contract, options.output);
  rmSync(output, { force: true });
  const hashes = sourceHashes(contract);
  const target = {
    apiOrigin,
    tenantSlug: routedTenantSlug,
    authorization,
    graphqlPath: contract.target.graphql_path,
  };

  const before = await loadPagesModule(target);
  const requested = withAllOn(before.settings);
  if (canonicalJson(before.settings) === canonicalJson(requested)) {
    fail("Pages settings already match all_on; no new control-plane execution evidence can be emitted");
  }
  const base = receiptBase(
    contract,
    head,
    hashes,
    review,
    reviewValidation,
    target,
    before,
    requested,
  );

  let applied;
  try {
    applied = await compareAndSwapPagesSettings(
      target,
      before.revision,
      before.settings,
      requested,
      "compareAndSwapModuleSettings promotion",
    );
  } catch (error) {
    if (error instanceof SnapshotConflictError) {
      writeFailureReceipt(output, base, contract.output.snapshot_conflict_status, {
        mutation: {
          outcome: "snapshot_conflict",
          response: error.response,
          conflict_code: CONFLICT_CODE,
          requires_rereview: true,
          control_plane_execution_confirmed: false,
          tenant_rollout_mutation_confirmed: false,
        },
        rollback: { attempted: false, reason: "no_confirmed_mutation" },
      });
      fail("module settings snapshot conflict requires a fresh read and fresh promotion review");
    }
    if (error instanceof MutationOutcomeAmbiguousError) {
      writeFailureReceipt(output, base, contract.output.manual_reconciliation_status, {
        mutation: {
          outcome: "ambiguous",
          response: error.response,
          control_plane_execution_confirmed: false,
          tenant_rollout_mutation_confirmed: false,
        },
        rollback: {
          attempted: false,
          reason: "ambiguous_mutation_outcome_must_not_auto_rollback",
        },
        manual_reconciliation_required: true,
      });
      fail("promotion mutation outcome is ambiguous; manual reconciliation is required");
    }
    throw error;
  }

  const appliedHash = sha256(canonicalJson(applied.settings));
  let postcondition;
  let postconditionError = null;
  try {
    if (!hasAllOnFlags(applied.settings)) {
      fail("confirmed CAS result does not contain the all_on profile");
    }
    postcondition = await verifyPromotedPostcondition(target, applied.settings);
  } catch (error) {
    postconditionError = error;
  }

  if (postconditionError !== null) {
    let rollback;
    let restored;
    try {
      rollback = await compareAndSwapPagesSettings(
        target,
        applied.revision,
        applied.settings,
        before.settings,
        "compareAndSwapModuleSettings promotion rollback",
      );
      if (canonicalJson(rollback.settings) !== canonicalJson(before.settings)) {
        throw new MutationOutcomeAmbiguousError(
          "rollback returned settings different from the original snapshot",
          rollback.response,
        );
      }
      restored = await loadPagesModule(target);
      if (canonicalJson(restored.settings) !== canonicalJson(before.settings)) {
        throw new MutationOutcomeAmbiguousError(
          "rollback verification does not match the original settings snapshot",
          restored.response,
        );
      }
    } catch (rollbackError) {
      const response =
        rollbackError instanceof SnapshotConflictError ||
        rollbackError instanceof MutationOutcomeAmbiguousError
          ? rollbackError.response
          : null;
      writeFailureReceipt(output, base, contract.output.manual_reconciliation_status, {
        mutation: mutationReceipt(applied, appliedHash),
        postcondition: {
          passed: false,
          failure: postconditionError.message,
          raw_failure_payload_persisted: false,
        },
        rollback: {
          attempted: true,
          outcome:
            rollbackError instanceof SnapshotConflictError
              ? "snapshot_conflict"
              : "ambiguous_or_failed",
          response,
          net_target_state_retained: "unknown",
        },
        manual_reconciliation_required: true,
      });
      fail(`promotion postcondition failed and CAS rollback was not confirmed: ${rollbackError.message}`);
    }

    writeFailureReceipt(output, base, contract.output.rolled_back_status, {
      mutation: mutationReceipt(applied, appliedHash),
      postcondition: {
        passed: false,
        failure: postconditionError.message,
        raw_failure_payload_persisted: false,
      },
      rollback: {
        attempted: true,
        outcome: "confirmed_restored",
        response: rollback.response,
        restore_read: restored.response,
        restored_settings_semantic_sha256: sha256(canonicalJson(restored.settings)),
        net_target_state_retained: false,
      },
    });
    fail("promotion postcondition failed; original Pages settings were restored by CAS rollback");
  }

  writeAtomic(output, {
    ...base,
    status: contract.output.success_status,
    mutation: mutationReceipt(applied, appliedHash),
    postcondition: {
      passed: true,
      tenant_modules: postcondition.tenant_modules,
      rollout_snapshot: postcondition.rollout_snapshot,
      provider_health_observed: postcondition.provider_health_observed,
      current_provider_health_asserted: false,
    },
    rollback: {
      attempted: false,
      outcome: "not_required",
      net_target_state_retained: true,
    },
    readiness: readinessBoundary(),
    boundaries: {
      control_plane_change_executed: true,
      tenant_rollout_mutated: true,
      canonical_source_mutated: false,
      readiness_board_mutated: false,
      current_provider_health_asserted: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
    },
  });
}

try {
  await main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
