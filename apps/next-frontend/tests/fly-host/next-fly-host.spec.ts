/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

import { expect, test } from '@playwright/test';

// Unlike the component harness, this exercises Next's production bundle, actual routes, React
// hydration and router. Only the API is fake; it says nothing about deployed backend policy/CSP.
const api = `http://127.0.0.1:${Number(process.env.FLY_HOST_API_PORT || 3311)}`;
const variantId = '11111111-1111-4111-8111-111111111111';
type MockRequest = {
  op: string;
  variables: { input?: { variantId?: string; quantity?: number }; quantity?: number; currencyCode?: string } | null;
  tenant: string | null;
};
type MockState = { requests: MockRequest[] };

test('production Next host hydrates a landing and hands purchase to Product for confirmation', async ({ page, request, baseURL }) => {
  expect((await request.post(`${api}/__reset`)).ok()).toBe(true);
  const readState = async () => (await (await request.get(`${api}/__state`)).json()) as MockState;
  const errors: string[] = [];
  const browserGraphqlOrigins: string[] = [];
  page.on('pageerror', (error) => errors.push(error.stack || error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('request', (browserRequest) => {
    const url = new URL(browserRequest.url());
    if (url.pathname === '/api/graphql') browserGraphqlOrigins.push(url.origin);
  });

  const response = await request.get(`${baseURL}/en/pages/smoke`);
  expect(response.status()).toBe(200);
  expect(await response.text()).toContain('data-testid="real-landing"');

  await page.goto('/en/pages/smoke', { waitUntil: 'domcontentloaded' });
  // The attribute is written by the mounted bridge, so this also waits for hydration.
  await expect(page.getByTestId('real-count')).toHaveAttribute('aria-busy', 'false');
  await expect(page.getByTestId('real-count')).toHaveText('0');
  const panel = page.getByTestId('real-panel');
  await expect(panel).toHaveCSS('color', 'rgb(1, 2, 3)');
  await page.getByTestId('real-toggle').click();
  await expect(panel).toBeHidden();
  await page.getByTestId('real-toggle').click();
  await expect(panel).toBeVisible();

  await page.getByTestId('real-cart').click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByTestId('real-buy').click();
  await page.waitForURL(/\/en\/products\/cup\?/);
  const query = new URL(page.url()).searchParams;
  expect(query.get('variant_id')).toBe(variantId);
  expect(query.get('quantity')).toBe('2');
  expect(query.get('currency')).toBe('AED');
  await expect(page.getByRole('heading', { name: 'Smoke Cup' })).toBeVisible();
  await expect(page.getByText('10.00 AED', { exact: true })).toBeVisible();
  const buy = page.getByRole('button', { name: 'Add to Cart', exact: true });
  await expect(buy).toBeEnabled();

  const before = await readState();
  expect(before.requests.some(({ op }) => op === 'CreateStorefrontCart' || op === 'AddStorefrontCartLineItem')).toBe(false);
  expect(before.requests.some(({ op, variables }) => op === 'StorefrontProductPricing' &&
    variables?.quantity === 2 && variables.currencyCode === 'AED')).toBe(true);
  await buy.click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await expect(page.getByRole('dialog').getByText('Cup', { exact: true })).toBeVisible();
  const after = await readState();
  expect(after.requests.filter(({ op }) => op === 'CreateStorefrontCart')).toHaveLength(1);
  const adds = after.requests.filter(({ op }) => op === 'AddStorefrontCartLineItem');
  expect(adds).toHaveLength(1);
  expect(adds[0].variables?.input).toMatchObject({ variantId, quantity: 2 });
  expect(adds[0].tenant).toBe('fly-host-test');
  expect(browserGraphqlOrigins.length).toBeGreaterThan(0);
  expect(browserGraphqlOrigins.every((origin) => origin === baseURL)).toBe(true);

  await page.keyboard.press('Escape');
  await page.goBack({ waitUntil: 'domcontentloaded' });
  await page.waitForURL(/\/en\/pages\/smoke$/);
  await expect(page.getByTestId('real-count')).toHaveText('2');
  expect(errors).toEqual([]);
});
