import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/rust-hosts",
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  reporter: [
    ["list"],
    ["html", { outputFolder: "playwright-report-rust-hosts", open: "never" }],
  ],
  use: {
    baseURL: process.env.RUSTOK_BROWSER_BASE_URL || "http://127.0.0.1:5150",
    extraHTTPHeaders: {
      "x-tenant-slug": process.env.RUSTOK_BROWSER_TENANT_SLUG || "default",
    },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "rust-hosted-chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  outputDir: "test-results-rust-hosts",
});
