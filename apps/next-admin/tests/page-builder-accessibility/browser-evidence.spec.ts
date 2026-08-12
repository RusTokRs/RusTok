import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Locator,
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

const repoRoot = path.resolve(process.cwd(), "../..");
const contractPath =
  "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json";
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, contractPath), "utf8"),
) as BrowserContract;

const rootSelector = "[data-fly-browser-root='true']";
const editFieldsetSelector = "fieldset[data-fly-capability='edit']";
const propertiesFieldsetSelector = "fieldset[data-fly-capability='properties']";

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
  fixture_requirements: {
    minimum_page_count: number;
  };
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
  throw new Error(`Page Builder accessibility browser evidence failed: ${message}`);
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
  return requireCommit(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
  );
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
  const urls = {
    full: requireUrl(
      requiredEnvironment(contract.environment.full_url),
      "full profile URL",
    ),
    read_only: requireUrl(
      requiredEnvironment(contract.environment.read_only_url),
      "read-only profile URL",
    ),
  };
  return {
    sourceCommit,
    deploymentDigest,
    editorStorage,
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

function pageManager(page: Page): Locator {
  return page.locator("section").filter({
    has: page.getByRole("heading", { name: "Pages", exact: true }),
  });
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

async function activePageIndex(buttons: Locator, count: number): Promise<number> {
  let active = -1;
  for (let index = 0; index < count; index += 1) {
    if ((await buttons.nth(index).getAttribute("aria-pressed")) === "true") {
      if (active !== -1) fail("multiple page buttons expose aria-pressed=true");
      active = index;
    }
  }
  if (active === -1) fail("no active page button exposes aria-pressed=true");
  return active;
}

async function provePageKeyboardSelection(
  page: Page,
  buttons: Locator,
  count: number,
): Promise<void> {
  const active = await activePageIndex(buttons, count);
  const target = active + 1 < count ? active + 1 : active - 1;
  const forward = target > active;
  const activeButton = buttons.nth(active);
  const targetButton = buttons.nth(target);

  await expect(activeButton).toMatchAriaSnapshot(`- button [pressed=true]`);
  await activeButton.focus();
  await expect(activeButton).toBeFocused();
  await page.keyboard.press(forward ? "Tab" : "Shift+Tab");
  await expect(targetButton).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(targetButton).toHaveAttribute("aria-pressed", "true");
  await expect(activeButton).toHaveAttribute("aria-pressed", "false");
  await expect(targetButton).toMatchAriaSnapshot(`- button [pressed=true]`);
}

test.describe.serial("Page Builder generic accessibility browser evidence", () => {
  test.beforeAll(() => {
    expect(contract.status).toBe("source_ready_maintainer_execution_pending");
    expect(contract.profiles).toEqual(["full", "read_only"]);
    expect(contract.fixture_requirements.minimum_page_count).toBeGreaterThanOrEqual(2);
    inputs = loadInputs();
  });

  test.afterAll(() => {
    if (inputs === null) return;
    if (!contract.profiles.every((profile) => observations[profile]?.passed === true)) {
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
        profile_url_sha256: inputs.routeHashes,
      },
      observations,
      retained_secrets: false,
      raw_dom_retained: false,
      aria_snapshot_text_retained: false,
      screen_reader_execution_pending: true,
      wcag_conformance_not_claimed: true,
      executed_at: new Date().toISOString(),
    });
  });

  test("full profile exposes keyboard focus, activation and accessibility-tree state", async ({
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
      const panel = pageManager(page);
      await expect(panel).toBeVisible();
      const buttons = panel.locator("button[aria-pressed]");
      const pageCount = await buttons.count();
      expect(pageCount).toBeGreaterThanOrEqual(
        contract.fixture_requirements.minimum_page_count,
      );

      await provePageKeyboardSelection(page, buttons, pageCount);

      const addPageName = panel.getByRole("textbox", {
        name: "Add page: Page name",
        exact: true,
      });
      await expect(addPageName).toBeVisible();
      await expect(addPageName).toMatchAriaSnapshot(
        `- textbox "Add page: Page name"`,
      );
      const addPageButton = panel.getByRole("button", {
        name: "Add page",
        exact: true,
      });
      await expect(addPageButton).toBeEnabled();
      await addPageName.focus();
      await expect(addPageName).toBeFocused();
      await page.keyboard.press("Tab");
      await expect(addPageButton).toBeFocused();
      await page.keyboard.press("Shift+Tab");
      await expect(addPageName).toBeFocused();

      await expect(
        panel.getByRole("textbox", { name: "Page name", exact: true }),
      ).toBeVisible();
      await expect(
        panel.getByRole("textbox", { name: "Page id", exact: true }),
      ).toBeVisible();

      assertNoCriticalFailures(failures, "full profile");
      observations.full = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          pageCount,
          tabFocusBetweenAdjacentPages: true,
          keyboardActivationUpdatedPressedState: true,
          addPageSequentialFocusOrder: true,
          ariaTreePressedStateObserved: true,
          ariaTreeAddPageNameObserved: true,
          pageMetadataAccessibleNamesResolved: true,
        },
      };
    } finally {
      await context.close();
    }
  });

  test("read-only profile keeps navigation keyboard-operable and mutation controls browser-disabled", async ({
    browser,
  }) => {
    if (inputs === null) fail("browser inputs were not initialized");
    const { context, page, failures } = await openProfile(
      browser,
      inputs.urls.read_only,
      inputs.editorStorage.path,
      "read-only profile",
    );
    try {
      const editFieldset = page.locator(editFieldsetSelector);
      const propertiesFieldset = page.locator(propertiesFieldsetSelector);
      await expect(editFieldset).toBeDisabled();
      await expect(editFieldset).toHaveAttribute("aria-disabled", "true");
      await expect(propertiesFieldset).toBeDisabled();
      await expect(propertiesFieldset).toHaveAttribute("aria-disabled", "true");

      const panel = pageManager(page);
      const buttons = panel.locator("button[aria-pressed]");
      const pageCount = await buttons.count();
      expect(pageCount).toBeGreaterThanOrEqual(
        contract.fixture_requirements.minimum_page_count,
      );
      await provePageKeyboardSelection(page, buttons, pageCount);

      const addPageName = panel.getByRole("textbox", {
        name: "Add page: Page name",
        exact: true,
      });
      const addPageButton = panel.getByRole("button", {
        name: "Add page",
        exact: true,
      });
      await expect(addPageName).toBeDisabled();
      await expect(addPageButton).toBeDisabled();
      await expect(
        panel.getByRole("textbox", { name: "Page name", exact: true }),
      ).toBeDisabled();
      await expect(
        panel.getByRole("textbox", { name: "Page id", exact: true }),
      ).toBeDisabled();

      assertNoCriticalFailures(failures, "read-only profile");
      observations.read_only = {
        passed: true,
        criticalFailures: criticalFailureCount(failures),
        facts: {
          pageCount,
          editFieldsetBrowserDisabled: true,
          editFieldsetAriaDisabled: true,
          propertiesFieldsetBrowserDisabled: true,
          propertiesFieldsetAriaDisabled: true,
          mutationControlsBrowserDisabled: true,
          pageNavigationKeyboardAvailable: true,
        },
      };
    } finally {
      await context.close();
    }
  });
});
