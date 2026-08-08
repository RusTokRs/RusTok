import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/pages-builder-rollout-feature-preflight",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    trace: "off",
    screenshot: "off",
    video: "off",
  },
  projects: [
    {
      name: "pages-builder-rollout-feature-preflight-chromium",
      use: { browserName: "chromium" },
    },
  ],
});
