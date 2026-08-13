import {
  test,
  type BrowserContext,
  type Page,
  type Request
} from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../../../', import.meta.url));
const contractPath =
  'crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json';
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), 'utf8')
) as MatrixContract;

const graphqlPath = '/api/graphql';
const capabilityPath = '/api/fn/pages/page-builder-capability';
const providerSelector = '[data-fly-provider-control-state]';
const previewPanelSelector = '[data-page-builder-server-preview="true"]';
const propertiesFieldsetSelector = 'fieldset[data-fly-capability="properties"]';
const publishFieldsetSelector = 'fieldset[data-fly-capability="publish"]';

const tenantModulesQuery =
  'query RolloutMatrixTenantModules($limit: Int) { tenantModules(limit: $limit) { moduleSlug enabled settings } }';
const updateSettingsMutation =
  'mutation RolloutMatrixUpdateSettings($moduleSlug: String!, $settings: String!) { updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings) { moduleSlug enabled settings } }';
const rolloutSnapshotQuery =
  'query RolloutMatrixSnapshot { pageBuilderRolloutSnapshot { tenantSlug builderEnabled previewEnabled propertiesEnabled publishEnabled providerHealthObserved } }';
const pagesReadsQuery =
  'query RolloutMatrixPagesReads($id: UUID!) { pages { total items { id } } page(id: $id) { id status } }';

type MatrixFlags = {
  builder_enabled: boolean;
  preview_enabled: boolean;
  properties_enabled: boolean;
  publish_enabled: boolean;
};

type MatrixProfile = {
  id: 'all_on' | 'publish_off' | 'preview_off' | 'builder_off';
  flags: MatrixFlags;
  provider_state: string;
  preview_ui: 'enabled' | 'disabled';
  preview_ssr: 'pass' | 'typed_capability_disabled';
  properties_ui: string;
  publish_dry: string;
};

type MatrixContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  predecessor: {
    environment: string;
    format: string;
    status: string;
  };
  fixtures: {
    api_origin_environment: string;
    admin_origin_environment: string;
    api_storage_state_environment: string;
    admin_storage_state_environment: string;
    tenant_slug_environment: string;
    page_id_environment: string;
    admin_route_environment: string;
    common_headers_environment: string;
  };
  profiles: MatrixProfile[];
  output: {
    environment: string;
    default_path: string;
    format: string;
    status: string;
  };
  required_source_files: string[];
};

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

type PreviewTemplate = {
  url: string;
  method: string;
  body: Buffer;
  headers: Record<string, string>;
};

type PreviewObservation = {
  status: number;
  body_bytes: number;
  body_sha256: string;
  capability_disabled: boolean;
};

function fail(message: string): never {
  throw new Error(`Pages rollout runtime matrix failed: ${message}`);
}

function sha256(value: Buffer | string): string {
  return createHash('sha256').update(value).digest('hex');
}

