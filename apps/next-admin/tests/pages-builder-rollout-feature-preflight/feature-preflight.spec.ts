import { test, type BrowserContext } from "@playwright/test";
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
  "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json";
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), "utf8"),
) as PreflightContract;

const graphqlPath = "/api/graphql";
const tenantModulesQuery =
  "query RolloutPreflightTenantModules { tenantModules { moduleSlug enabled settings } }";
const updateSettingsMutation =
  "mutation RolloutPreflightUpdateSettings($moduleSlug: String!, $settings: String!) { updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings) { moduleSlug enabled settings } }";
const capabilityPreflightQuery =
  "query RolloutCapabilityPreflight { preview: pageBuilderCapabilityPreflight(capability: PREVIEW) { capability allowed errorKind errorCode } properties: pageBuilderCapabilityPreflight(capability: PROPERTIES) { capability allowed errorKind errorCode } publish: pageBuilderCapabilityPreflight(capability: PUBLISH) { capability allowed errorKind errorCode } }";

type FileRecord = {
  path: string;
  bytes: number;
  sha256: string;
};

type GraphqlResult = {
  status: number;
  responseBytes: number;
  responseSha256: string;
  data: Record<string, unknown>;
};

type ProfileExpectation = "allowed" | "feature_disabled";

type PreflightProfile = {
  id: "all_on" | "publish_off" | "preview_off" | "builder_off";
  flags: [boolean, boolean, boolean, boolean];
  preview: ProfileExpectation;
  properties: ProfileExpectation;
  publish: ProfileExpectation;
};

type PreflightContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  predecessors: {
    browser: { environment: string; format: string; status: string };
    rollout_matrix: { environment: string; format: string; status: string };
  };
  fixtures: {
    api_origin_environment: string;
    api_storage_state_environment: string;
    tenant_slug_environment: string;
    common_headers_environment: string;
  };
  profiles: PreflightProfile[];
  output: {
    environment: string;
    default_path: string;
    format: string;
    status: string;
  };
  required_source_files: string[];
};

function fail(message: string): never {
  throw new Error(`Pages rollout feature preflight failed: ${message}`);
}

function sha256(value: Buffer | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function requiredEnvironment(name: string, maximumLength = 16_384): string {
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

function optionalEnvironment(name: string, maximumLength = 16_384): string | null {
  const value = process.env[name];
  if (value === undefined || value === "") return null;
  if (value.length > maximumLength || /[\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded environment input`);
  }
  return value;
}

function requireOrigin(value: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    fail("feature preflight API origin must be absolute HTTP(S)");
  }
  if (
    !["http:", "https:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !["", "/"].includes(parsed.pathname)
  ) {
    fail("feature preflight API origin must be credential-free with no path/query/fragment");
  }
  return parsed.origin;
}

function requireTenantSlug(value: string): string {
  if (
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, "utf8") > 128 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail("feature preflight tenant slug must be a bounded header-safe value");
  }
  return value;
}

function resolveInput(value: string): string {
  return path.isAbsolute(value) ? path.resolve(value) : path.resolve(repoRoot, value);
}

function regularFileRecord(value: string, label: string): FileRecord {
  const absolute = resolveInput(value);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0 || stats.size > 32 * 1024 * 1024) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { path: absolute, bytes: stats.size, sha256: sha256(bytes) };
}

function readJsonInput(value: string, label: string): {
  record: FileRecord;
  document: Record<string, unknown>;
} {
  const record = regularFileRecord(value, label);
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(record.path, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${(error as Error).message}`);
  }
  return { record, document: objectValue(parsed, label) };
}

function currentCommit(): string {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail("git HEAD is not a full commit SHA");
  return value;
}

function sourceHashes(): Record<string, string> {
  if (!Array.isArray(contract.required_source_files) || contract.required_source_files.length === 0) {
    fail("feature preflight contract has no required source files");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const record = regularFileRecord(relativePath, `source file ${relativePath}`);
      return [relativePath, record.sha256];
    }),
  );
}

