import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Page
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
  'crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json';
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), 'utf8')
) as BrowserContract;

const surfaceSelector = "[data-pages-published-metadata-surface='registered']";
const errorSurfaceSelector = "[data-pages-published-metadata-surface='error']";
const panelSelector = "[data-fly-consumer-properties='ready']";

type BrowserContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  runner: string;
  config: string;
  global_setup: string;
  output: {
    environment: string;
    default_path: string;
    format: string;
    status: string;
  };
  environment: Record<string, string>;
  profiles: string[];
  required_source_files: string[];
};

type FileRecord = {
  path: string;
  bytes: number;
  sha256: string;
};

type BrowserFailures = {
  consoleErrors: number;
  pageErrors: number;
  criticalRequestFailures: number;
};

type EvidenceInputs = {
  sourceCommit: string;
  deploymentDigest: string;
  editorStorage: FileRecord;
  urls: Record<string, string>;
  routeHashes: Record<string, string>;
  sourceHashes: Record<string, string>;
};

type ScenarioObservation = {
  passed: boolean;
  criticalFailures: number;
  facts: Record<string, boolean | number>;
};

const observations: Record<string, ScenarioObservation> = {};
let inputs: EvidenceInputs | null = null;

function fail(message: string): never {
  throw new Error(
    `Pages published metadata browser evidence failed: ${message}`
  );
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

function requireCommit(value: string): string {
  if (!/^[0-9a-f]{40}$/u.test(value)) {
    fail('source commit must be a full lowercase Git SHA');
  }
  return value;
}

function requireDeploymentDigest(value: string): string {
  if (!/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(value)) {
    fail('deployment digest must be an immutable image RepoDigest');
  }
  return value;
}

function requireUrl(value: string, label: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    fail(`${label} must be an absolute HTTP(S) URL`);
  }
  if (
    !['http:', 'https:'].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.hash ||
    value.length > 4096
  ) {
    fail(
      `${label} must be a bounded credential-free HTTP(S) URL without a fragment`
    );
  }
  return parsed.toString();
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
  if (stats.size <= 0 || stats.size > 8 * 1024 * 1024) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { path: absolute, bytes: stats.size, sha256: sha256(bytes) };
}

