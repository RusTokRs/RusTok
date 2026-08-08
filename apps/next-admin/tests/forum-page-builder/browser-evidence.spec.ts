import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Page,
} from "@playwright/test";
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
  "crates/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json";
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), "utf8"),
) as BrowserContract;

const rootSelector = "[data-fly-browser-root='true']";
const topicListBlockSelector = "[data-fly-block-id='forum.topic_list']";
const propertiesPanelSelector =
  "[data-page-builder-contribution-properties='true']";
const previewPanelSelector = "[data-page-builder-contribution-preview='true']";
const contributionRegistrySelector = "[data-fly-contribution-registry='true']";
const topicListSchemaSelector =
  "[data-page-builder-contribution-property-schema='forum.topic_list.v1']";
const issuesSelector =
  "[data-page-builder-contribution-property-issues='true']";
const normalizedStatusText =
  "Owner-normalized properties applied to the Fly draft";
const categoryUuid = "550e8400-e29b-41d4-a716-446655440000";

type BrowserContract = {
  schema_version: number;
  module: string;
  packet: string;
  status: string;
  runner: string;
  config: string;
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
  noReadStorage: FileRecord;
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
  throw new Error(`Forum Page Builder browser evidence failed: ${message}`);
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

function requireCommit(value: string): string {
  if (!/^[0-9a-f]{40}$/u.test(value)) {
    fail("source commit must be a full lowercase Git SHA");
  }
  return value;
}

function requireDeploymentDigest(value: string): string {
  if (!/^[^@\s]+@sha256:[0-9a-f]{64}$/u.test(value)) {
    fail("deployment digest must be an immutable image RepoDigest");
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
    !["http:", "https:"].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.hash ||
    value.length > 4096
  ) {
    fail(`${label} must be a bounded credential-free HTTP(S) URL without a fragment`);
  }
  return parsed.toString();
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
  if (stats.size <= 0 || stats.size > 8 * 1024 * 1024) {
    fail(`${label} must be a bounded non-empty file`);
  }
  const bytes = readFileSync(absolute);
  return { path: absolute, bytes: stats.size, sha256: sha256(bytes) };
}

function currentCommit(): string {
  const value = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).trim();
  return requireCommit(value);
}

function sourceHashes(): Record<string, string> {
  if (
    !Array.isArray(contract.required_source_files) ||
    contract.required_source_files.length === 0
  ) {
    fail("browser execution contract has no required source files");
  }
  return Object.fromEntries(
    contract.required_source_files.map((relativePath) => {
      const record = regularFileRecord(
        relativePath,
        `source file ${relativePath}`,
      );
      return [relativePath, record.sha256];
    }),
  );
}

function outputPath(): string {
  const raw = optionalEnvironment(contract.output.environment, 16_384);
  const absolute = resolveInput(raw ?? contract.output.default_path);
  const targetRoot = path.resolve(repoRoot, "target");
  const relative = path.relative(targetRoot, absolute);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    fail("browser evidence output must remain inside repository target/");
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
  const document = JSON.parse(readFileSync(packagePath, "utf8")) as {
    version?: unknown;
  };
  if (typeof document.version !== "string" || document.version.length === 0) {
    fail("installed Playwright version is unavailable");
  }
  return document.version;
}

function loadInputs(): EvidenceInputs {
  const sourceCommit = requireCommit(
    requiredEnvironment(contract.environment.source_commit),
  );
  const head = currentCommit();
  if (sourceCommit !== head) {
    fail(`source commit ${sourceCommit} does not match checkout HEAD ${head}`);
  }

  const deploymentDigest = requireDeploymentDigest(
    requiredEnvironment(contract.environment.deployment_digest),
  );
  const editorStorage = regularFileRecord(
    requiredEnvironment(contract.environment.editor_storage_state),
    "editor storage state",
  );
  const noReadStorage = regularFileRecord(
    requiredEnvironment(contract.environment.no_read_storage_state),
    "no-read storage state",
  );
  const urlEnvironment = {
    full: contract.environment.full_url,
    preview_off: contract.environment.preview_off_url,
    properties_off: contract.environment.properties_off_url,
    forum_disabled: contract.environment.forum_disabled_url,
    no_read: contract.environment.no_read_url,
  };
  const urls = Object.fromEntries(
    Object.entries(urlEnvironment).map(([profile, environment]) => [
      profile,
      requireUrl(requiredEnvironment(environment), `${profile} profile URL`),
    ]),
  );
  return {
    sourceCommit,
    deploymentDigest,
    editorStorage,
    noReadStorage,
    urls,
    routeHashes: Object.fromEntries(
      Object.entries(urls).map(([profile, url]) => [profile, sha256(url)]),
    ),
    sourceHashes: sourceHashes(),
  };
}

