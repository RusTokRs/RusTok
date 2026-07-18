import { expect, test, type APIResponse, type Page } from "@playwright/test";

const STRICT_STYLE_ATTRIBUTE_DIRECTIVE = "style-src-attr 'none'";
const STRICT_SCRIPT_ATTRIBUTE_DIRECTIVE = "script-src-attr 'none'";

function expectStrictUiCsp(response: APIResponse): void {
  const csp = response.headers()["content-security-policy"];
  expect(csp, "UI response must include an enforced CSP").toBeTruthy();
  expect(csp).toContain(STRICT_SCRIPT_ATTRIBUTE_DIRECTIVE);
  expect(csp).toContain(STRICT_STYLE_ATTRIBUTE_DIRECTIVE);
  expect(csp).not.toContain("'unsafe-eval'");
  expect(csp).not.toContain("style-src-attr 'unsafe-inline'");
  expect(csp).not.toMatch(/(?:^|;)\s*style-src\s+[^;]*'unsafe-inline'/);
}

function expectNonceBackedElements(
  html: string,
  tagName: "script" | "style",
  minimum: number,
): void {
  const tags = [...html.matchAll(new RegExp(`<${tagName}\\b[^>]*>`, "gi"))].map(
    (match) => match[0],
  );
  expect(tags.length, `expected at least ${minimum} <${tagName}> element(s)`).toBeGreaterThanOrEqual(
    minimum,
  );
  for (const tag of tags) {
    expect(tag, `${tagName} element must carry the response nonce`).toMatch(
      /\snonce="[A-Za-z0-9_-]+"/,
    );
  }
}

async function observeCriticalFailures(page: Page): Promise<string[]> {
  const failures: string[] = [];
  page.on("requestfailed", (request) => {
    const url = request.url();
    const resourceType = request.resourceType();
    const critical =
      resourceType === "document" ||
      resourceType === "script" ||
      resourceType === "stylesheet" ||
      url.endsWith(".wasm");
    if (critical) {
      failures.push(`${resourceType} ${url}: ${request.failure()?.errorText || "unknown failure"}`);
    }
  });
  return failures;
}

test("health endpoint reports the running monolith", async ({ request }) => {
  const response = await request.get("/health");
  expect(response.status()).toBe(200);
  expect(await response.json()).toMatchObject({ status: "ok", app: "rustok" });
  expect(response.headers()["content-security-policy"]).toContain("default-src 'none'");
});

test("server-hosted storefront renders under strict CSP", async ({ request, page }) => {
  const response = await request.get("/");
  expect(response.status()).toBe(200);
  expectStrictUiCsp(response);
  const html = await response.text();
  expect(html).toContain("<title>RusToK Storefront</title>");
  expect(html).not.toMatch(/\sstyle\s*=/i);

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  const failures = await observeCriticalFailures(page);
  const navigation = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(navigation?.status()).toBe(200);
  await expect(page).toHaveTitle("RusToK Storefront");
  await page.waitForTimeout(1_000);
  expect(pageErrors).toEqual([]);
  expect(failures).toEqual([]);
});

test("embedded admin assets hydrate from the admin mount under strict CSP", async ({ request, page }) => {
  const response = await request.get("/admin/");
  expect(response.status()).toBe(200);
  expectStrictUiCsp(response);
  const html = await response.text();
  expect(html).toContain("<title>RusToK Admin</title>");
  expect(html).not.toMatch(/\sstyle\s*=/i);
  expect(html).not.toMatch(/(?:src|href)="\/(?:rustok-admin|snippets|output\.css)/i);
  expect(html).toContain("/admin/");
  expectNonceBackedElements(html, "script", 1);
  expectNonceBackedElements(html, "style", 0);

  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  const failures = await observeCriticalFailures(page);
  const navigation = await page.goto("/admin/", { waitUntil: "domcontentloaded" });
  expect(navigation?.status()).toBe(200);
  await expect(page).toHaveTitle("RusToK Admin");
  await page.waitForTimeout(2_000);
  expect(pageErrors).toEqual([]);
  expect(failures).toEqual([]);
});
