import { defineConfig, devices } from '@playwright/test';

const port = Number(process.env.RUSTOK_NEXT_ADMIN_E2E_PORT ?? 3200);
const baseURL =
  process.env.RUSTOK_NEXT_ADMIN_E2E_URL ?? `http://127.0.0.1:${port}`;
const smokePath =
  process.env.RUSTOK_NEXT_ADMIN_E2E_SMOKE_PATH ?? '/auth/sign-in';
const reuseExistingServer = process.env.CI !== 'true';
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  expect: {
    timeout: 5_000
  },
  fullyParallel: true,
  retries: process.env.CI === 'true' ? 2 : 0,
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: 'playwright-report' }]
  ],
  use: {
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    launchOptions: executablePath ? { executablePath } : undefined
  },
  webServer: {
    command: `npm run dev -- --hostname 127.0.0.1 --port ${port}`,
    env: {
      AUTH_SECRET:
        process.env.AUTH_SECRET ??
        'rustok-next-admin-e2e-local-secret-do-not-use-in-production',
      NEXTAUTH_SECRET:
        process.env.NEXTAUTH_SECRET ??
        process.env.AUTH_SECRET ??
        'rustok-next-admin-e2e-local-secret-do-not-use-in-production'
    },
    url: `${baseURL}${smokePath}`,
    reuseExistingServer,
    timeout: 120_000
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ]
});