async function openProfile(
  browser: Browser,
  url: string,
  storageState: string,
  label: string,
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
    criticalRequestFailures: 0,
  };
  page.on("console", (message) => {
    if (message.type() === "error") failures.consoleErrors += 1;
  });
  page.on("pageerror", () => {
    failures.pageErrors += 1;
  });
  page.on("requestfailed", (request) => {
    if (
      ["document", "script", "stylesheet", "fetch", "xhr"].includes(
        request.resourceType(),
      )
    ) {
      failures.criticalRequestFailures += 1;
    }
  });

  const response = await page.goto(url, { waitUntil: "domcontentloaded" });
  expect(response, `${label} navigation response`).not.toBeNull();
  expect(response?.status(), `${label} status`).toBeLessThan(400);
  const root = page.locator(rootSelector);
  await expect(root, `${label} Page Builder root`).toBeVisible();
  await expect(root).toHaveAttribute("data-fly-runtime", "ssr");
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
  label: string,
): void {
  expect(failures.consoleErrors, `${label} console errors`).toBe(0);
  expect(failures.pageErrors, `${label} page errors`).toBe(0);
  expect(
    failures.criticalRequestFailures,
    `${label} critical request failures`,
  ).toBe(0);
}

async function insertTopicList(page: Page): Promise<void> {
  const block = page.locator(topicListBlockSelector);
  await expect(block).toBeVisible();
  await expect(block).toHaveAttribute("data-fly-can-insert", "true");
  await block.locator("[data-fly-action='insert-block']").click();
  const layer = page
    .locator("[data-fly-action='select-component']")
    .filter({ hasText: "forum.topic_list" })
    .last();
  await expect(layer).toBeVisible();
  await layer.click();
}

async function loadTopicListSchema(page: Page): Promise<void> {
  const panel = page.locator(propertiesPanelSelector);
  await expect(panel).toBeVisible();
  const load = panel.getByRole("button", { name: "Load schema" });
  await expect(load).toBeEnabled();
  await load.click();
  await expect(panel.locator(topicListSchemaSelector)).toBeVisible();
}

async function applyProperties(page: Page): Promise<void> {
  const panel = page.locator(propertiesPanelSelector);
  const apply = panel.getByRole("button", {
    name: "Apply normalized properties",
  });
  await expect(apply).toBeEnabled();
  await apply.click();
}