function currentCommit(): string {
  const value = execFileSync('git', ['rev-parse', 'HEAD'], {
    cwd: repoRoot,
    encoding: 'utf8'
  }).trim();
  return requireCommit(value);
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

function loadInputs(): EvidenceInputs {
  const sourceCommit = requireCommit(
    requiredEnvironment(contract.environment.source_commit)
  );
  const head = currentCommit();
  if (sourceCommit !== head) {
    fail(`source commit ${sourceCommit} does not match checkout HEAD ${head}`);
  }

  const deploymentDigest = requireDeploymentDigest(
    requiredEnvironment(contract.environment.deployment_digest)
  );
  const editorStorage = regularFileRecord(
    requiredEnvironment(contract.environment.editor_storage_state),
    'editor storage state'
  );
  const urlEnvironment = {
    published: contract.environment.published_url,
    draft: contract.environment.draft_url,
    archived: contract.environment.archived_url,
    missing: contract.environment.missing_url
  };
  const urls = Object.fromEntries(
    Object.entries(urlEnvironment).map(([profile, environment]) => [
      profile,
      requireUrl(requiredEnvironment(environment), `${profile} profile URL`)
    ])
  );

  return {
    sourceCommit,
    deploymentDigest,
    editorStorage,
    urls,
    routeHashes: Object.fromEntries(
      Object.entries(urls).map(([profile, url]) => [profile, sha256(url)])
    ),
    sourceHashes: sourceHashes()
  };
}

async function openProfile(
  browser: Browser,
  url: string,
  storageState: string,
  label: string
): Promise<{
  context: BrowserContext;
  page: Page;
  failures: BrowserFailures;
}> {
  const context = await browser.newContext({ storageState });
  const page = await context.newPage();
  const failures: BrowserFailures = {
    consoleErrors: 0,
    pageErrors: 0,
    criticalRequestFailures: 0
  };
  page.on('console', (message) => {
    if (message.type() === 'error') failures.consoleErrors += 1;
  });
  page.on('pageerror', () => {
    failures.pageErrors += 1;
  });
  page.on('requestfailed', (request) => {
    if (
      ['document', 'script', 'stylesheet', 'fetch', 'xhr'].includes(
        request.resourceType()
      )
    ) {
      failures.criticalRequestFailures += 1;
    }
  });

  const response = await page.goto(url, { waitUntil: 'domcontentloaded' });
  expect(response, `${label} navigation response`).not.toBeNull();
  expect(response?.status(), `${label} status`).toBeLessThan(400);
  return { context, page, failures };
}

function criticalFailureCount(failures: BrowserFailures): number {
  return (
    failures.consoleErrors +
    failures.pageErrors +
    failures.criticalRequestFailures
  );
}

function assertNoCriticalFailures(
  failures: BrowserFailures,
  label: string
): void {
  expect(failures.consoleErrors, `${label} console errors`).toBe(0);
  expect(failures.pageErrors, `${label} page errors`).toBe(0);
  expect(
    failures.criticalRequestFailures,
    `${label} critical request failures`
  ).toBe(0);
}

async function assertHiddenProfile(
  browser: Browser,
  profile: 'draft' | 'archived' | 'missing'
): Promise<void> {
  if (inputs === null) fail('browser inputs were not initialized');
  const { context, page, failures } = await openProfile(
    browser,
    inputs.urls[profile],
    inputs.editorStorage.path,
    `${profile} profile`
  );
  try {
    await expect(page.locator(surfaceSelector)).toHaveCount(0);
    await expect(page.locator(errorSurfaceSelector)).toHaveCount(0);
    assertNoCriticalFailures(failures, `${profile} profile`);
    observations[profile] = {
      passed: true,
      criticalFailures: criticalFailureCount(failures),
      facts: {
        registered_published_surface_absent: true,
        metadata_surface_error_absent: true
      }
    };
  } finally {
    await context.close();
  }
}

test.describe
  .serial('Pages published metadata retained browser evidence', () => {
  test.beforeAll(() => {
    expect(contract.schema_version).toBe(1);
    expect(contract.module).toBe('pages');
    expect(contract.packet).toBe('published_metadata_surface_browser_evidence');
    expect(contract.status).toBe('source_ready_maintainer_execution_pending');
    expect(contract.profiles).toEqual([
      'published',
      'draft',
      'archived',
      'missing'
    ]);
    inputs = loadInputs();
  });

  test.afterAll(() => {
    if (inputs === null) return;
    if (
      !contract.profiles.every(
        (profile) => observations[profile]?.passed === true
      )
    ) {
      return;
    }
    writeAtomic(outputPath(), {
      format: contract.output.format,
      status: contract.output.status,
      source_commit: inputs.sourceCommit,
      deployment_digest: inputs.deploymentDigest,
      node_version: process.version,
      playwright_version: playwrightVersion(),
      source_files: inputs.sourceHashes,
      input_records: {
        editor_storage_state: {
          bytes: inputs.editorStorage.bytes,
          sha256: inputs.editorStorage.sha256
        },
        profile_url_sha256: inputs.routeHashes
      },
      observations,
      retained_secrets: false,
      metadata_values_retained: false,
      browser_execution_only: true,
      consumer_properties_admission_pending: true,
      executed_at: new Date().toISOString()
    });
  });

  test('published page exposes registered metadata without Fly authoring', async ({
    browser
  }) => {
    if (inputs === null) fail('browser inputs were not initialized');
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.published,
      inputs.editorStorage.path,
      'published profile'
    );
    try {
      const surface = page.locator(surfaceSelector);
      await expect(surface).toBeVisible();
      await expect(surface).toHaveAttribute(
        'data-pages-published-metadata-admission',
        'published-only'
      );
      await expect(surface).toHaveAttribute(
        'data-pages-fly-canvas-mounted',
        'false'
      );
      await expect(surface).toHaveAttribute(
        'data-pages-document-authoring',
        'false'
      );
      await expect(surface).toHaveAttribute(
        'data-pages-metadata-runtime',
        'registered'
      );
      await expect(surface).toHaveAttribute(
        'data-pages-metadata-persistence',
        'owner-port'
      );

      const panel = surface.locator(panelSelector);
      await expect(panel).toBeVisible();
      await expect(panel).toHaveAttribute(
        'data-fly-consumer-property-editor',
        'rustok.pages.metadata.editor'
      );
      await expect(panel.locator('#fly-consumer-property-title')).toBeVisible();
      await expect(panel.locator('#fly-consumer-property-slug')).toBeVisible();
      await expect(
        panel.getByRole('button', { name: 'Save properties' })
      ).toBeEnabled();
      await expect(page.locator(errorSurfaceSelector)).toHaveCount(0);
      assertNoCriticalFailures(failures, 'published profile');

      observations.published = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          registered_surface_visible: true,
          published_only_admission: true,
          fly_canvas_unmounted: true,
          document_authoring_unmounted: true,
          registered_runtime_present: true,
          owner_port_persistence_declared: true,
          registered_property_panel_ready: true,
          save_action_available_without_mutation: true
        }
      };
    } finally {
      await context.close();
    }
  });

  test('draft page hides the published-only registered surface', async ({
    browser
  }) => {
    await assertHiddenProfile(browser, 'draft');
  });

  test('archived page hides the published-only registered surface', async ({
    browser
  }) => {
    await assertHiddenProfile(browser, 'archived');
  });

  test('missing selection hides the published-only registered surface', async ({
    browser
  }) => {
    await assertHiddenProfile(browser, 'missing');
  });
});
