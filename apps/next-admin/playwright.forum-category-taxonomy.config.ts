import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/forum-category-taxonomy',
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
      name: 'forum-category-taxonomy-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ],
  outputDir: 'test-results-forum-category-taxonomy'
});
