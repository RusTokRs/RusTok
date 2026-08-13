import {
  expect,
  test,
  type APIResponse,
  type BrowserContext,
  type Locator,
  type Page,
  type Request,
  type Response
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
  'crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json';
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), 'utf8')
) as BrowserContract;
const commitPath = '/api/fn/pages/inline-edit/commit';
const authoringPath = '/modules/pages-authoring';
const launchSelector =
  '[data-pages-inline-edit-launch="same-origin"] a[aria-label="Open authenticated inline editor"]';
const pageBuilderRootSelector =
  '[data-rustok-page-builder-inline-storefront="true"]';
const authoringSurfaceSelector =
  '[data-pages-authenticated-inline-edit="true"]';
const authoringAssetPaths = new Map([
  ['bootstrap', '/assets/pages-inline-edit-bootstrap.js'],
  ['module', '/assets/pages-inline-edit/rustok_storefront.js'],
  ['wasm', '/assets/pages-inline-edit/rustok_storefront_bg.wasm']
]);
const forbiddenDomMarkers = [
  'data-inline-session',
  'data-inline-proof',
  'authorization_proof',
  'access_token',
  'refresh_token',
  'signing_secret'
];
const forbiddenUrlKeys = new Set([
  'authorization',
  'token',
  'access_token',
  'refresh_token',
  'session',
  'session_id',
  'grant',
  'proof',
  'authorization_proof',
  'signing_secret'
]);

type BrowserContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  artifact_http_input: {
    environment: string;
    format: string;
    status: string;
  };
  output: {
    environment: string;
    default_path: string;
    format: string;
    status: string;
  };
  required_source_files: string[];
};

type FileRecord = {
  bytes: number;
  sha256: string;
};

type FailureCounters = {
  console_errors: number;
  page_errors: number;
  critical_request_failures: number;
};

type RootFacts = {
  id: string;
  pageId: string;
  revision: string;
  projectHash: string;
};

type CommitCapture = {
  request: Request;
  response: Response;
  requestBody: Buffer;
  responseBody: Buffer;
  requestCount: number;
};

function fail(message: string): never {
  throw new Error(`Pages inline edit browser evidence failed: ${message}`);
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
    fail(
      `${label} must be an HTTP(S) origin without credentials, path, query, or fragment`
    );
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

function requireUuid(value: string, label: string): string {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(
      value
    )
  ) {
    fail(`${label} must be a UUID`);
  }
  return value.toLowerCase();
}

function requireLocale(value: string): string {
  if (
    value.trim() !== value ||
    value.length === 0 ||
    Buffer.byteLength(value, 'utf8') > 64 ||
    /[\u0000-\u001f\u007f/\\?#]/u.test(value)
  ) {
    fail('draft locale must be a bounded path-safe value');
  }
  return value;
}

function requireComponentId(value: string, label: string): string {
  if (!/^[A-Za-z0-9_-]{1,128}$/u.test(value)) {
    fail(
      `${label} must use the bounded browser evidence component-id alphabet`
    );
  }
  return value;
}

function requireDeploymentDigest(value: string): string {
  if (!/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(value)) {
    fail('deployment digest must be an immutable image RepoDigest');
  }
  return value;
}

function requireExpiryDelay(value: string): number {
  if (!/^\d+$/u.test(value)) fail('expiry delay must be an integer');
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 1_000 || parsed > 120_000) {
    fail('expiry delay must be between 1000 and 120000 milliseconds');
  }
  return parsed;
}

function resolveInput(value: string): string {
  return path.isAbsolute(value)
    ? path.resolve(value)
    : path.resolve(repoRoot, value);
}

function regularFileRecord(
  value: string,
  label: string
): FileRecord & { path: string } {
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
  record: FileRecord & { path: string };
  document: Record<string, unknown>;
} {
  const record = regularFileRecord(value, label);
  let document: unknown;
  try {
    document = JSON.parse(readFileSync(record.path, 'utf8'));
  } catch (error) {
    fail(`${label} is not valid JSON: ${(error as Error).message}`);
  }
  if (
    document === null ||
    typeof document !== 'object' ||
    Array.isArray(document)
  ) {
    fail(`${label} must contain a JSON object`);
  }
  return { record, document: document as Record<string, unknown> };
}

