import { test, type BrowserContext, type Page } from "@playwright/test";
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

const repoRoot = fileURLToPath(new URL("../../../../", import.meta.url));
const contractPath =
  "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json";
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), "utf8"),
) as RuntimeContract;

const graphqlPath = "/api/graphql";
const capabilityPath = "/api/fn/pages/page-builder-capability";
const providerSelector = "[data-fly-provider-control-state]";
const previewPanelSelector = '[data-page-builder-server-preview="true"]';
const propertiesFieldsetSelector = 'fieldset[data-fly-capability="properties"]';
const publishFieldsetSelector = 'fieldset[data-fly-capability="publish"]';
const mismatchPageId = "__provider_health_runtime_probe_mismatch__";

const runtimeQuery = `query ProviderHealthRuntimeEvidence {
  pageBuilderRolloutSnapshot {
    tenantSlug builderEnabled previewEnabled propertiesEnabled publishEnabled
    providerHealthObserved
    providerHealth {
      state degradationReasons previewP95Ms publishP95Ms sanitizeFailureRate runtimeErrorRate
    }
  }
  preview: pageBuilderCapabilityPreflight(capability: PREVIEW) {
    capability allowed errorKind errorCode
  }
  properties: pageBuilderCapabilityPreflight(capability: PROPERTIES) {
    capability allowed errorKind errorCode
  }
  publish: pageBuilderCapabilityPreflight(capability: PUBLISH) {
    capability allowed errorKind errorCode
  }
}`;

type PredecessorSpec = {
  environment: string;
  format: string;
  status: string;
  decision?: string;
  rollback_action?: string;
};

type RuntimeContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  predecessors: Record<string, PredecessorSpec>;
  fixtures: Record<string, unknown> & {
    api_origin_environment: string;
    admin_origin_environment: string;
    api_storage_state_environment: string;
    admin_storage_state_environment: string;
    tenant_slug_environment: string;
    page_id_environment: string;
    admin_route_environment: string;
    common_headers_environment: string;
  };
  output: {
    environment: string;
    default_path: string;
    format: string;
    status: string;
  };
  required_source_files?: string[];
};

type FileRecord = { path: string; bytes: number; sha256: string };
type JsonInput = { record: FileRecord; document: Record<string, unknown> };
type GraphqlResult = {
  status: number;
  responseBytes: number;
  responseSha256: string;
  data: Record<string, unknown>;
};
type HealthState = "ready" | "degraded" | "unavailable";
type CapabilityExpectation = "allowed" | "feature_disabled";
type AcceptedHealth = {
  state: HealthState;
  degradation_reasons: string[];
  thresholds: Record<string, number>;
  observed: {
    preview_p95_ms: number;
    publish_p95_ms: number;
    sanitize_failure_rate: number;
    runtime_error_rate: number;
  };
};

function fail(message: string): never {
  throw new Error(`Pages provider-health runtime evidence failed: ${message}`);
}

function sha256(value: Buffer | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function objectValue(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
}

function requiredEnvironment(name: string, maximumLength = 16_384): string {
  const value = process.env[name];
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${name} must contain a bounded non-empty value`);
  }
  return value;
}

function optionalEnvironment(name: string, maximumLength = 16_384): string | null {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  if (value.length > maximumLength || /[\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded input`);
  }
  return value;
}

function requireOrigin(value: string, label: string): string {
  let parsed: URL;
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
    fail(`${label} must be credential-free with no path/query/fragment`);
  }
  return parsed.origin;
}

