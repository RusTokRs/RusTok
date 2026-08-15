import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/pages-published-metadata',
  globalSetup: './tests/pages-published-metadata/global-setup.ts',
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
    screenshot: 'off',
    video: 'off'
  },
  projects: [
    {
      name: 'pages-published-metadata-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ],
  outputDir: 'test-results-pages-published-metadata'
});
