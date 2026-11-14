import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

const port = Number(process.env.RUSTOK_NEXT_ADMIN_E2E_PORT ?? 3200);
const baseURL = process.env.RUSTOK_NEXT_ADMIN_E2E_URL ?? `http://127.0.0.1:${port}`;
const leptosURL = process.env.RUSTOK_LEPTOS_ADMIN_URL ?? 'http://127.0.0.1:5150';

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

export default defineConfig({
  testDir: './tests/visual-regression',
  timeout: 30_000,
  expect: {
    timeout: 10_000,
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.05,
      animations: 'disabled'
    }
  },
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: 'playwright-report-visual-regression' }]
  ],
  use: {
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: executablePath ? { executablePath } : undefined,
    viewport: { width: 1280, height: 800 }
  },
  projects: [
    {
      name: 'visual-regression-chromium',
      use: { ...devices['Desktop Chrome'] }
    }
  ],
  outputDir: 'test-results-visual-regression'
});
