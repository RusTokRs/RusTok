import { defineConfig, devices } from '@playwright/test';

/**
 * Product admin evidence suite (audit runbook §19.4 / §19.5).
 *
 * Runs against an already-running development stack — see
 * product-audit-report-2026-11-17.md §19.3 (PostgreSQL + Axum server :5150 +
 * next-admin). There is intentionally no `webServer`: the product pages are
 * React server components whose GraphQL calls happen on the Next.js server,
 * so a backend-less Next instance cannot exercise the catalog chains.
 */
export default defineConfig({
  testDir: './tests/product-admin',
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 180_000,
  expect: {
    timeout: 15_000
  },
  reporter: [['list']],
  use: {
    trace: 'off',
    screenshot: 'only-on-failure',
    video: 'off'
  },
  projects: [
    {
      name: 'product-admin-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ],
  outputDir: 'test-results-product-admin'
});
