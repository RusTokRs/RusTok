import { defineConfig, devices } from '@playwright/test';

// These contract tests mount the shipped browser assets and React components with fake host
// boundaries. They do not require the Next dev server or claim deployed-host coverage.
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: 'fly-*.spec.ts',
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: [['list']],
  projects: [{
    name: 'fly-chromium',
    use: {
      ...devices['Desktop Chrome'],
      launchOptions: executablePath ? { executablePath } : undefined,
    },
  }],
});
