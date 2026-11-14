/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { defineConfig, devices } from '@playwright/test';

// A separate, slower smoke test: build and run the actual Next production host against a fake
// local API. It must not be confused with the real backend, deployment, or CSP acceptance.
function portFromEnv(name: string, fallback: number): number {
  const port = Number(process.env[name] || fallback);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error(`Invalid ${name}`);
  return port;
}
const apiPort = portFromEnv('FLY_HOST_API_PORT', 3311);
const hostPort = portFromEnv('FLY_HOST_NEXT_PORT', 3301);
const api = `http://127.0.0.1:${apiPort}`;
const host = `http://127.0.0.1:${hostPort}`;
const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;

export default defineConfig({
  testDir: './tests/fly-host',
  testMatch: 'next-fly-host.spec.ts',
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: 45_000,
  expect: { timeout: 10_000 },
  reporter: [['list']],
  use: { baseURL: host },
  projects: [{
    name: 'next-fly-host-chromium',
    use: {
      ...devices['Desktop Chrome'],
      launchOptions: executablePath ? { executablePath } : undefined,
    },
  }],
  webServer: [
    {
      command: 'node tests/fly-host/api.mjs',
      env: { FLY_HOST_API_PORT: String(apiPort) },
      url: `${api}/__state`,
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: `npm run build && npm run start -- --hostname 0.0.0.0 --port ${hostPort}`,
      env: {
        NEXT_TELEMETRY_DISABLED: '1',
        NEXT_PUBLIC_TENANT_SLUG: 'fly-host-test',
        NEXT_PUBLIC_API_URL: api,
        RUSTOK_API_URL: api,
      },
      url: `${host}/en/pages/smoke`,
      reuseExistingServer: false,
      timeout: 300_000,
    },
  ],
});