function requiredEnvironment(name: string, maximumLength = 16_384): string {
  const value = process.env[name];
  if (
    typeof value !== 'string' ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${name} must be a bounded non-empty environment value`);
  }
  return value;
}

function optionalEnvironment(
  name: string,
  maximumLength = 16_384
): string | null {
  const value = process.env[name];
  if (value === undefined || value === '') return null;
  if (value.length > maximumLength || /[\u0000]/u.test(value)) {
    fail(`${name} is outside the bounded environment input`);
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
    !['http:', 'https:'].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    !['', '/'].includes(parsed.pathname)
  ) {
    fail(`${label} must be credential-free and contain no path/query/fragment`);
  }
  return parsed.origin;
}

function requireRelativePath(value: string, label: string): string {
  if (
    !value.startsWith('/') ||
    value.startsWith('//') ||
    value.length > 4096 ||
    /[\u0000\r\n]/u.test(value)
  ) {
    fail(`${label} must be a bounded same-origin absolute path`);
  }
  const parsed = new URL(value, 'https://evidence.invalid');
  if (
    parsed.origin !== 'https://evidence.invalid' ||
    parsed.username ||
    parsed.password
  ) {
    fail(`${label} must remain same-origin and credential-free`);
  }
  return `${parsed.pathname}${parsed.search}`;
}

function requireUuid(value: string): string {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(
      value
    )
  ) {
    fail('matrix page id must be a UUID');
  }
  return value.toLowerCase();
}

function requireTenantSlug(value: string): string {
  if (
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, 'utf8') > 128 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail('matrix tenant slug must be a bounded header-safe value');
  }
  return value;
}

function resolveInput(value: string): string {
  return path.isAbsolute(value)
    ? path.resolve(value)
    : path.resolve(repoRoot, value);
}

function regularFileRecord(value: string, label: string): FileRecord {
  const absolute = resolveInput(value);
  if (!existsSync(absolute)) fail(`${label} is missing`);
  const link = lstatSync(absolute);
  if (link.isSymbolicLink() || !link.isFile()) {
    fail(`${label} must be a regular non-symlink file`);
  }
  const stats = statSync(absolute);
  if (stats.size <= 0) fail(`${label} must be non-empty`);
  const bytes = readFileSync(absolute);
  return { path: absolute, bytes: stats.size, sha256: sha256(bytes) };
}

function readJsonInput(
  value: string,
  label: string
): {
  record: FileRecord;
  document: Record<string, unknown>;
} {
  const record = regularFileRecord(value, label);
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(record.path, 'utf8'));
  } catch (error) {
    fail(`${label} is not valid JSON: ${(error as Error).message}`);
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    fail(`${label} must contain a JSON object`);
  }
  return { record, document: parsed as Record<string, unknown> };
}

function currentCommit(): string {
  const value = execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: repoRoot,
    encoding: 'utf8'
  }).trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail('git HEAD is not a full commit SHA');
  return value;
}

function playwrightVersion(): string {
  const packagePath = path.join(
    repoRoot,
    'apps/next-admin/node_modules/@playwright/test/package.json'
  );
  const document = JSON.parse(readFileSync(packagePath, 'utf8')) as {
    version?: unknown;
  };
  if (typeof document.version !== 'string' || document.version.length === 0) {
    fail('installed Playwright version is unavailable');
  }
  return document.version;
}

function sourceHashes(): Record<string, string> {
  if (
    !Array.isArray(contract.required_source_files) ||
    contract.required_source_files.length === 0
  ) {
    fail('matrix execution contract has no required source files');
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const record = regularFileRecord(
        relativePath,
        `source file ${relativePath}`
      );
      return [relativePath, record.sha256];
    })
  );
}

function outputPath(): string {
  const raw = optionalEnvironment(contract.output.environment);
  const absolute = resolveInput(raw ?? contract.output.default_path);
  const targetRoot = path.resolve(repoRoot, 'target');
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    fail('matrix output must remain inside repository target/');
  }
  return absolute;
}

function writeAtomic(
  location: string,
  document: Record<string, unknown>
): void {
  mkdirSync(path.dirname(location), { recursive: true });
  const temporary = `${location}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, 'utf8');
  renameSync(temporary, location);
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
    if (
      parsed === null ||
      typeof parsed !== 'object' ||
      Array.isArray(parsed)
    ) {
      fail(`${name} must contain a JSON object`);
    }
    for (const [headerName, headerValue] of Object.entries(parsed)) {
      const normalized = headerName.toLowerCase();
      if (!/^[a-z0-9!#$%&'*+.^_`|~-]+$/u.test(normalized)) {
        fail(`${name} contains an invalid header name`);
      }
      if (['authorization', 'cookie', 'set-cookie'].includes(normalized)) {
        fail(`${name} must not contain credential headers`);
      }
      if (
        typeof headerValue !== 'string' ||
        headerValue.length > 4096 ||
        /[\u0000\r\n]/u.test(headerValue)
      ) {
        fail(`${name} contains an invalid header value`);
      }
      headers[normalized] = headerValue;
    }
  }
  headers['x-tenant-slug'] = tenantSlug;
  return { headers, environmentNames: raw === null ? [] : [name] };
}