function currentCommit(): string {
  const value = execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: repoRoot,
    encoding: 'utf8'
  }).trim();
  if (!/^[0-9a-f]{40}$/u.test(value)) fail('git HEAD is not a full commit SHA');
  return value;
}

function sourceHashes(): Record<string, string> {
  if (
    !Array.isArray(contract.required_source_files) ||
    contract.required_source_files.length === 0
  ) {
    fail('browser execution contract has no required source files');
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

function commonHeaders(): {
  headers: Record<string, string>;
  environmentNames: string[];
} {
  const name = 'RUSTOK_PAGES_INLINE_EDIT_BROWSER_COMMON_HEADERS_JSON';
  const raw = optionalEnvironment(name);
  if (raw === null) return { headers: {}, environmentNames: [] };
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    fail(`${name} must contain a JSON object: ${(error as Error).message}`);
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    fail(`${name} must contain a JSON object`);
  }
  const headers: Record<string, string> = {};
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
  return { headers, environmentNames: [name] };
}

function outputPath(): string {
  const raw = optionalEnvironment(contract.output.environment, 16_384);
  const absolute = resolveInput(raw ?? contract.output.default_path);
  const targetRoot = path.resolve(repoRoot, 'target');
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    fail('browser evidence output must remain inside repository target/');
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

function validateArtifactHttpInput(
  document: Record<string, unknown>,
  head: string,
  origin: string,
  deploymentDigest: string
): void {
  if (
    document.format !== contract.artifact_http_input.format ||
    document.status !== contract.artifact_http_input.status ||
    document.source_commit !== head
  ) {
    fail('artifact/HTTP evidence identity, status, or source commit drifted');
  }
  const http = document.http as Record<string, unknown> | undefined;
  const docker = document.docker as Record<string, unknown> | undefined;
  if (
    http?.origin !== origin ||
    http?.deployment_image_digest !== deploymentDigest
  ) {
    fail(
      'artifact/HTTP origin or deployment digest does not match browser target'
    );
  }
  if (
    !Array.isArray(docker?.repo_digests) ||
    !docker.repo_digests.includes(deploymentDigest)
  ) {
    fail(
      'browser deployment digest is absent from artifact/HTTP Docker evidence'
    );
  }
  const boundaries = document.boundaries as Record<string, unknown> | undefined;
  if (
    boundaries?.browser_edit_save_replay_expiry_executed !== false ||
    boundaries?.tenant_rollout_executed !== false
  ) {
    fail('artifact/HTTP input browser or rollout boundary drifted');
  }
}

function observeFailures(page: Page): FailureCounters {
  const counters: FailureCounters = {
    console_errors: 0,
    page_errors: 0,
    critical_request_failures: 0
  };
  page.on('console', (message) => {
    if (message.type() === 'error') counters.console_errors += 1;
  });
  page.on('pageerror', () => {
    counters.page_errors += 1;
  });
  page.on('requestfailed', (request) => {
    const pathname = new URL(request.url()).pathname;
    const critical =
      ['document', 'script', 'stylesheet'].includes(request.resourceType()) ||
      pathname.endsWith('.wasm');
    if (critical) counters.critical_request_failures += 1;
  });
  return counters;
}

async function settlePage(page: Page, route: string): Promise<void> {
  const response = await page.goto(route, { waitUntil: 'domcontentloaded' });
  if (response === null) fail('browser navigation did not return a response');
  if (response.status() >= 400)
    fail('browser fixture route did not return a successful response');
  await page
    .waitForLoadState('networkidle', { timeout: 15_000 })
    .catch(() => undefined);
  await page.waitForTimeout(500);
}

async function assertLaunchHidden(
  context: BrowserContext,
  route: string
): Promise<boolean> {
  const page = await context.newPage();
  try {
    await settlePage(page, route);
    await expect(page.locator(launchSelector)).toHaveCount(0);
    return true;
  } finally {
    await page.close();
  }
}

function validateLaunchHref(
  href: string | null,
  baseOrigin: string,
  pageId: string,
  locale: string
): URL {
  if (href === null || !href.startsWith('/') || href.startsWith('//')) {
    fail('allowed launch must use a relative same-origin href');
  }
  const target = new URL(href, baseOrigin);
  if (
    target.origin !== baseOrigin ||
    target.pathname !== authoringPath ||
    target.searchParams.size !== 2 ||
    target.searchParams.get('page_id') !== pageId ||
    target.searchParams.get('lang') !== locale
  ) {
    fail(
      'allowed launch target does not bind the exact same-origin page and locale query'
    );
  }
  for (const key of target.searchParams.keys()) {
    if (forbiddenUrlKeys.has(key.toLowerCase())) {
      fail('allowed launch URL contains a forbidden credential or proof key');
    }
  }
  return target;
}

function scanForbiddenDomMarkers(source: string, label: string): void {
  const lower = source.toLowerCase();
  for (const marker of forbiddenDomMarkers) {
    if (lower.includes(marker.toLowerCase())) {
      fail(
        `${label} contains forbidden session, grant, proof, token, or signing marker`
      );
    }
  }
}

function domId(value: string): string {
  return [...value]
    .map((character) => (/[A-Za-z0-9_-]/u.test(character) ? character : '-'))
    .join('');
}

async function waitForAuthoringRoot(page: Page): Promise<RootFacts> {
  await page.locator(authoringSurfaceSelector).waitFor({ state: 'visible' });
  const root = page.locator(pageBuilderRootSelector);
  await root.waitFor({ state: 'visible' });
  await expect(root).not.toHaveAttribute('data-render-error', 'true');
  const [id, pageId, revision, projectHash, sessionAttribute, proofAttribute] =
    await Promise.all([
      root.getAttribute('id'),
      root.getAttribute('data-page-id'),
      root.getAttribute('data-inline-revision'),
      root.getAttribute('data-inline-project-hash'),
      root.getAttribute('data-inline-session'),
      root.getAttribute('data-inline-proof')
    ]);
  if (
    !id ||
    !pageId ||
    !revision ||
    !projectHash ||
    !/^[0-9a-f]+$/u.test(projectHash)
  ) {
    fail('hydrated Page Builder root identity is incomplete');
  }
  if (sessionAttribute !== null || proofAttribute !== null) {
    fail('hydrated Page Builder root exposes a session or proof attribute');
  }
  if (id !== `fly-inline-${domId(pageId)}-${projectHash}`) {
    fail(
      'hydrated Page Builder root id is not bound to page identity and project hash'
    );
  }
  return { id, pageId, revision, projectHash };
}

function componentLocator(page: Page, componentId: string): Locator {
  return page.locator(`[data-fly-component-id="${componentId}"]`);
}

async function assertEligibility(
  page: Page,
  editableId: string,
  blockedIds: string[]
): Promise<void> {
  const editable = componentLocator(page, editableId);
  await expect(editable).toHaveCount(1);
  await expect(editable).toHaveAttribute('data-fly-inline-editable', 'content');
  await expect(editable).toHaveAttribute('contenteditable', 'plaintext-only');
  for (const componentId of blockedIds) {
    const blocked = componentLocator(page, componentId);
    await expect(blocked).toHaveCount(1);
    if (
      (await blocked.getAttribute('data-fly-inline-editable')) !== null ||
      (await blocked.getAttribute('contenteditable')) !== null
    ) {
      fail(
        'a provider, composite, templated, interactive, or runtime-owned component became editable'
      );
    }
  }
}

async function mutateEditable(locator: Locator, value: string): Promise<void> {
  await locator.evaluate((element, nextValue) => {
    const target = element as HTMLElement;
    target.innerText = nextValue;
    target.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
  }, value);
}

async function captureCommit(
  page: Page,
  editableId: string,
  value: string
): Promise<CommitCapture> {
  let requestCount = 0;
  const count = (request: Request) => {
    if (new URL(request.url()).pathname === commitPath) requestCount += 1;
  };
  page.on('request', count);
  try {
    const requestPromise = page.waitForRequest(
      (request) => new URL(request.url()).pathname === commitPath
    );
    const responsePromise = page.waitForResponse(
      (response) => new URL(response.url()).pathname === commitPath
    );
    await mutateEditable(componentLocator(page, editableId), value);
    const [request, response] = await Promise.all([
      requestPromise,
      responsePromise
    ]);
    const requestBody = request.postDataBuffer() ?? Buffer.alloc(0);
    const responseBody = await response.body();
    await page.waitForTimeout(250);
    return { request, response, requestBody, responseBody, requestCount };
  } finally {
    page.off('request', count);
  }
}

async function captureRejectedCommit(
  page: Page,
  editableId: string,
  value: string
): Promise<{ status: number; body: Buffer }> {
  const responsePromise = page.waitForResponse(
    (response) => new URL(response.url()).pathname === commitPath
  );
  await mutateEditable(componentLocator(page, editableId), value);
  const response = await responsePromise;
  const body = await response.body();
  if (response.status() < 400)
    fail('expected inline commit rejection returned a success status');
  await page.getByRole('alert').waitFor({ state: 'visible' });
  return { status: response.status(), body };
}

async function replaySuccessfulRequest(
  context: BrowserContext,
  capture: CommitCapture
): Promise<APIResponse> {
  const original = await capture.request.allHeaders();
  const headers: Record<string, string> = {};
  for (const [name, value] of Object.entries(original)) {
    const lower = name.toLowerCase();
    if (['host', 'content-length', 'connection', 'cookie'].includes(lower))
      continue;
    headers[lower] = value;
  }
  return context.request.fetch(capture.request.url(), {
    method: capture.request.method(),
    headers,
    data: capture.requestBody,
    failOnStatusCode: false
  });
}

async function textForComponent(
  page: Page,
  componentId: string
): Promise<string> {
  const value = await componentLocator(page, componentId).innerText();
  return value.replace(/\r\n/gu, '\n').replace(/\r/gu, '\n');
}

test('retains bounded Pages inline edit browser evidence', async ({
  browser
}) => {
  if (
    contract.schema_version !== 1 ||
    contract.module !== 'pages' ||
    contract.packet !== 'pages-inline-edit-browser-execution' ||
    contract.status !== 'source_ready_maintainer_execution_pending'
  ) {
    fail('browser execution contract identity drifted');
  }

  const output = outputPath();
  rmSync(output, { force: true });

  const head = currentCommit();
  const baseOrigin = requireOrigin(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_BASE_URL'),
    'browser base URL'
  );
  const standaloneOrigin = requireOrigin(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_STANDALONE_BASE_URL'),
    'standalone browser base URL'
  );
  if (standaloneOrigin === baseOrigin) {
    fail('standalone admin evidence must use a distinct origin');
  }
  const deploymentDigest = requireDeploymentDigest(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_DEPLOYMENT_DIGEST')
  );
  const pageId = requireUuid(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_DRAFT_PAGE_ID'),
    'draft Pages page id'
  );
  const locale = requireLocale(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_DRAFT_LOCALE')
  );
  const paths = {
    draft: requireRelativePath(
      requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_DRAFT_ADMIN_PATH'),
      'draft admin path'
    ),
    published: requireRelativePath(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_PUBLISHED_ADMIN_PATH'
      ),
      'published admin path'
    ),
    localeLess: requireRelativePath(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_LOCALELESS_ADMIN_PATH'
      ),
      'locale-less admin path'
    ),
    missing: requireRelativePath(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_MISSING_ADMIN_PATH'
      ),
      'missing admin path'
    ),
    standalone: requireRelativePath(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_STANDALONE_ADMIN_PATH'
      ),
      'standalone admin path'
    )
  };
  const componentIds = {
    editable: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_EDITABLE_COMPONENT_ID'
      ),
      'editable component id'
    ),
    provider: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_PROVIDER_COMPONENT_ID'
      ),
      'provider component id'
    ),
    composite: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_COMPOSITE_COMPONENT_ID'
      ),
      'composite component id'
    ),
    templated: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_TEMPLATED_COMPONENT_ID'
      ),
      'templated component id'
    ),
    interactive: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_INTERACTIVE_COMPONENT_ID'
      ),
      'interactive component id'
    ),
    runtime: requireComponentId(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_RUNTIME_COMPONENT_ID'
      ),
      'runtime-owned component id'
    )
  };
  if (
    new Set(Object.values(componentIds)).size !==
    Object.keys(componentIds).length
  ) {
    fail('browser evidence component identities must be unique');
  }
  const expiryDelayMs = requireExpiryDelay(
    requiredEnvironment('RUSTOK_PAGES_INLINE_EDIT_BROWSER_EXPIRY_DELAY_MS')
  );

  const storageInputs = {
    editor: regularFileRecord(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_EDITOR_STORAGE_STATE'
      ),
      'editor storage state'
    ),
    unauthorized: regularFileRecord(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_UNAUTHORIZED_STORAGE_STATE'
      ),
      'unauthorized storage state'
    ),
    standalone: regularFileRecord(
      requiredEnvironment(
        'RUSTOK_PAGES_INLINE_EDIT_BROWSER_STANDALONE_STORAGE_STATE'
      ),
      'standalone storage state'
    )
  };
  const artifactInput = readJsonInput(
    requiredEnvironment(
      'RUSTOK_PAGES_INLINE_EDIT_BROWSER_ARTIFACT_HTTP_EVIDENCE'
    ),
    'artifact/HTTP evidence'
  );
  validateArtifactHttpInput(
    artifactInput.document,
    head,
    baseOrigin,
    deploymentDigest
  );
  const shared = commonHeaders();

  const editorContext = await browser.newContext({
    baseURL: baseOrigin,
    storageState: storageInputs.editor.path,
    extraHTTPHeaders: shared.headers
  });
  const unauthorizedContext = await browser.newContext({
    baseURL: baseOrigin,
    storageState: storageInputs.unauthorized.path,
    extraHTTPHeaders: shared.headers
  });
  const standaloneContext = await browser.newContext({
    baseURL: standaloneOrigin,
    storageState: storageInputs.standalone.path,
    extraHTTPHeaders: shared.headers
  });

  try {
    const draftAdmin = await editorContext.newPage();
    await settlePage(draftAdmin, paths.draft);
    const launch = draftAdmin.locator(launchSelector);
    await launch.waitFor({ state: 'visible' });
    const launchTarget = validateLaunchHref(
      await launch.getAttribute('href'),
      baseOrigin,
      pageId,
      locale
    );

    const popupPromise = editorContext.waitForEvent('page');
    await launch.click();
    const popup = await popupPromise;
    await popup.waitForLoadState('domcontentloaded');
    if (new URL(popup.url()).toString() !== launchTarget.toString()) {
      fail(
        'admin launch did not navigate to the exact same-origin authoring target'
      );
    }
    await popup.close();
    await draftAdmin.close();

    const hidden = {
      published: await assertLaunchHidden(editorContext, paths.published),
      locale_less: await assertLaunchHidden(editorContext, paths.localeLess),
      missing: await assertLaunchHidden(editorContext, paths.missing),
      unauthorized: await assertLaunchHidden(unauthorizedContext, paths.draft),
      standalone: await assertLaunchHidden(standaloneContext, paths.standalone)
    };

    const ssrResponse = await editorContext.request.get(
      launchTarget.toString(),
      {
        failOnStatusCode: false
      }
    );
    if (ssrResponse.status() !== 200)
      fail('authenticated authoring SSR request did not return 200');
    const ssrHtml = await ssrResponse.text();
    scanForbiddenDomMarkers(ssrHtml, 'authoring SSR HTML');
    if (
      !ssrHtml.includes('id="pages-inline-edit-client-root"') ||
      !ssrHtml.includes(`data-pages-page-id="${pageId}"`) ||
      !ssrHtml.includes(`data-pages-locale="${locale}"`)
    ) {
      fail(
        'authoring SSR HTML does not bind the expected root, page, and locale'
      );
    }

    const mainPage = await editorContext.newPage();
    const mainFailures = observeFailures(mainPage);
    const assetStatuses = new Map<string, number>();
    mainPage.on('response', (response) => {
      const pathname = new URL(response.url()).pathname;
      for (const [id, expectedPath] of authoringAssetPaths) {
        if (pathname === expectedPath) assetStatuses.set(id, response.status());
      }
    });
    await mainPage.goto(launchTarget.toString(), {
      waitUntil: 'domcontentloaded'
    });
    const initialRoot = await waitForAuthoringRoot(mainPage);
    await expect.poll(() => assetStatuses.size).toBe(authoringAssetPaths.size);
    for (const [id] of authoringAssetPaths) {
      if (assetStatuses.get(id) !== 200)
        fail(`authoring ${id} asset did not return 200`);
    }
    if (
      mainFailures.console_errors !== 0 ||
      mainFailures.page_errors !== 0 ||
      mainFailures.critical_request_failures !== 0
    ) {
      fail('dedicated authoring client produced a bounded browser failure');
    }
    scanForbiddenDomMarkers(await mainPage.content(), 'hydrated authoring DOM');
    for (const key of new URL(mainPage.url()).searchParams.keys()) {
      if (forbiddenUrlKeys.has(key.toLowerCase())) {
        fail(
          'hydrated authoring URL contains a forbidden credential or proof key'
        );
      }
    }

    await assertEligibility(mainPage, componentIds.editable, [
      componentIds.provider,
      componentIds.composite,
      componentIds.templated,
      componentIds.interactive,
      componentIds.runtime
    ]);

    const stalePage = await editorContext.newPage();
    const staleFailures = observeFailures(stalePage);
    await stalePage.goto(launchTarget.toString(), {
      waitUntil: 'domcontentloaded'
    });
    const staleInitialRoot = await waitForAuthoringRoot(stalePage);
    if (
      staleInitialRoot.revision !== initialRoot.revision ||
      staleInitialRoot.projectHash !== initialRoot.projectHash
    ) {
      fail(
        'preloaded stale tab did not start from the same document revision and hash'
      );
    }

    const unique = `${Date.now()}-${Math.floor(Math.random() * 1_000_000)}`;
    const savedText = `Pages inline evidence saved ${unique}`;
    const staleText = `Pages inline evidence stale ${unique}`;
    const expiredText = `Pages inline evidence expired ${unique}`;

    const successful = await captureCommit(
      mainPage,
      componentIds.editable,
      savedText
    );
    if (successful.response.status() !== 200 || successful.requestCount !== 1) {
      fail(
        'one changed focusout did not produce exactly one successful commit request'
      );
    }
    await mainPage.getByRole('status').waitFor({ state: 'visible' });
    await expect
      .poll(async () => waitForAuthoringRoot(mainPage))
      .not.toEqual(initialRoot);
    const currentRoot = await waitForAuthoringRoot(mainPage);
    if (
      currentRoot.revision === initialRoot.revision ||
      currentRoot.projectHash === initialRoot.projectHash
    ) {
      fail('successful save did not replace revision and project hash');
    }

    const replay = await replaySuccessfulRequest(editorContext, successful);
    const replayBody = await replay.body();
    if (replay.status() < 400)
      fail('exact successful inline request replay was accepted');

    const staleRejected = await captureRejectedCommit(
      stalePage,
      componentIds.editable,
      staleText
    );
    await stalePage.reload({ waitUntil: 'domcontentloaded' });
    const staleReloadRoot = await waitForAuthoringRoot(stalePage);
    if (
      (await textForComponent(stalePage, componentIds.editable)) !==
        savedText ||
      staleReloadRoot.revision !== currentRoot.revision
    ) {
      fail(
        'stale rejection produced a partial document write or wrong reload revision'
      );
    }
    if (
      staleFailures.console_errors !== 0 ||
      staleFailures.page_errors !== 0 ||
      staleFailures.critical_request_failures !== 0
    ) {
      fail('stale browser scenario produced a bounded browser failure');
    }

    await mainPage.reload({ waitUntil: 'domcontentloaded' });
    const reloadedRoot = await waitForAuthoringRoot(mainPage);
    if (
      (await textForComponent(mainPage, componentIds.editable)) !== savedText ||
      reloadedRoot.revision !== currentRoot.revision ||
      reloadedRoot.projectHash !== currentRoot.projectHash
    ) {
      fail(
        'successful inline text or replacement revision did not survive reload'
      );
    }

    const expiryPage = await editorContext.newPage();
    const expiryFailures = observeFailures(expiryPage);
    await expiryPage.goto(launchTarget.toString(), {
      waitUntil: 'domcontentloaded'
    });
    const expiryInitialRoot = await waitForAuthoringRoot(expiryPage);
    if (expiryInitialRoot.revision !== currentRoot.revision) {
      fail('expiry tab did not load the current saved revision');
    }
    await expiryPage.waitForTimeout(expiryDelayMs);
    const expiryRejected = await captureRejectedCommit(
      expiryPage,
      componentIds.editable,
      expiredText
    );
    await expiryPage.reload({ waitUntil: 'domcontentloaded' });
    const expiryReloadRoot = await waitForAuthoringRoot(expiryPage);
    if (
      (await textForComponent(expiryPage, componentIds.editable)) !==
        savedText ||
      expiryReloadRoot.revision !== currentRoot.revision
    ) {
      fail(
        'expiry rejection produced a partial document write or wrong reload revision'
      );
    }
    if (
      expiryFailures.console_errors !== 0 ||
      expiryFailures.page_errors !== 0 ||
      expiryFailures.critical_request_failures !== 0
    ) {
      fail('expiry browser scenario produced a bounded browser failure');
    }

    const outputDocument: Record<string, unknown> = {
      format: contract.output.format,
      status: contract.output.status,
      source_commit: head,
      generated_at: new Date().toISOString(),
      toolchain: {
        node: process.version,
        playwright: playwrightVersion(),
        browser_project: 'pages-inline-edit-chromium'
      },
      source_sha256: sourceHashes(),
      inputs: {
        artifact_http: {
          bytes: artifactInput.record.bytes,
          sha256: artifactInput.record.sha256
        },
        storage_states: {
          editor: {
            bytes: storageInputs.editor.bytes,
            sha256: storageInputs.editor.sha256
          },
          unauthorized: {
            bytes: storageInputs.unauthorized.bytes,
            sha256: storageInputs.unauthorized.sha256
          },
          standalone: {
            bytes: storageInputs.standalone.bytes,
            sha256: storageInputs.standalone.sha256
          }
        },
        common_header_environment_names: shared.environmentNames
      },
      target: {
        origin_sha256: sha256(baseOrigin),
        standalone_origin_sha256: sha256(standaloneOrigin),
        deployment_image_digest: deploymentDigest
      },
      identity_sha256: {
        pages_page: sha256(pageId),
        locale: sha256(locale),
        components: Object.fromEntries(
          Object.entries(componentIds).map(([role, value]) => [
            role,
            sha256(value)
          ])
        ),
        edited_values: {
          saved: sha256(savedText),
          stale: sha256(staleText),
          expired: sha256(expiredText)
        }
      },
      launch: {
        allowed_draft_visible: true,
        relative_same_origin_exact_locale: true,
        hidden
      },
      authoring: {
        ssr_status: ssrResponse.status(),
        ssr_body_bytes: Buffer.byteLength(ssrHtml),
        ssr_body_sha256: sha256(ssrHtml),
        ssr_raw_html_persisted: false,
        hydrated_root_bound_to_page_and_project_hash: true,
        session_grant_proof_markers_absent: true,
        asset_statuses: Object.fromEntries(assetStatuses),
        failures: mainFailures
      },
      eligibility: {
        editable_static_leaf_count: 1,
        provider_component_read_only: true,
        composite_component_read_only: true,
        templated_component_read_only: true,
        interactive_component_read_only: true,
        runtime_owned_component_read_only: true
      },
      save: {
        commit_request_count: successful.requestCount,
        request_body_bytes: successful.requestBody.length,
        request_body_sha256: sha256(successful.requestBody),
        response_status: successful.response.status(),
        response_body_bytes: successful.responseBody.length,
        response_body_sha256: sha256(successful.responseBody),
        replacement_revision_observed: true,
        replacement_project_hash_observed: true,
        reload_persistence_observed: true,
        raw_request_or_response_persisted: false
      },
      replay: {
        response_status: replay.status(),
        response_body_bytes: replayBody.length,
        response_body_sha256: sha256(replayBody),
        exact_successful_request_rejected: true,
        raw_request_or_response_persisted: false
      },
      stale: {
        response_status: staleRejected.status,
        response_body_bytes: staleRejected.body.length,
        response_body_sha256: sha256(staleRejected.body),
        partial_write_absent_after_reload: true,
        failures: staleFailures,
        raw_response_persisted: false
      },
      expiry: {
        delay_ms: expiryDelayMs,
        response_status: expiryRejected.status,
        response_body_bytes: expiryRejected.body.length,
        response_body_sha256: sha256(expiryRejected.body),
        partial_write_absent_after_reload: true,
        failures: expiryFailures,
        raw_response_persisted: false
      },
      boundaries: {
        tenant_rollout_executed: false,
        ffa_promoted: false,
        fba_promoted: false,
        canonical_source_mutated: false
      },
      privacy: {
        storage_state_contents_persisted: false,
        authorization_or_cookie_values_persisted: false,
        session_ids_grants_or_proofs_persisted: false,
        page_ids_component_ids_or_edited_text_persisted: false,
        raw_html_persisted: false,
        raw_request_or_response_bodies_persisted: false,
        console_message_text_persisted: false,
        traces_persisted: false,
        screenshots_persisted: false,
        videos_persisted: false
      }
    };

    writeAtomic(output, outputDocument);
    await expiryPage.close();
    await stalePage.close();
    await mainPage.close();
  } finally {
    await Promise.all([
      editorContext.close(),
      unauthorizedContext.close(),
      standaloneContext.close()
    ]);
  }
});
