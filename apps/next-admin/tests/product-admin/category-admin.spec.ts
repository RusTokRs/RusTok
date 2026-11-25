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
 * Category admin browser evidence (audit runbook §19.4 row 1, §20/§21).
 *
 * Covers the category chain end to end against a live stack: the client-side
 * table filter, the searchable parent picker of the create dialog, and a real
 * subcategory creation (server action → GraphQL createCatalogCategory →
 * taxonomy sync → catalog_categories insert → reload verification).
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

const baseUrl = requiredUrl('RUSTOK_CATEGORY_ADMIN_BASE_URL');
const storageStatePath = requiredEnvironment(
  'RUSTOK_CATEGORY_ADMIN_STORAGE_STATE',
  1024
);
const seededParentTitle = requiredEnvironment(
  'RUSTOK_CATEGORY_ADMIN_SEEDED_PARENT_TITLE',
  512
);

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

async function openCreateDialog(page: Page): Promise<void> {
  await page.goto(`${baseUrl}/dashboard/product/categories`);
  await expect(
    page.getByRole('heading', { name: 'Catalog Categories' })
  ).toBeVisible();
  await page.getByRole('button', { name: 'Add Category' }).click();
  await expect(
    page.getByRole('dialog').getByText('Create Catalog Category')
  ).toBeVisible();
}

test('categories page filter narrows the category table', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    await page.goto(`${baseUrl}/dashboard/product/categories`);
    const response = await page.reload();
    expect(response, 'categories page must respond').not.toBeNull();
    expect(
      response!.status(),
      'categories page must not return an HTTP error'
    ).toBeLessThan(400);

    const fragment = seededParentTitle.slice(
      0,
      Math.min(4, seededParentTitle.length)
    );
    const missTerm = `${fragment}-no-such-category-path`;

    await page
      .getByPlaceholder('Filter categories by name, code or path...')
      .fill(fragment);
    await expect(
      page.getByText(seededParentTitle, { exact: false }).first()
    ).toBeVisible();

    await page
      .getByPlaceholder('Filter categories by name, code or path...')
      .fill(missTerm);
    await expect(
      page.getByText(seededParentTitle, { exact: false })
    ).toHaveCount(0);
  } finally {
    await close();
  }
});

test('parent category picker searches the category list', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    await openCreateDialog(page);

    const fragment = seededParentTitle.slice(
      0,
      Math.min(4, seededParentTitle.length)
    );
    const missTerm = `${fragment}-no-such-parent`;

    await page.locator('#cat-parent').click();
    await page
      .getByPlaceholder('Search parent category...')
      .fill(fragment);
    await expect(
      page.getByRole('option', { name: fragment }).first()
    ).toBeVisible();

    await page.getByPlaceholder('Search parent category...').fill(missTerm);
    await expect(page.getByText('No categories found.')).toBeVisible();
  } finally {
    await close();
  }
});

test('structural root and subcategory persist in the catalog tree', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const runId = Date.now();
    const rootName = `E2E Category ${runId}`;
    const rootSlug = `e2e-category-${runId}`;
    const childPath = `${rootSlug}/e2e-subcategory-${runId}`;
    const childName = `E2E Subcategory ${runId}`;

    await openCreateDialog(page);
    await page.locator('#cat-name').fill(rootName);
    await page.getByRole('button', { name: 'Create Category' }).click();
    await expect(page.getByText('Category created successfully')).toBeVisible({
      timeout: 30_000
    });

    await openCreateDialog(page);
    await page.locator('#cat-name').fill(childName);
    await page.locator('#cat-parent').click();
    await page.getByPlaceholder('Search parent category...').fill(rootSlug);
    await page.getByRole('option', { name: rootSlug, exact: true }).click();

    await page.getByRole('button', { name: 'Create Category' }).click();
    await expect(page.getByText('Category created successfully')).toBeVisible({
      timeout: 30_000
    });

    await page.goto(`${baseUrl}/dashboard/product/categories`);
    await page
      .getByPlaceholder('Filter categories by name, code or path...')
      .fill(rootSlug);
    const rootRow = page.getByRole('row').filter({
      has: page.getByText(rootName, { exact: true })
    });
    const childRow = page.getByRole('row').filter({
      has: page.getByText(childName, { exact: true })
    });
    await expect(
      rootRow.getByRole('cell', { name: 'structural' })
    ).toBeVisible();
    await expect(
      childRow.getByRole('cell', { name: 'structural' })
    ).toBeVisible();
    await expect(
      childRow.getByRole('cell', {
        name: childPath,
        exact: true
      })
    ).toBeVisible();

    await page.goto(`${baseUrl}/dashboard/product/new`);
    const primaryCategory = page.locator('#category-select');
    await primaryCategory.click();
    await page.getByPlaceholder('Search categories...').fill(childPath);
    await page.getByRole('option', { name: childPath, exact: true }).click();
    await expect(primaryCategory).toContainText(childPath);
  } finally {
    await close();
  }
});