function validatePredecessor(
  document: Record<string, unknown>,
  head: string,
  apiOrigin: string,
  adminOrigin: string
): string {
  if (
    document.format !== contract.predecessor.format ||
    document.status !== contract.predecessor.status ||
    document.source_commit !== head
  ) {
    fail('browser predecessor identity/status/source commit drifted');
  }
  const target = document.target as Record<string, unknown> | undefined;
  if (target?.origin_sha256 !== sha256(apiOrigin)) {
    fail('matrix API origin does not match browser predecessor origin hash');
  }
  if (target?.standalone_origin_sha256 !== sha256(adminOrigin)) {
    fail(
      'matrix admin origin does not match browser predecessor standalone-origin hash'
    );
  }
  const deploymentDigest = target?.deployment_image_digest;
  if (
    typeof deploymentDigest !== 'string' ||
    !/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(deploymentDigest)
  ) {
    fail('browser predecessor has no immutable deployment RepoDigest');
  }
  const boundaries = document.boundaries as Record<string, unknown> | undefined;
  if (
    boundaries?.tenant_rollout_executed !== false ||
    boundaries?.ffa_promoted !== false ||
    boundaries?.fba_promoted !== false ||
    boundaries?.canonical_source_mutated !== false
  ) {
    fail('browser predecessor rollout/promotion/source boundary drifted');
  }
  return deploymentDigest;
}

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, canonicalize(nested)])
    );
  }
  return value;
}

function canonicalJson(value: unknown): string {
  return JSON.stringify(canonicalize(value));
}

