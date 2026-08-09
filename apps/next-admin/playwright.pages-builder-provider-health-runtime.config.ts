import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/pages-builder-provider-health-runtime",
  testMatch: "runtime.spec.ts",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["line"]],
  use: {
    headless: true,
    trace: "off",
    screenshot: "off",
    video: "off",
  },
  projects: [
    {
      name: "pages-builder-provider-health-runtime-chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