function requireRelativePath(value: string, label: string): string {
  if (
    !value.startsWith("/") ||
    value.startsWith("//") ||
    value.length > 4096 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} must be a bounded same-origin absolute path`);
  }
  const parsed = new URL(value, "https://evidence.invalid");
  if (parsed.origin !== "https://evidence.invalid") {
    fail(`${label} must remain same-origin`);
  }
  return `${parsed.pathname}${parsed.search}`;
}

function requireTenantSlug(value: string): string {
  if (
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value) > 128 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail("tenant slug is invalid");
  }
  return value;
}

function requirePageId(value: string): string {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(
      value,
    )
  ) {
    fail("runtime evidence page id must be a UUID");
  }
  const normalized = value.toLowerCase();
  if (normalized === mismatchPageId) {
    fail("runtime evidence page id collides with non-mutating mismatch sentinel");
  }
  return normalized;
}

function resolveInput(value: string): string {
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function regularFileRecord(
  value: string,
  label: string,
  maximumBytes = 8 * 1024 * 1024,
): FileRecord {
  const absolute = resolveInput(value);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const size = statSync(absolute).size;
  if (size <= 0 || size > maximumBytes) fail(`${label} is outside the bounded size`);
  const bytes = readFileSync(absolute);
  return { path: absolute, bytes: size, sha256: sha256(bytes) };
}

function readJsonInput(value: string, label: string): JsonInput {
  const record = regularFileRecord(value, label);
  try {
    return {
      record,
      document: objectValue(JSON.parse(readFileSync(record.path, "utf8")), label),
    };
  } catch (error) {
    fail(`${label} is invalid JSON: ${(error as Error).message}`);
  }
}

function currentCommit(): string {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) {
    fail("git HEAD is not a canonical lowercase SHA");
  }
  return value;
}

function canonicalIso(value: unknown, label: string): number {
  if (typeof value !== "string" || value.length === 0 || value.length > 128) {
    fail(`${label} is invalid`);
  }
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds) || new Date(milliseconds).toISOString() !== value) {
    fail(`${label} must be canonical ISO-8601 UTC`);
  }
  return milliseconds;
}

function canonicalRepoDigest(value: unknown, label: string): string {
  if (typeof value !== "string" || !/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(value)) {
    fail(`${label} must be an immutable RepoDigest`);
  }
  return value;
}

function canonicalJson(value: unknown): string {
  const normalize = (input: unknown): unknown => {
    if (Array.isArray(input)) return input.map(normalize);
    if (input !== null && typeof input === "object") {
      return Object.fromEntries(
        Object.entries(input as Record<string, unknown>)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, nested]) => [key, normalize(nested)]),
      );
    }
    return input;
  };
  return JSON.stringify(normalize(value));
}

function acceptedHealth(value: unknown): AcceptedHealth {
  const snapshot = objectValue(value, "accepted provider health snapshot");
  if (!["ready", "degraded", "unavailable"].includes(String(snapshot.state))) {
    fail("accepted provider health state is invalid");
  }
  if (
    !Array.isArray(snapshot.degradation_reasons) ||
    !snapshot.degradation_reasons.every((reason) => typeof reason === "string")
  ) {
    fail("accepted degradation reasons are invalid");
  }
  const observed = objectValue(snapshot.observed, "accepted provider observations");
  const thresholds = objectValue(snapshot.thresholds, "accepted provider thresholds");
  for (const [label, candidate] of [
    ...Object.entries(observed),
    ...Object.entries(thresholds),
  ]) {
    if (typeof candidate !== "number" || !Number.isFinite(candidate) || candidate < 0) {
      fail(`${label} is invalid`);
    }
  }
  return snapshot as unknown as AcceptedHealth;
}

function validateEvidenceChain(
  identity: JsonInput,
  evaluation: JsonInput,
  acceptance: JsonInput,
  head: string,
): {
  sourceCommit: string;
  deploymentId: string;
  deploymentDigest: string;
  healthValidUntil: string;
  health: AcceptedHealth;
  sloEvaluation: Record<string, unknown>;
} {
  const identitySpec = contract.predecessors.deployment_identity;
  const evaluationSpec = contract.predecessors.deployment_evaluation;
  const acceptanceSpec = contract.predecessors.owner_acceptance;
  if (
    identity.document.format !== identitySpec.format ||
    identity.document.status !== identitySpec.status
  ) {
    fail("identity evidence identity/status drifted");
  }
  if (
    evaluation.document.format !== evaluationSpec.format ||
    evaluation.document.status !== evaluationSpec.status
  ) {
    fail("evaluation evidence identity/status drifted");
  }
  if (
    acceptance.document.format !== acceptanceSpec.format ||
    acceptance.document.status !== acceptanceSpec.status
  ) {
    fail("owner acceptance identity/status drifted");
  }
  if (
    acceptanceSpec.decision !== "accept_for_pages_binding" ||
    acceptanceSpec.rollback_action !== "restore_unobserved_provider_health"
  ) {
    fail("runtime contract owner-acceptance policy drifted");
  }

  const identityDeployment = objectValue(identity.document.deployment, "identity deployment");
  const evaluationDeployment = objectValue(
    evaluation.document.deployment,
    "evaluation deployment",
  );
  const acceptanceDeployment = objectValue(
    acceptance.document.deployment,
    "acceptance deployment",
  );
  const sourceCommit = String(identityDeployment.source_commit ?? "");
  if (!/^[0-9a-f]{40}$/u.test(sourceCommit) || sourceCommit !== head) {
    fail("identity source commit does not equal checkout HEAD");
  }
  if (
    evaluationDeployment.source_commit !== sourceCommit ||
    acceptanceDeployment.source_commit !== sourceCommit
  ) {
    fail("source commit differs across evidence chain");
  }
  const deploymentId = String(identityDeployment.deployment_id ?? "");
  if (!/^[A-Za-z0-9._:/-]{1,128}$/u.test(deploymentId)) {
    fail("deployment id is invalid");
  }
  if (
    evaluationDeployment.deployment_id !== deploymentId ||
    acceptanceDeployment.deployment_id !== deploymentId
  ) {
    fail("deployment id differs across evidence chain");
  }
  const deploymentDigest = canonicalRepoDigest(
    identityDeployment.deployment_image_digest,
    "identity deployment image digest",
  );
  if (
    evaluationDeployment.deployment_image_digest !== deploymentDigest ||
    acceptanceDeployment.deployment_image_digest !== deploymentDigest
  ) {
    fail("deployment RepoDigest differs across evidence chain");
  }

  const capturedAt = identity.document.captured_at;
  canonicalIso(capturedAt, "identity captured_at");
  if (evaluationDeployment.identity_captured_at !== capturedAt) {
    fail("evaluation identity timestamp is not bound to supplied identity packet");
  }

  const acceptanceDecision = objectValue(
    acceptance.document.decision,
    "owner acceptance decision",
  );
  if (
    acceptanceDecision.value !== acceptanceSpec.decision ||
    acceptanceDecision.rollback_action !== acceptanceSpec.rollback_action ||
    acceptanceDecision.owner_identity_is_operator_assertion !== true ||
    acceptanceDecision.cryptographic_signature_present !== false ||
    acceptanceDecision.free_text_reason_retained !== false
  ) {
    fail("owner acceptance decision contract drifted");
  }

  const acceptanceEvaluation = objectValue(
    acceptance.document.evaluation,
    "acceptance evaluation",
  );
  if (
    acceptanceEvaluation.format !== evaluation.document.format ||
    acceptanceEvaluation.status !== evaluation.document.status ||
    acceptanceEvaluation.evaluated_at !== evaluation.document.evaluated_at ||
    acceptanceEvaluation.evaluation_sha256 !== evaluation.record.sha256 ||
    acceptanceEvaluation.raw_evaluation_path_persisted !== false ||
    acceptanceEvaluation.source_hashes_verified_against_checkout !== true
  ) {
    fail("owner acceptance is not bound to the supplied evaluation packet");
  }

  const evaluationSnapshot = acceptedHealth(evaluation.document.snapshot);
  const acceptanceSnapshot = acceptedHealth(acceptanceEvaluation.snapshot);
  if (canonicalJson(evaluationSnapshot) !== canonicalJson(acceptanceSnapshot)) {
    fail("accepted health snapshot differs from evaluator snapshot");
  }
  const evaluationSlo = objectValue(
    evaluation.document.slo_evaluation,
    "evaluation SLO evaluation",
  );
  const acceptanceSlo = objectValue(
    acceptanceEvaluation.slo_evaluation,
    "accepted SLO evaluation",
  );
  if (canonicalJson(evaluationSlo) !== canonicalJson(acceptanceSlo)) {
    fail("accepted SLO evaluation differs from evaluator SLO evaluation");
  }

  const healthValidUntil = String(acceptanceEvaluation.health_valid_until ?? "");
  const validUntilMs = canonicalIso(healthValidUntil, "accepted health_valid_until");
  if (Date.now() > validUntilMs + 5_000) {
    fail("accepted provider health is expired before runtime observation begins");
  }

  const binding = objectValue(acceptance.document.binding, "acceptance binding");
  if (
    binding.server_binding_authorized !== true ||
    binding.server_binding_performed !== false ||
    binding.required_live_source_commit !== sourceCommit ||
    binding.required_deployment_image_digest !== deploymentDigest ||
    binding.failure_action !== acceptanceSpec.rollback_action
  ) {
    fail("owner acceptance binding contract drifted");
  }

  return {
    sourceCommit,
    deploymentId,
    deploymentDigest,
    healthValidUntil,
    health: acceptanceSnapshot,
    sloEvaluation: acceptanceSlo,
  };
}

function commonHeaders(tenantSlug: string): Record<string, string> {
  const name = contract.fixtures.common_headers_environment;
  const raw = optionalEnvironment(name);
  const headers: Record<string, string> = { "x-tenant-slug": tenantSlug };
  if (raw === null) return headers;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    fail(`${name} is invalid JSON: ${(error as Error).message}`);
  }
  const values = objectValue(parsed, name);
  for (const [headerName, value] of Object.entries(values)) {
    const normalized = headerName.toLowerCase();
    if (
      !/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized) ||
      ["authorization", "cookie", "set-cookie", "host", "content-length"].includes(
        normalized,
      )
    ) {
      fail(`${name} contains forbidden header ${headerName}`);
    }
    if (
      typeof value !== "string" ||
      value.length > 4096 ||
      /[\u0000\r\n]/u.test(value)
    ) {
      fail(`${name} contains invalid header ${headerName}`);
    }
    headers[normalized] = value;
  }
  headers["x-tenant-slug"] = tenantSlug;
  return headers;
}

async function graphql(
  context: BrowserContext,
  query: string,
  label: string,
): Promise<GraphqlResult> {
  const response = await context.request.post(graphqlPath, {
    data: { query },
    failOnStatusCode: false,
  });
  const body = await response.body();
  if (response.status() !== 200) fail(`${label} did not return HTTP 200`);
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString("utf8"));
  } catch {
    fail(`${label} did not return JSON`);
  }
  const envelope = objectValue(parsed, `${label} response`);
  if (Array.isArray(envelope.errors) && envelope.errors.length > 0) {
    fail(`${label} returned GraphQL errors`);
  }
  return {
    status: response.status(),
    responseBytes: body.length,
    responseSha256: sha256(body),
    data: objectValue(envelope.data, `${label} data`),
  };
}

function validateCapability(
  value: unknown,
  capability: "preview" | "properties" | "publish",
  expected: CapabilityExpectation,
): Record<string, unknown> {
  const result = objectValue(value, `${capability} preflight`);
  if (result.capability !== capability.toUpperCase()) {
    fail(`${capability} preflight returned another capability`);
  }
  if (expected === "allowed") {
    if (
      result.allowed !== true ||
      result.errorKind !== null ||
      result.errorCode !== null
    ) {
      fail(`${capability} should be allowed`);
    }
  } else if (
    result.allowed !== false ||
    result.errorKind !== "feature-disabled" ||
    result.errorCode !== "FEATURE_DISABLED"
  ) {
    fail(`${capability} must return feature-disabled / FEATURE_DISABLED`);
  }
  return {
    capability,
    allowed: result.allowed,
    error_kind: result.errorKind,
    error_code: result.errorCode,
  };
}

function expectedForHealth(state: HealthState): {
  preview: CapabilityExpectation;
  properties: CapabilityExpectation;
  publish: CapabilityExpectation;
} {
  if (state === "ready") {
    return { preview: "allowed", properties: "allowed", publish: "allowed" };
  }
  if (state === "degraded") {
    return {
      preview: "allowed",
      properties: "allowed",
      publish: "feature_disabled",
    };
  }
  return {
    preview: "feature_disabled",
    properties: "feature_disabled",
    publish: "feature_disabled",
  };
}

function validateGraphqlObservation(
  result: GraphqlResult,
  tenantSlug: string,
  accepted: AcceptedHealth,
): Record<string, unknown> {
  const snapshot = objectValue(
    result.data.pageBuilderRolloutSnapshot,
    "runtime rollout snapshot",
  );
  if (
    snapshot.tenantSlug !== tenantSlug ||
    snapshot.builderEnabled !== true ||
    snapshot.previewEnabled !== true ||
    snapshot.propertiesEnabled !== true ||
    snapshot.publishEnabled !== true
  ) {
    fail("runtime evidence requires configured all_on rollout flags");
  }
  if (snapshot.providerHealthObserved !== true) {
    fail("runtime snapshot did not observe provider health");
  }
  const health = objectValue(snapshot.providerHealth, "runtime provider health");
  if (
    health.state !== accepted.state ||
    canonicalJson(health.degradationReasons) !==
      canonicalJson(accepted.degradation_reasons) ||
    health.previewP95Ms !== accepted.observed.preview_p95_ms ||
    health.publishP95Ms !== accepted.observed.publish_p95_ms ||
    health.sanitizeFailureRate !== accepted.observed.sanitize_failure_rate ||
    health.runtimeErrorRate !== accepted.observed.runtime_error_rate
  ) {
    fail("runtime GraphQL health differs from accepted provider snapshot");
  }
  const expected = expectedForHealth(accepted.state);
  return {
    status: result.status,
    response_body_bytes: result.responseBytes,
    response_body_sha256: result.responseSha256,
    configured_rollout_all_on: true,
    provider_health_observed: true,
    provider_state: accepted.state,
    preview: validateCapability(result.data.preview, "preview", expected.preview),
    properties: validateCapability(
      result.data.properties,
      "properties",
      expected.properties,
    ),
    publish: validateCapability(result.data.publish, "publish", expected.publish),
    raw_request_or_response_persisted: false,
  };
}

async function settleAdminPage(page: Page, adminRoute: string): Promise<void> {
  const response = await page.goto(adminRoute, { waitUntil: "domcontentloaded" });
  if (response === null || response.status() >= 400) {
    fail("admin runtime evidence route failed");
  }
  await page
    .waitForLoadState("networkidle", { timeout: 15_000 })
    .catch(() => undefined);
  await page
    .locator(providerSelector)
    .first()
    .waitFor({ state: "visible", timeout: 15_000 });
}

async function workspaceObservation(
  page: Page,
  state: HealthState,
): Promise<Record<string, unknown>> {
  const provider = page.locator(providerSelector).first();
  const providerState = await provider.getAttribute("data-fly-provider-control-state");
  const providerHealth = await provider.getAttribute("data-fly-provider-health");
  if (providerState !== state || providerHealth !== state) {
    fail("workspace provider control does not match accepted health");
  }

  const preview = page.locator(previewPanelSelector).first();
  await preview.waitFor({ state: "visible", timeout: 15_000 });
  const previewEnabled =
    (await preview.getAttribute("data-page-builder-provider-preview")) === "true";
  const expected = expectedForHealth(state);
  if (previewEnabled !== (expected.preview === "allowed")) {
    fail("workspace preview capability differs from accepted health");
  }

  const stateOf = async (
    selector: string,
    shouldEnable: boolean,
  ): Promise<"enabled" | "disabled" | "hidden"> => {
    const fieldset = page.locator(selector).first();
    if ((await fieldset.count()) === 0) {
      if (shouldEnable) fail(`${selector} is unexpectedly hidden`);
      return "hidden";
    }
    const disabled = (await fieldset.getAttribute("disabled")) !== null;
    if (disabled === shouldEnable) {
      fail(`${selector} capability differs from accepted health`);
    }
    return disabled ? "disabled" : "enabled";
  };

  return {
    provider_control_state: providerState,
    provider_health: providerHealth,
    preview_enabled: previewEnabled,
    properties: await stateOf(
      propertiesFieldsetSelector,
      expected.properties === "allowed",
    ),
    publish: await stateOf(publishFieldsetSelector, expected.publish === "allowed"),
  };
}

async function safeSsrPreviewObservation(
  page: Page,
  state: HealthState,
): Promise<Record<string, unknown>> {
  const button = page.locator(previewPanelSelector).first().locator("button").first();
  if (state === "unavailable") {
    if (!(await button.isDisabled())) {
      fail("unavailable health must disable preview before SSR dispatch");
    }
    return { request_attempted: false, ui_blocked: true, mutation_possible: false };
  }
  if (await button.isDisabled()) {
    fail("ready/degraded health unexpectedly disabled preview");
  }
  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === "POST" &&
      new URL(request.url()).pathname === capabilityPath,
    { timeout: 15_000 },
  );
  await button.click();
  const request = await requestPromise;
  const response = await request.response();
  if (response === null) fail("SSR preview request produced no response");
  const body = await response.body();
  if (
    response.status() >= 400 ||
    /capability disabled: preview/iu.test(body.toString("utf8"))
  ) {
    fail("SSR preview was rejected under ready/degraded health");
  }
  return {
    request_attempted: true,
    status: response.status(),
    response_body_bytes: body.length,
    response_body_sha256: sha256(body),
    capability_disabled: false,
    mutation_possible: false,
    raw_request_or_response_persisted: false,
  };
}

async function safeBrowserIntentDenial(
  adminContext: BrowserContext,
  pageId: string,
  intent: "save" | "rename_page",
  capability: "publish" | "properties",
): Promise<Record<string, unknown>> {
  const response = await adminContext.request.post(
    `/api/admin/pages/${encodeURIComponent(pageId)}/builder/intents`,
    {
      data: {
        protocol: "fly_iframe",
        instance_id: "pages-provider-health-runtime-evidence",
        intent,
        payload:
          intent === "rename_page"
            ? { page_id: mismatchPageId, new_page_id: "never-applied" }
            : {},
        page_id: mismatchPageId,
        sequence: 1,
      },
      failOnStatusCode: false,
    },
  );
  const body = await response.body();
  if (response.status() !== 403) {
    fail(
      `${intent} did not return health-limited FLY_CAPABILITY_DENIED; mismatched page id prevents mutation if health was revoked`,
    );
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString("utf8"));
  } catch {
    fail(`${intent} denial did not return JSON`);
  }
  const problem = objectValue(parsed, `${intent} browser-intent denial`);
  if (
    problem.status !== 403 ||
    problem.code !== "FLY_CAPABILITY_DENIED" ||
    problem.intent !== intent ||
    problem.capability !== capability ||
    !Array.isArray(problem.missing) ||
    !problem.missing.includes(capability)
  ) {
    fail(`${intent} browser-intent denial contract drifted`);
  }
  return {
    status: response.status(),
    response_body_bytes: body.length,
    response_body_sha256: sha256(body),
    code: problem.code,
    capability: problem.capability,
    intent: problem.intent,
    mismatch_page_id_used_as_non_mutating_fallback: true,
    raw_request_or_response_persisted: false,
  };
}

function outputPath(): string {
  const raw = optionalEnvironment(contract.output.environment);
  const absolute = resolveInput(raw ?? contract.output.default_path);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("runtime evidence output must remain under target/");
  }
  return absolute;
}

function writeAtomic(location: string, document: Record<string, unknown>): void {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, location);
}

function sourceHashes(): Record<string, string> {
  const required = contract.required_source_files ?? [];
  if (required.length === 0) {
    fail("runtime evidence contract has no required source files");
  }
  return Object.fromEntries(
    required.map((relativePath) => {
      const record = regularFileRecord(relativePath, `source file ${relativePath}`);
      return [relativePath, record.sha256];
    }),
  );
}

test("Pages observed provider health is bound across runtime consumers", async ({ browser }) => {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "pages" ||
    contract.packet !== "pages-builder-provider-health-runtime-evidence" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("runtime evidence contract identity drifted");
  }

  const head = currentCommit();
  const identity = readJsonInput(
    requiredEnvironment(contract.predecessors.deployment_identity.environment),
    "deployment identity evidence",
  );
  const evaluation = readJsonInput(
    requiredEnvironment(contract.predecessors.deployment_evaluation.environment),
    "deployment evaluation evidence",
  );
  const acceptance = readJsonInput(
    requiredEnvironment(contract.predecessors.owner_acceptance.environment),
    "owner acceptance evidence",
  );
  const admitted = validateEvidenceChain(identity, evaluation, acceptance, head);

  const apiOrigin = requireOrigin(
    requiredEnvironment(contract.fixtures.api_origin_environment),
    "API origin",
  );
  const adminOrigin = requireOrigin(
    requiredEnvironment(contract.fixtures.admin_origin_environment),
    "admin origin",
  );
  const tenantSlug = requireTenantSlug(
    requiredEnvironment(contract.fixtures.tenant_slug_environment),
  );
  const pageId = requirePageId(
    requiredEnvironment(contract.fixtures.page_id_environment),
  );
  const adminRoute = requireRelativePath(
    requiredEnvironment(contract.fixtures.admin_route_environment),
    "admin route",
  );
  const apiStorage = regularFileRecord(
    requiredEnvironment(contract.fixtures.api_storage_state_environment),
    "API storage state",
  );
  const adminStorage = regularFileRecord(
    requiredEnvironment(contract.fixtures.admin_storage_state_environment),
    "admin storage state",
  );
  const headers = commonHeaders(tenantSlug);

  const apiContext = await browser.newContext({
    baseURL: apiOrigin,
    storageState: apiStorage.path,
    extraHTTPHeaders: headers,
  });
  const adminContext = await browser.newContext({
    baseURL: adminOrigin,
    storageState: adminStorage.path,
    extraHTTPHeaders: headers,
  });
  const page = await adminContext.newPage();
  const output = outputPath();
  rmSync(output, { force: true });

  try {
    const graphqlBefore = await graphql(
      apiContext,
      runtimeQuery,
      "provider-health runtime snapshot before workspace",
    );
    const graphqlObservation = validateGraphqlObservation(
      graphqlBefore,
      tenantSlug,
      admitted.health,
    );

    await settleAdminPage(page, adminRoute);
    const workspace = await workspaceObservation(page, admitted.health.state);
    const ssrPreview = await safeSsrPreviewObservation(page, admitted.health.state);

    const browserIntent: Record<string, unknown>[] = [];
    if (admitted.health.state === "degraded") {
      browserIntent.push(
        await safeBrowserIntentDenial(adminContext, pageId, "save", "publish"),
      );
    } else if (admitted.health.state === "unavailable") {
      browserIntent.push(
        await safeBrowserIntentDenial(adminContext, pageId, "save", "publish"),
      );
      browserIntent.push(
        await safeBrowserIntentDenial(
          adminContext,
          pageId,
          "rename_page",
          "properties",
        ),
      );
    }

    const graphqlAfter = await graphql(
      apiContext,
      runtimeQuery,
      "provider-health runtime snapshot after consumers",
    );
    validateGraphqlObservation(graphqlAfter, tenantSlug, admitted.health);
    if (
      Date.now() >
      canonicalIso(admitted.healthValidUntil, "accepted health_valid_until") + 5_000
    ) {
      fail("provider health expired during runtime evidence collection");
    }

    writeAtomic(output, {
      format: contract.output.format,
      status: contract.output.status,
      generated_at: new Date().toISOString(),
      source_commit: admitted.sourceCommit,
      deployment: {
        deployment_id: admitted.deploymentId,
        deployment_image_digest: admitted.deploymentDigest,
      },
      input_packets: {
        deployment_identity: {
          bytes: identity.record.bytes,
          sha256: identity.record.sha256,
        },
        deployment_evaluation: {
          bytes: evaluation.record.bytes,
          sha256: evaluation.record.sha256,
        },
        owner_acceptance: {
          bytes: acceptance.record.bytes,
          sha256: acceptance.record.sha256,
        },
        raw_paths_persisted: false,
      },
      source_sha256: sourceHashes(),
      target: {
        api_origin_sha256: sha256(apiOrigin),
        admin_origin_sha256: sha256(adminOrigin),
        tenant_slug_sha256: sha256(tenantSlug),
        page_id_sha256: sha256(pageId),
      },
      accepted_health: {
        health_valid_until: admitted.healthValidUntil,
        snapshot: admitted.health,
        slo_evaluation: admitted.sloEvaluation,
      },
      observations: {
        graphql: graphqlObservation,
        workspace,
        authoritative_ssr_preview: ssrPreview,
        standalone_browser_intent: browserIntent,
        graphql_after_consumers: {
          status: graphqlAfter.status,
          response_body_bytes: graphqlAfter.responseBytes,
          response_body_sha256: graphqlAfter.responseSha256,
          provider_health_still_observed: true,
        },
      },
      boundaries: {
        exact_identity_evaluator_acceptance_chain_verified: true,
        accepted_packet_runtime_observed: true,
        configured_rollout_all_on: true,
        rollout_settings_mutated: false,
        publish_mutation_executed: false,
        mismatched_page_id_protects_browser_intent_probe_if_health_revoked: true,
        owner_observed_health_acceptance: false,
        pages_reference_consumer_gate_accepted: false,
        forum_wave_accepted: false,
        ffa_promoted: false,
        fba_promoted: false,
        canonical_source_mutated: false,
      },
      privacy: {
        tenant_slug_or_id_persisted: false,
        page_id_persisted: false,
        authorization_or_cookie_values_persisted: false,
        storage_state_contents_persisted: false,
        tokens_or_session_ids_persisted: false,
        raw_graphql_bodies_persisted: false,
        raw_server_function_bodies_persisted: false,
        raw_evidence_paths_persisted: false,
        screenshots_persisted: false,
        videos_persisted: false,
        traces_persisted: false,
      },
    });
  } finally {
    await page.close().catch(() => undefined);
    await apiContext.close();
    await adminContext.close();
  }
});