function objectValue(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be a JSON object`);
  }
  return value as Record<string, unknown>;
}

function parseSettings(raw: unknown, label: string): Record<string, unknown> {
  if (typeof raw !== 'string' || Buffer.byteLength(raw, 'utf8') > 512 * 1024) {
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

function withProfile(
  original: Record<string, unknown>,
  flags: MatrixFlags
): Record<string, unknown> {
  const cloned = JSON.parse(JSON.stringify(original)) as Record<
    string,
    unknown
  >;
  const builder =
    cloned.builder !== null &&
    typeof cloned.builder === 'object' &&
    !Array.isArray(cloned.builder)
      ? { ...(cloned.builder as Record<string, unknown>) }
      : {};
  const nested = (value: unknown): Record<string, unknown> =>
    value !== null && typeof value === 'object' && !Array.isArray(value)
      ? { ...(value as Record<string, unknown>) }
      : {};
  builder.enabled = flags.builder_enabled;
  builder.preview = {
    ...nested(builder.preview),
    enabled: flags.preview_enabled
  };
  builder.properties = {
    ...nested(builder.properties),
    enabled: flags.properties_enabled
  };
  builder.publish = {
    ...nested(builder.publish),
    enabled: flags.publish_enabled
  };
  cloned.builder = builder;
  return cloned;
}

async function graphql(
  context: BrowserContext,
  query: string,
  variables: Record<string, unknown>,
  label: string
): Promise<GraphqlResult> {
  const response = await context.request.post(graphqlPath, {
    data: { query, variables },
    failOnStatusCode: false
  });
  const body = await response.body();
  if (response.status() !== 200) fail(`${label} did not return HTTP 200`);
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString('utf8'));
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
    data: objectValue(envelope.data, `${label} data`)
  };
}

async function loadPagesModule(
  apiContext: BrowserContext
): Promise<{ settings: Record<string, unknown>; read: GraphqlResult }> {
  const read = await graphql(
    apiContext,
    tenantModulesQuery,
    { limit: 100 },
    'tenantModules rollout snapshot'
  );
  const modules = read.data.tenantModules;
  if (!Array.isArray(modules)) fail('tenantModules did not return an array');
  const pages = modules.find(
    (entry) =>
      entry !== null &&
      typeof entry === 'object' &&
      (entry as Record<string, unknown>).moduleSlug === 'pages'
  ) as Record<string, unknown> | undefined;
  if (pages === undefined || pages.enabled !== true) {
    fail('Pages module must be enabled for rollout matrix execution');
  }
  return {
    settings: parseSettings(pages.settings, 'Pages module settings'),
    read
  };
}

async function writePagesSettings(
  apiContext: BrowserContext,
  settings: Record<string, unknown>
): Promise<GraphqlResult> {
  const result = await graphql(
    apiContext,
    updateSettingsMutation,
    { moduleSlug: 'pages', settings: JSON.stringify(settings) },
    'updateModuleSettings pages'
  );
  const module = objectValue(
    result.data.updateModuleSettings,
    'updated Pages module'
  );
  if (module.moduleSlug !== 'pages' || module.enabled !== true) {
    fail('updateModuleSettings did not return the enabled Pages module');
  }
  const returned = parseSettings(module.settings, 'updated Pages settings');
  if (canonicalJson(returned) !== canonicalJson(settings)) {
    fail(
      'updateModuleSettings returned settings different from the requested semantic object'
    );
  }
  return result;
}

async function readRolloutSnapshot(
  apiContext: BrowserContext,
  tenantSlug: string,
  profile: MatrixProfile
): Promise<GraphqlResult> {
  const result = await graphql(
    apiContext,
    rolloutSnapshotQuery,
    {},
    'pageBuilderRolloutSnapshot'
  );
  const snapshot = objectValue(
    result.data.pageBuilderRolloutSnapshot,
    'Page Builder rollout snapshot'
  );
  const expected = profile.flags;
  if (
    snapshot.tenantSlug !== tenantSlug ||
    snapshot.builderEnabled !== expected.builder_enabled ||
    snapshot.previewEnabled !== expected.preview_enabled ||
    snapshot.propertiesEnabled !== expected.properties_enabled ||
    snapshot.publishEnabled !== expected.publish_enabled ||
    snapshot.providerHealthObserved !== false
  ) {
    fail(`server-owned rollout snapshot does not match profile ${profile.id}`);
  }
  return result;
}

async function assertPagesReads(
  apiContext: BrowserContext,
  pageId: string
): Promise<GraphqlResult> {
  const result = await graphql(
    apiContext,
    pagesReadsQuery,
    { id: pageId },
    'Pages owned reads'
  );
  const list = objectValue(result.data.pages, 'Pages list read');
  if (typeof list.total !== 'number' || !Array.isArray(list.items)) {
    fail('Pages list read did not return the expected owner shape');
  }
  const page = objectValue(result.data.page, 'Pages document read');
  if (page.id !== pageId || typeof page.status !== 'string') {
    fail('Pages document read did not return the selected page');
  }
  return result;
}

async function settleAdminPage(page: Page, adminRoute: string): Promise<void> {
  const response = await page.goto(adminRoute, {
    waitUntil: 'domcontentloaded'
  });
  if (response === null || response.status() >= 400) {
    fail(
      'Pages admin rollout fixture route did not return a successful response'
    );
  }
  await page
    .waitForLoadState('networkidle', { timeout: 15_000 })
    .catch(() => undefined);
  await page
    .locator(providerSelector)
    .first()
    .waitFor({ state: 'visible', timeout: 15_000 });
}

async function assertUiProfile(
  page: Page,
  profile: MatrixProfile
): Promise<{
  provider_state: string;
  provider_health: string;
  preview_enabled: boolean;
  properties: 'enabled' | 'disabled' | 'hidden';
  publish: 'enabled' | 'disabled' | 'hidden';
}> {
  const provider = page.locator(providerSelector).first();
  const providerState = await provider.getAttribute(
    'data-fly-provider-control-state'
  );
  const providerHealth = await provider.getAttribute(
    'data-fly-provider-health'
  );
  if (
    providerState !== profile.provider_state ||
    providerHealth !== 'unobserved'
  ) {
    fail(`provider status UI does not match profile ${profile.id}`);
  }

  const preview = page.locator(previewPanelSelector).first();
  await preview.waitFor({ state: 'visible' });
  const previewEnabled =
    (await preview.getAttribute('data-page-builder-provider-preview')) ===
    'true';
  if (previewEnabled !== (profile.preview_ui === 'enabled')) {
    fail(`preview UI does not match profile ${profile.id}`);
  }
  if (
    (await preview.locator('button').first().isDisabled()) !== !previewEnabled
  ) {
    fail(`preview button disabled state does not match profile ${profile.id}`);
  }

  const capabilityState = async (
    selector: string,
    expectedEnabled: boolean,
    label: string
  ): Promise<'enabled' | 'disabled' | 'hidden'> => {
    const fieldset = page.locator(selector).first();
    if ((await fieldset.count()) === 0) {
      if (expectedEnabled)
        fail(`${label} fieldset is unexpectedly hidden for ${profile.id}`);
      return 'hidden';
    }
    const disabled = (await fieldset.getAttribute('disabled')) !== null;
    if (disabled === expectedEnabled) {
      fail(
        `${label} fieldset capability state does not match profile ${profile.id}`
      );
    }
    return disabled ? 'disabled' : 'enabled';
  };

  return {
    provider_state: providerState,
    provider_health: providerHealth,
    preview_enabled: previewEnabled,
    properties: await capabilityState(
      propertiesFieldsetSelector,
      profile.flags.builder_enabled && profile.flags.properties_enabled,
      'properties'
    ),
    publish: await capabilityState(
      publishFieldsetSelector,
      profile.flags.builder_enabled && profile.flags.publish_enabled,
      'publish'
    )
  };
}

async function captureReplayHeaders(
  request: Request
): Promise<Record<string, string>> {
  const original = await request.allHeaders();
  const headers: Record<string, string> = {};
  for (const [name, value] of Object.entries(original)) {
    const lower = name.toLowerCase();
    if (['host', 'content-length', 'connection', 'cookie'].includes(lower))
      continue;
    headers[lower] = value;
  }
  return headers;
}

async function allowedPreview(
  page: Page,
  captureTemplate: boolean
): Promise<{
  observation: PreviewObservation;
  template: PreviewTemplate | null;
}> {
  const panel = page.locator(previewPanelSelector).first();
  const button = panel.locator('button').first();
  if (await button.isDisabled()) fail('allowed preview button is disabled');
  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' &&
      new URL(request.url()).pathname === capabilityPath,
    { timeout: 15_000 }
  );
  await button.click();
  const request = await requestPromise;
  const response = await request.response();
  if (response === null) fail('server preview request produced no response');
  const responseBody = await response.body();
  if (
    response.status() >= 400 ||
    /capability disabled: preview/iu.test(responseBody.toString('utf8'))
  ) {
    fail('allowed preview profile was rejected by authoritative SSR dispatch');
  }
  await page
    .locator('[data-page-builder-server-preview-frame="true"]')
    .first()
    .waitFor({ state: 'attached', timeout: 15_000 });

  let template: PreviewTemplate | null = null;
  if (captureTemplate) {
    const body = request.postDataBuffer();
    if (body === null || body.length === 0)
      fail('server preview request body is unavailable');
    template = {
      url: request.url(),
      method: request.method(),
      body,
      headers: await captureReplayHeaders(request)
    };
  }
  return {
    observation: {
      status: response.status(),
      body_bytes: responseBody.length,
      body_sha256: sha256(responseBody),
      capability_disabled: false
    },
    template
  };
}

async function deniedPreview(
  adminContext: BrowserContext,
  template: PreviewTemplate
): Promise<PreviewObservation> {
  const response = await adminContext.request.fetch(template.url, {
    method: template.method,
    data: template.body,
    headers: template.headers,
    failOnStatusCode: false
  });
  const body = await response.body();
  if (!/capability disabled: preview/iu.test(body.toString('utf8'))) {
    fail(
      'disabled preview profile did not return the typed capability-disabled marker'
    );
  }
  return {
    status: response.status(),
    body_bytes: body.length,
    body_sha256: sha256(body),
    capability_disabled: true
  };
}

async function deniedBrowserIntent(
  adminContext: BrowserContext,
  pageId: string,
  intent: 'save' | 'rename_page',
  expectedCapability: 'publish' | 'properties'
): Promise<{
  status: number;
  body_bytes: number;
  body_sha256: string;
  code: string;
  capability: string;
  intent: string;
}> {
  const payload =
    intent === 'rename_page'
      ? { page_id: pageId, new_page_id: 'rollout-matrix-denied-probe' }
      : {};
  const response = await adminContext.request.post(
    `/api/admin/pages/${encodeURIComponent(pageId)}/builder/intents`,
    {
      data: {
        protocol: 'fly_iframe',
        instance_id: 'pages-rollout-matrix',
        intent,
        payload,
        page_id: pageId,
        sequence: 1
      },
      failOnStatusCode: false
    }
  );
  const body = await response.body();
  if (response.status() !== 403)
    fail(`${intent} rollout preflight did not return HTTP 403`);
  let parsed: unknown;
  try {
    parsed = JSON.parse(body.toString('utf8'));
  } catch {
    fail(`${intent} rollout preflight did not return JSON`);
  }
  const problem = objectValue(parsed, `${intent} rollout problem`);
  if (
    problem.status !== 403 ||
    problem.code !== 'FLY_CAPABILITY_DENIED' ||
    problem.intent !== intent ||
    problem.capability !== expectedCapability ||
    !Array.isArray(problem.missing) ||
    !problem.missing.includes(expectedCapability)
  ) {
    fail(
      `${intent} rollout preflight did not return the expected typed capability denial`
    );
  }
  return {
    status: response.status(),
    body_bytes: body.length,
    body_sha256: sha256(body),
    code: String(problem.code),
    capability: String(problem.capability),
    intent: String(problem.intent)
  };
}

function validateProfiles(): MatrixProfile[] {
  const expected = [
    ['all_on', true, true, true, true, 'unobserved'],
    ['publish_off', true, true, true, false, 'degraded'],
    ['preview_off', true, false, true, false, 'degraded'],
    ['builder_off', false, false, false, false, 'unavailable']
  ];
  const actual = contract.profiles.map((profile) => [
    profile.id,
    profile.flags.builder_enabled,
    profile.flags.preview_enabled,
    profile.flags.properties_enabled,
    profile.flags.publish_enabled,
    profile.provider_state
  ]);
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail('matrix contract profile order/identity/flags drifted');
  }
  return contract.profiles;
}

function responseRecord(response: GraphqlResult): Record<string, unknown> {
  return {
    status: response.status,
    response_body_bytes: response.responseBytes,
    response_body_sha256: response.responseSha256,
    raw_request_or_response_persisted: false
  };
}

test('Pages persisted rollout profiles agree across owner, UI, SSR, intents and reads', async ({
  browser
}) => {
  if (
    contract.schema_version !== 1 ||
    contract.module !== 'pages' ||
    contract.packet !== 'pages-builder-rollout-runtime-matrix' ||
    contract.status !== 'source_ready_maintainer_execution_pending'
  ) {
    fail('matrix execution contract identity drifted');
  }

  const head = currentCommit();
  const profiles = validateProfiles();
  const apiOrigin = requireOrigin(
    requiredEnvironment(contract.fixtures.api_origin_environment),
    'matrix API origin'
  );
  const adminOrigin = requireOrigin(
    requiredEnvironment(contract.fixtures.admin_origin_environment),
    'matrix standalone admin origin'
  );
  if (apiOrigin === adminOrigin)
    fail('matrix API and standalone admin origins must be distinct');

  const tenantSlug = requireTenantSlug(
    requiredEnvironment(contract.fixtures.tenant_slug_environment)
  );
  const pageId = requireUuid(
    requiredEnvironment(contract.fixtures.page_id_environment)
  );
  const adminRoute = requireRelativePath(
    requiredEnvironment(contract.fixtures.admin_route_environment),
    'matrix admin route'
  );
  const apiStorage = regularFileRecord(
    requiredEnvironment(contract.fixtures.api_storage_state_environment),
    'matrix API operator storage state'
  );
  const adminStorage = regularFileRecord(
    requiredEnvironment(contract.fixtures.admin_storage_state_environment),
    'matrix standalone admin operator storage state'
  );
  const predecessor = readJsonInput(
    requiredEnvironment(contract.predecessor.environment),
    'Pages inline-edit browser predecessor'
  );
  const deploymentDigest = validatePredecessor(
    predecessor.document,
    head,
    apiOrigin,
    adminOrigin
  );
  const shared = commonHeaders(tenantSlug);
  const output = outputPath();
  rmSync(output, { force: true });

  const apiContext = await browser.newContext({
    baseURL: apiOrigin,
    storageState: apiStorage.path,
    extraHTTPHeaders: shared.headers
  });
  const adminContext = await browser.newContext({
    baseURL: adminOrigin,
    storageState: adminStorage.path,
    extraHTTPHeaders: shared.headers
  });

  const profileObservations: Record<string, unknown> = {};
  let originalSettings: Record<string, unknown> | null = null;
  let originalSettingsHash = '';
  let previewTemplate: PreviewTemplate | null = null;
  let restoreVerified = false;

  try {
    const original = await loadPagesModule(apiContext);
    originalSettings = original.settings;
    originalSettingsHash = sha256(canonicalJson(original.settings));

    for (const profile of profiles) {
      const profileSettings = withProfile(original.settings, profile.flags);
      const settingsWrite = await writePagesSettings(
        apiContext,
        profileSettings
      );
      const serverSnapshot = await readRolloutSnapshot(
        apiContext,
        tenantSlug,
        profile
      );
      const reads = await assertPagesReads(apiContext, pageId);

      const page = await adminContext.newPage();
      try {
        await settleAdminPage(page, adminRoute);
        const ui = await assertUiProfile(page, profile);

        let preview: PreviewObservation;
        if (profile.preview_ssr === 'pass') {
          const allowed = await allowedPreview(page, profile.id === 'all_on');
          preview = allowed.observation;
          if (allowed.template !== null) previewTemplate = allowed.template;
        } else {
          if (previewTemplate === null) {
            fail(
              'disabled preview profile has no captured all_on request template'
            );
          }
          preview = await deniedPreview(adminContext, previewTemplate);
        }

        let publishDry: Record<string, unknown>;
        if (profile.id === 'all_on') {
          if (ui.publish !== 'enabled')
            fail('all_on publish capability is not enabled in UI');
          publishDry = {
            ui_capability_enabled: true,
            mutating_save_request_sent: false
          };
        } else {
          publishDry = await deniedBrowserIntent(
            adminContext,
            pageId,
            'save',
            'publish'
          );
        }

        const propertiesDenial =
          profile.id === 'builder_off'
            ? await deniedBrowserIntent(
                adminContext,
                pageId,
                'rename_page',
                'properties'
              )
            : null;

        profileObservations[profile.id] = {
          flags: profile.flags,
          settings_write: responseRecord(settingsWrite),
          server_snapshot: {
            ...responseRecord(serverSnapshot),
            tenant_match: true,
            flags_match: true,
            provider_health_observed: false
          },
          pages_owned_reads: {
            ...responseRecord(reads),
            list_read: true,
            document_read: true
          },
          ui,
          preview_ssr: preview,
          publish_dry: publishDry,
          properties_denial: propertiesDenial
        };
      } finally {
        await page.close();
      }
    }
  } finally {
    try {
      if (originalSettings !== null) {
        await writePagesSettings(apiContext, originalSettings);
        const restored = await loadPagesModule(apiContext);
        if (
          canonicalJson(restored.settings) !== canonicalJson(originalSettings)
        ) {
          fail(
            'Pages module settings were not semantically restored after matrix execution'
          );
        }
        if (sha256(canonicalJson(restored.settings)) !== originalSettingsHash) {
          fail(
            'restored Pages settings hash does not match the original snapshot'
          );
        }
        restoreVerified = true;
      }
    } finally {
      await Promise.all([apiContext.close(), adminContext.close()]);
    }
  }

  if (!restoreVerified || originalSettings === null) {
    fail(
      'matrix completed without verified restoration of original Pages settings'
    );
  }

  writeAtomic(output, {
    format: contract.output.format,
    status: contract.output.status,
    source_commit: head,
    generated_at: new Date().toISOString(),
    toolchain: {
      node: process.version,
      playwright: playwrightVersion(),
      browser_project: 'pages-builder-rollout-matrix-chromium'
    },
    source_sha256: sourceHashes(),
    inputs: {
      browser_predecessor: {
        bytes: predecessor.record.bytes,
        sha256: predecessor.record.sha256
      },
      storage_states: {
        api: {
          bytes: apiStorage.bytes,
          sha256: apiStorage.sha256
        },
        admin: {
          bytes: adminStorage.bytes,
          sha256: adminStorage.sha256
        }
      },
      common_header_environment_names: shared.environmentNames
    },
    target: {
      api_origin_sha256: sha256(apiOrigin),
      admin_origin_sha256: sha256(adminOrigin),
      deployment_image_digest: deploymentDigest
    },
    identity_sha256: {
      tenant_slug: sha256(tenantSlug),
      page_id: sha256(pageId),
      admin_route: sha256(adminRoute)
    },
    original_settings: {
      semantic_sha256: originalSettingsHash,
      raw_settings_persisted: false,
      restored: true,
      restore_verified: true
    },
    profiles: profileObservations,
    boundaries: {
      runtime_matrix_executed: true,
      owner_review_pending: true,
      gate_accepted: false,
      forum_wave_accepted: false,
      provider_health_observed: false,
      ffa_promoted: false,
      fba_promoted: false,
      canonical_source_mutated: false
    },
    privacy: {
      tenant_slug_or_id_persisted: false,
      page_id_or_admin_route_persisted: false,
      authorization_or_cookie_values_persisted: false,
      storage_state_contents_persisted: false,
      tokens_or_session_ids_persisted: false,
      raw_module_settings_persisted: false,
      raw_graphql_bodies_persisted: false,
      raw_preview_request_or_response_persisted: false,
      raw_browser_intent_response_persisted: false,
      raw_html_persisted: false,
      traces_persisted: false,
      screenshots_persisted: false,
      videos_persisted: false
    }
  });
});