test.describe.serial("Forum Page Builder retained browser evidence", () => {
  test.beforeAll(() => {
    expect(contract.status).toBe("source_ready_maintainer_execution_pending");
    expect(contract.profiles).toEqual([
      "full",
      "preview_off",
      "properties_off",
      "forum_disabled",
      "no_read",
    ]);
    inputs = loadInputs();
  });

  test.afterAll(() => {
    if (inputs === null) return;
    const requiredProfiles = contract.profiles;
    if (
      !requiredProfiles.every(
        (profile) => observations[profile]?.passed === true,
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
          sha256: inputs.editorStorage.sha256,
        },
        no_read_storage_state: {
          bytes: inputs.noReadStorage.bytes,
          sha256: inputs.noReadStorage.sha256,
        },
        profile_url_sha256: inputs.routeHashes,
      },
      observations,
      retained_secrets: false,
      browser_execution_only: true,
      runtime_authorization_evidence_pending: true,
      observed_page_builder_wave_pending: true,
      executed_at: new Date().toISOString(),
    });
  });

  test("full profile edits owner-normalized props and preserves Fly history", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.full,
      inputs.editorStorage.path,
      "full profile",
    );
    try {
      await expect(page.locator(contributionRegistrySelector)).toBeVisible();
      await insertTopicList(page);
      await loadTopicListSchema(page);

      const panel = page.locator(propertiesPanelSelector);
      const perPage = panel.locator("#fly-owner-property-per_page");
      const categoryId = panel.locator("#fly-owner-property-category_id");
      await expect(perPage).toHaveValue("20");

      await perPage.fill("101");
      await applyProperties(page);
      await expect(panel.locator(issuesSelector)).toContainText("per_page");
      await expect(
        panel.getByText("Owner validation rejected the current widget properties"),
      ).toBeVisible();

      await perPage.fill("10");
      await categoryId.fill(` ${categoryUuid} `);
      await applyProperties(page);
      await expect(panel.getByText(normalizedStatusText)).toBeVisible();
      await expect(panel.locator(issuesSelector)).toContainText("category_id");

      await loadTopicListSchema(page);
      await expect(perPage).toHaveValue("10");
      await expect(categoryId).toHaveValue(categoryUuid);

      const undo = page.locator("[data-fly-action='intent:undo']");
      const redo = page.locator("[data-fly-action='intent:redo']");
      await expect(undo).toBeEnabled();
      await undo.click();
      await loadTopicListSchema(page);
      await expect(perPage).toHaveValue("20");
      await expect(categoryId).toHaveValue("");

      await expect(redo).toBeEnabled();
      await redo.click();
      await loadTopicListSchema(page);
      await expect(perPage).toHaveValue("10");
      await expect(categoryId).toHaveValue(categoryUuid);

      const preview = page.locator(previewPanelSelector);
      await expect(preview).toBeVisible();
      const refresh = preview.getByRole("button", { name: "Refresh" });
      await expect(refresh).toBeEnabled();
      await refresh.click();
      await expect(
        preview.locator(
          "[data-page-builder-contribution-preview-result='ready']",
        ),
      ).toBeVisible();

      const save = page.locator("[data-fly-action='intent:save']");
      await expect(save).toBeEnabled();
      await save.click();
      await expect(save).toBeDisabled();
      assertNoCriticalFailures(failures, "full profile");
      observations.full = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          topic_list_admitted: true,
          invalid_owner_props_rejected: true,
          owner_normalization_observed: true,
          fly_undo_observed: true,
          fly_redo_observed: true,
          owner_preview_ready: true,
          pages_save_completed: true,
        },
      };
    } finally {
      await context.close();
    }
  });

  test("preview_off keeps authoring and owner properties but removes preview admission", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.preview_off,
      inputs.editorStorage.path,
      "preview_off profile",
    );
    try {
      await insertTopicList(page);
      await loadTopicListSchema(page);
      const preview = page.locator(previewPanelSelector);
      await expect(preview).toBeVisible();
      await expect(preview.getByRole("button", { name: "Refresh" })).toBeDisabled();
      await expect(
        preview.locator(
          "[data-page-builder-contribution-preview-provider='rustok.forum']",
        ),
      ).toHaveCount(0);
      assertNoCriticalFailures(failures, "preview_off profile");
      observations.preview_off = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          topic_list_admitted: true,
          owner_properties_actionable: true,
          owner_preview_not_admitted: true,
        },
      };
    } finally {
      await context.close();
    }
  });

  test("properties_off removes Forum authoring/property admission", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.properties_off,
      inputs.editorStorage.path,
      "properties_off profile",
    );
    try {
      await expect(page.locator(topicListBlockSelector)).toHaveCount(0);
      const panel = page.locator(propertiesPanelSelector);
      await expect(panel).toBeVisible();
      await expect(panel.getByRole("button", { name: "Load schema" })).toBeDisabled();
      await expect(
        panel.locator(
          "[data-page-builder-contribution-property-provider='rustok.forum']",
        ),
      ).toHaveCount(0);
      assertNoCriticalFailures(failures, "properties_off profile");
      observations.properties_off = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          topic_list_not_admitted: true,
          owner_properties_not_actionable: true,
        },
      };
    } finally {
      await context.close();
    }
  });

  test("Forum-disabled tenant exposes no Forum contribution extension", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.forum_disabled,
      inputs.editorStorage.path,
      "Forum-disabled profile",
    );
    try {
      await expect(page.locator(topicListBlockSelector)).toHaveCount(0);
      await expect(page.locator(propertiesPanelSelector)).toHaveCount(0);
      await expect(page.locator(previewPanelSelector)).toHaveCount(0);
      assertNoCriticalFailures(failures, "Forum-disabled profile");
      observations.forum_disabled = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          topic_list_absent: true,
          owner_property_panel_absent: true,
          owner_preview_panel_absent: true,
        },
      };
    } finally {
      await context.close();
    }
  });

  test("missing forum_topics:read prevents Forum contribution admission", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.no_read,
      inputs.noReadStorage.path,
      "no-read profile",
    );
    try {
      await expect(page.locator(topicListBlockSelector)).toHaveCount(0);
      const panel = page.locator(propertiesPanelSelector);
      if ((await panel.count()) > 0) {
        await expect(panel.getByRole("button", { name: "Load schema" })).toBeDisabled();
        await expect(
          panel.locator(
            "[data-page-builder-contribution-property-provider='rustok.forum']",
          ),
        ).toHaveCount(0);
      }
      assertNoCriticalFailures(failures, "no-read profile");
      observations.no_read = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          topic_list_not_admitted: true,
          owner_properties_not_actionable: true,
        },
      };
    } finally {
      await context.close();
    }
  });
});
