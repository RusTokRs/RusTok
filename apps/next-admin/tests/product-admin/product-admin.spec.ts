/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */
import { expect, test, type Browser, type Page } from '@playwright/test';

/**
 * Product admin browser evidence (matrix §19.4 of the audit report).
 *
 * Runs against a live development stack (audit runbook §19.3):
 * PostgreSQL + Axum server :5150 + next-admin. The suite follows the
 * forum-category-taxonomy conventions: no webServer in the config, every
 * external input arrives through bounded, validated environment variables.
 */
function requiredEnvironment(name: string, maximumLength = 4096): string {
  const value = process.env[name];
  if (
    typeof value !== 'string' ||
    value.trim().length === 0 ||
    value.length > maximumLength ||
    /[\u0000\r\n]/u.test(value)
  ) {
    throw new Error(`${name} must be a bounded non-empty environment value`);
  }
  return value.trim();
}

function requiredUrl(name: string): string {
  const value = requiredEnvironment(name);
  const parsed = new URL(value);
  if (
    !['http:', 'https:'].includes(parsed.protocol) ||
    parsed.username ||
    parsed.password ||
    parsed.hash
  ) {
    throw new Error(
      `${name} must be a credential-free HTTP(S) URL without a fragment`
    );
  }
  return parsed.toString().replace(/\/$/u, '');
}

function requiredUuid(name: string): string {
  const value = requiredEnvironment(name, 36);
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu.test(
      value
    )
  ) {
    throw new Error(`${name} must be a canonical UUID`);
  }
  return value;
}

function optionalEnvironment(name: string): string | undefined {
  const value = process.env[name];
  if (typeof value !== 'string' || value.trim().length === 0) {
    return undefined;
  }
  return value.trim();
}

const baseUrl = requiredUrl('RUSTOK_PRODUCT_ADMIN_BASE_URL');
const storageStatePath = requiredEnvironment(
  'RUSTOK_PRODUCT_ADMIN_STORAGE_STATE',
  1024
);
const seededProductId = requiredUuid('RUSTOK_PRODUCT_ADMIN_SEEDED_PRODUCT_ID');
const seededTitle = requiredEnvironment(
  'RUSTOK_PRODUCT_ADMIN_SEEDED_TITLE',
  512
);
const seededSku = optionalEnvironment('RUSTOK_PRODUCT_ADMIN_SEEDED_SKU');

async function authenticatedAdminPage(
  browser: Browser
): Promise<{ page: Page; close: () => Promise<void> }> {
  const context = await browser.newContext({ storageState: storageStatePath });
  const page = await context.newPage();
  return {
    page,
    close: async () => {
      await context.close();
    }
  };
}

test('unauthenticated catalog access redirects to sign-in', async ({
  page
}) => {
  await page.goto(`${baseUrl}/dashboard/product`);
  await expect(page).toHaveURL(/\/auth\/sign-in/u);
});

test('catalog list renders the seeded product row', async ({ browser }) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const response = await page.goto(`${baseUrl}/dashboard/product`);
    expect(response, 'catalog list must respond').not.toBeNull();
    expect(
      response!.status(),
      'catalog list must not return an HTTP error'
    ).toBeLessThan(400);
    await expect(
      page.getByRole('columnheader', { name: 'Product' })
    ).toBeVisible();
    await expect(
      page.getByText(seededTitle, { exact: false }).first()
    ).toBeVisible();
  } finally {
    await close();
  }
});

test('catalog title search narrows the product table', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const fragment = seededTitle.slice(0, Math.min(4, seededTitle.length));
    const missTerm = `${fragment}-no-such-catalog-title`;

    await page.goto(
      `${baseUrl}/dashboard/product?title=${encodeURIComponent(fragment)}`
    );
    await expect(
      page.getByText(seededTitle, { exact: false }).first()
    ).toBeVisible();

    await page.goto(
      `${baseUrl}/dashboard/product?title=${encodeURIComponent(missTerm)}`
    );
    await expect(
      page.getByText(seededTitle, { exact: false })
    ).toHaveCount(0);
  } finally {
    await close();
  }
});

test('product editor renders pricing, variants, and media sections', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const response = await page.goto(
      `${baseUrl}/dashboard/product/${seededProductId}`
    );
    expect(response, 'product editor must respond').not.toBeNull();
    expect(
      response!.status(),
      'product editor must not return an HTTP error'
    ).toBeLessThan(400);

    await expect(
      page.getByText('Pricing & Variants Matrix', { exact: false })
    ).toBeVisible();
    await expect(
      page.getByText('Media Gallery', { exact: false })
    ).toBeVisible();

    if (seededSku) {
      await expect(page.getByText(seededSku, { exact: true }).first()).toBeVisible();
    }
  } finally {
    await close();
  }
});