function outputPath(): string {
  const raw = optionalEnvironment(contract.output.environment);
  const absolute = resolveInput(raw ?? contract.output.default_path);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("feature preflight output must remain inside repository target/");
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

function playwrightVersion(): string {
  const packagePath = path.join(
    repoRoot,
    "apps/next-admin/node_modules/@playwright/test/package.json",
  );
  const document = JSON.parse(readFileSync(packagePath, "utf8")) as { version?: unknown };
  if (typeof document.version !== "string" || document.version.length === 0) {
    fail("installed Playwright version is unavailable");
  }
  return document.version;
}

function objectValue(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be a JSON object`);
  }
  return value as Record<string, unknown>;
}

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, canonicalize(nested)]),
    );
  }
  return value;
}

function canonicalJson(value: unknown): string {
  return JSON.stringify(canonicalize(value));
}

function parseSettings(raw: unknown, label: string): Record<string, unknown> {
  if (typeof raw !== "string" || Buffer.byteLength(raw, "utf8") > 512 * 1024) {
    fail(`${label} must be a bounded settings JSON string`);
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    fail(`${label} is not valid JSON: ${(error as Error).message}`);
  }
  return objectValue(parsed, label);
}

function commonHeaders(tenantSlug: string): {
  headers: Record<string, string>;
  environmentNames: string[];
} {
  const name = contract.fixtures.common_headers_environment;
  const raw = optionalEnvironment(name);
  const headers: Record<string, string> = {};
  if (raw !== null) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch (error) {
      fail(`${name} must contain a JSON object: ${(error as Error).message}`);
    }
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      fail(`${name} must contain a JSON object`);
    }
    for (const [headerName, headerValue] of Object.entries(parsed)) {
      const normalized = headerName.toLowerCase();
      if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) {
        fail(`${name} contains an invalid header name`);
      }
      if (["authorization", "cookie", "set-cookie"].includes(normalized)) {
        fail(`${name} must not contain credential headers`);
      }
      if (
        typeof headerValue !== "string" ||
        headerValue.length > 4096 ||
        /[\u0000\r\n]/u.test(headerValue)
      ) {
        fail(`${name} contains an invalid header value`);
      }
      headers[normalized] = headerValue;
    }
  }
  headers["x-tenant-slug"] = tenantSlug;
  return { headers, environmentNames: raw === null ? [] : [name] };
}

function validatePredecessors(
  browser: { record: FileRecord; document: Record<string, unknown> },
  matrix: { record: FileRecord; document: Record<string, unknown> },
  head: string,
  apiOrigin: string,
): string {
  if (
    browser.document.format !== contract.predecessors.browser.format ||
    browser.document.status !== contract.predecessors.browser.status ||
    browser.document.source_commit !== head
  ) {
    fail("browser predecessor identity/status/source commit drifted");
  }
  const browserTarget = objectValue(browser.document.target, "browser target");
  if (browserTarget.origin_sha256 !== sha256(apiOrigin)) {
    fail("feature preflight API origin does not match browser predecessor");
  }
  const deploymentDigest = browserTarget.deployment_image_digest;
  if (
    typeof deploymentDigest !== "string" ||
    !/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(deploymentDigest)
  ) {
    fail("browser predecessor has no immutable API deployment RepoDigest");
  }

  if (
    matrix.document.format !== contract.predecessors.rollout_matrix.format ||
    matrix.document.status !== contract.predecessors.rollout_matrix.status ||
    matrix.document.source_commit !== head
  ) {
    fail("rollout matrix predecessor identity/status/source commit drifted");
  }
  const matrixInputs = objectValue(matrix.document.inputs, "matrix inputs");
  const matrixBrowser = objectValue(matrixInputs.browser_predecessor, "matrix browser predecessor");
  if (matrixBrowser.bytes !== browser.record.bytes || matrixBrowser.sha256 !== browser.record.sha256) {
    fail("rollout matrix is not bound to the exact supplied browser packet");
  }
  const matrixTarget = objectValue(matrix.document.target, "matrix target");
  if (
    matrixTarget.api_origin_sha256 !== browserTarget.origin_sha256 ||
    matrixTarget.deployment_image_digest !== deploymentDigest
  ) {
    fail("rollout matrix API origin/digest differs from browser predecessor");
  }
  const originalSettings = objectValue(matrix.document.original_settings, "matrix original settings");
  if (originalSettings.restored !== true || originalSettings.restore_verified !== true) {
    fail("rollout matrix predecessor did not verify settings restoration");
  }
  const boundaries = objectValue(matrix.document.boundaries, "matrix boundaries");
  if (
    boundaries.runtime_matrix_executed !== true ||
    boundaries.gate_accepted !== false ||
    boundaries.provider_health_observed !== false ||
    boundaries.canonical_source_mutated !== false
  ) {
    fail("rollout matrix predecessor boundaries drifted");
  }
  return deploymentDigest;
}

async function graphql(
  context: BrowserContext,
  query: string,
  variables: Record<string, unknown>,
  label: string,
): Promise<GraphqlResult> {
  const response = await context.request.post(graphqlPath, {
    data: { query, variables },
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

async function loadPagesModule(
  context: BrowserContext,
): Promise<{ settings: Record<string, unknown>; read: GraphqlResult }> {
  const read = await graphql(context, tenantModulesQuery, {}, "tenantModules preflight snapshot");
  const modules = read.data.tenantModules;
  if (!Array.isArray(modules)) fail("tenantModules did not return an array");
  const pages = modules.find(
    (entry) =>
      entry !== null &&
      typeof entry === "object" &&
      (entry as Record<string, unknown>).moduleSlug === "pages",
  ) as Record<string, unknown> | undefined;
  if (pages === undefined || pages.enabled !== true) {
    fail("Pages module must be enabled for feature preflight execution");
  }
  return {
    settings: parseSettings(pages.settings, "Pages module settings"),
    read,
  };
}

async function writePagesSettings(
  context: BrowserContext,
  settings: Record<string, unknown>,
): Promise<GraphqlResult> {
  const result = await graphql(
    context,
    updateSettingsMutation,
    { moduleSlug: "pages", settings: JSON.stringify(settings) },
    "updateModuleSettings pages preflight",
  );
  const module = objectValue(result.data.updateModuleSettings, "updated Pages module");
  if (module.moduleSlug !== "pages" || module.enabled !== true) {
    fail("updateModuleSettings did not return enabled Pages");
  }
  const returned = parseSettings(module.settings, "updated Pages preflight settings");
  if (canonicalJson(returned) !== canonicalJson(settings)) {
    fail("updateModuleSettings returned settings different from requested semantic object");
  }
  return result;
}

function withProfile(
  original: Record<string, unknown>,
  flags: [boolean, boolean, boolean, boolean],
): Record<string, unknown> {
  const cloned = JSON.parse(JSON.stringify(original)) as Record<string, unknown>;
  const builder =
    cloned.builder !== null && typeof cloned.builder === "object" && !Array.isArray(cloned.builder)
      ? { ...(cloned.builder as Record<string, unknown>) }
      : {};
  const nested = (value: unknown): Record<string, unknown> =>
    value !== null && typeof value === "object" && !Array.isArray(value)
      ? { ...(value as Record<string, unknown>) }
      : {};
  builder.enabled = flags[0];
  builder.preview = { ...nested(builder.preview), enabled: flags[1] };
  builder.properties = { ...nested(builder.properties), enabled: flags[2] };
  builder.publish = { ...nested(builder.publish), enabled: flags[3] };
  cloned.builder = builder;
  return cloned;
}

function validateCapability(
  value: unknown,
  capability: "PREVIEW" | "PROPERTIES" | "PUBLISH",
  expected: ProfileExpectation,
): Record<string, unknown> {
  const result = objectValue(value, `${capability} feature preflight`);
  if (result.capability !== capability) {
    fail(`${capability} preflight returned a different capability`);
  }
  if (expected === "allowed") {
    if (result.allowed !== true || result.errorKind !== null || result.errorCode !== null) {
      fail(`${capability} preflight should be allowed without an error contract`);
    }
  } else if (
    result.allowed !== false ||
    result.errorKind !== "feature-disabled" ||
    result.errorCode !== "FEATURE_DISABLED"
  ) {
    fail(`${capability} preflight did not return feature-disabled / FEATURE_DISABLED`);
  }
  return {
    capability,
    allowed: result.allowed,
    error_kind: result.errorKind,
    error_code: result.errorCode,
  };
}

async function runCapabilityPreflight(
  context: BrowserContext,
  profile: PreflightProfile,
): Promise<Record<string, unknown>> {
  const result = await graphql(context, capabilityPreflightQuery, {}, "Page Builder feature preflight");
  return {
    status: result.status,
    response_body_bytes: result.responseBytes,
    response_body_sha256: result.responseSha256,
    raw_request_or_response_persisted: false,
    preview: validateCapability(result.data.preview, "PREVIEW", profile.preview),
    properties: validateCapability(result.data.properties, "PROPERTIES", profile.properties),
    publish: validateCapability(result.data.publish, "PUBLISH", profile.publish),
  };
}

function responseRecord(response: GraphqlResult): Record<string, unknown> {
  return {
    status: response.status,
    response_body_bytes: response.responseBytes,
    response_body_sha256: response.responseSha256,
    raw_request_or_response_persisted: false,
  };
}

function validateProfiles(): PreflightProfile[] {
  const expected = [
    ["all_on", [true, true, true, true], "allowed", "allowed", "allowed"],
    ["publish_off", [true, true, true, false], "allowed", "allowed", "feature_disabled"],
    ["preview_off", [true, false, true, false], "feature_disabled", "allowed", "feature_disabled"],
    ["builder_off", [false, false, false, false], "feature_disabled", "feature_disabled", "feature_disabled"],
  ];
  const actual = contract.profiles.map((profile) => [
    profile.id,
    profile.flags,
    profile.preview,
    profile.properties,
    profile.publish,
  ]);
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail("feature preflight profile contract drifted");
  }
  return contract.profiles;
}

test("Pages canonical Page Builder feature preflight matches all rollout profiles", async ({ browser }) => {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "pages" ||
    contract.packet !== "pages-builder-rollout-feature-preflight" ||
    contract.status !== "source_ready_maintainer_execution_pending"
  ) {
    fail("feature preflight contract identity drifted");
  }

  const head = currentCommit();
  const profiles = validateProfiles();
  const apiOrigin = requireOrigin(requiredEnvironment(contract.fixtures.api_origin_environment));
  const tenantSlug = requireTenantSlug(
    requiredEnvironment(contract.fixtures.tenant_slug_environment),
  );
  const storage = regularFileRecord(
    requiredEnvironment(contract.fixtures.api_storage_state_environment),
    "feature preflight API storage state",
  );
  const browserPredecessor = readJsonInput(
    requiredEnvironment(contract.predecessors.browser.environment),
    "feature preflight browser predecessor",
  );
  const matrixPredecessor = readJsonInput(
    requiredEnvironment(contract.predecessors.rollout_matrix.environment),
    "feature preflight rollout matrix predecessor",
  );
  const deploymentDigest = validatePredecessors(
    browserPredecessor,
    matrixPredecessor,
    head,
    apiOrigin,
  );
  const shared = commonHeaders(tenantSlug);
  const output = outputPath();
  rmSync(output, { force: true });

  const context = await browser.newContext({
    baseURL: apiOrigin,
    storageState: storage.path,
    extraHTTPHeaders: shared.headers,
  });

  const profileObservations: Record<string, unknown> = {};
  let originalSettings: Record<string, unknown> | null = null;
  let originalSettingsHash = "";
  let restoreVerified = false;

  try {
    const original = await loadPagesModule(context);
    originalSettings = original.settings;
    originalSettingsHash = sha256(canonicalJson(original.settings));

    for (const profile of profiles) {
      const settings = withProfile(original.settings, profile.flags);
      const settingsWrite = await writePagesSettings(context, settings);
      const capabilityPreflight = await runCapabilityPreflight(context, profile);
      profileObservations[profile.id] = {
        flags: {
          builder_enabled: profile.flags[0],
          preview_enabled: profile.flags[1],
          properties_enabled: profile.flags[2],
          publish_enabled: profile.flags[3],
        },
        settings_write: responseRecord(settingsWrite),
        capability_preflight: capabilityPreflight,
      };
    }
  } finally {
    try {
      if (originalSettings !== null) {
        await writePagesSettings(context, originalSettings);
        const restored = await loadPagesModule(context);
        if (canonicalJson(restored.settings) !== canonicalJson(originalSettings)) {
          fail("Pages settings were not semantically restored after feature preflight");
        }
        if (sha256(canonicalJson(restored.settings)) !== originalSettingsHash) {
          fail("restored Pages settings hash differs from feature-preflight snapshot");
        }
        restoreVerified = true;
      }
    } finally {
      await context.close();
    }
  }

  if (!restoreVerified || originalSettings === null) {
    fail("feature preflight completed without verified Pages settings restoration");
  }

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    source_commit: head,
    generated_at: new Date().toISOString(),
    toolchain: {
      node: process.version,
      playwright: playwrightVersion(),
      browser_project: "pages-builder-rollout-feature-preflight-chromium",
    },
    source_sha256: sourceHashes(),
    inputs: {
      browser_predecessor: {
        bytes: browserPredecessor.record.bytes,
        sha256: browserPredecessor.record.sha256,
      },
      rollout_matrix_predecessor: {
        bytes: matrixPredecessor.record.bytes,
        sha256: matrixPredecessor.record.sha256,
      },
      api_storage_state: {
        bytes: storage.bytes,
        sha256: storage.sha256,
      },
      common_header_environment_names: shared.environmentNames,
    },
    target: {
      api_origin_sha256: sha256(apiOrigin),
      deployment_image_digest: deploymentDigest,
    },
    identity_sha256: {
      tenant_slug: sha256(tenantSlug),
    },
    original_settings: {
      semantic_sha256: originalSettingsHash,
      raw_settings_persisted: false,
      restored: true,
      restore_verified: true,
    },
    profiles: profileObservations,
    boundaries: {
      feature_preflight_executed: true,
      rollout_matrix_remains_owner_review_pending: true,
      candidate_pending: true,
      gate_accepted: false,
      forum_wave_accepted: false,
      provider_health_observed: false,
      ffa_promoted: false,
      fba_promoted: false,
      canonical_source_mutated: false,
    },
    privacy: {
      tenant_slug_or_id_persisted: false,
      authorization_or_cookie_values_persisted: false,
      storage_state_contents_persisted: false,
      tokens_or_session_ids_persisted: false,
      raw_module_settings_persisted: false,
      raw_graphql_bodies_persisted: false,
      database_urls_persisted: false,
      traces_persisted: false,
      screenshots_persisted: false,
      videos_persisted: false,
    },
  });
});
