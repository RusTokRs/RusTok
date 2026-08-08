import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/forum-page-builder",
  globalSetup: "./tests/forum-page-builder/global-setup.ts",
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 240_000,
  expect: {
    timeout: 15_000,
  },
  reporter: [["list"]],
  use: {
    trace: "off",
    screenshot: "off",
    video: "off",
  },
  projects: [
    {
      name: "forum-page-builder-chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  outputDir: "test-results-forum-page-builder",
});
