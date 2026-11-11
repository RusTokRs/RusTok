import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

const port = Number(process.env.RUSTOK_NEXT_ADMIN_E2E_PORT ?? 3200);
const baseURL =
  process.env.RUSTOK_NEXT_ADMIN_E2E_URL ?? `http://127.0.0.1:${port}`;
const smokePath =
  process.env.RUSTOK_NEXT_ADMIN_E2E_SMOKE_PATH ?? '/auth/sign-in';
const reuseExistingServer = process.env.CI !== 'true';

function resolveChromiumExecutable(): string | undefined {
  if (process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE) {
    return process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;
  }
  if (process.platform === 'win32') {
    const candidates = [
      'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
      'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
      'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe'
    ];
    for (const candidate of candidates) {
      if (existsSync(candidate)) return candidate;
    }
  }
  return undefined;
}

const executablePath = resolveChromiumExecutable();
const skipWebServer = process.env.PLAYWRIGHT_SKIP_WEBSERVER === 'true';
const recordVideo = process.env.PLAYWRIGHT_RECORD_VIDEO === 'true';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  expect: {
    timeout: process.env.CI === 'true' ? 15_000 : 5_000
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
    video: recordVideo ? 'retain-on-failure' : 'off',
    launchOptions: executablePath ? { executablePath } : undefined
  },
  webServer: skipWebServer
    ? undefined
    : {
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
