import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/pages-builder-rollout-matrix',
  testMatch: 'runtime-matrix.spec.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['line']],
  use: {
    headless: true,
    trace: 'off',
    screenshot: 'off',
    video: 'off'
  },
  projects: [
    {
      name: 'pages-builder-rollout-matrix-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ]
});
