import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/pages-inline-edit',
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 240_000,
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
      name: 'pages-inline-edit-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ],
  outputDir: 'test-results-pages-inline-edit'
});
