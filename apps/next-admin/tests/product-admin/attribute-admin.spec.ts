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
 * Attribute admin browser evidence (audit runbook §19.4, §22).
 *
 * Covers the attribute chain end to end against a live stack: the table
 * filter, the searchable attribute picker of the schema authoring card, and a
 * real attribute creation (server action → GraphQL createProductAttribute →
 * schema service → catalog tables → reload verification).
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

const baseUrl = requiredUrl('RUSTOK_ATTRIBUTE_ADMIN_BASE_URL');
const storageStatePath = requiredEnvironment(
  'RUSTOK_ATTRIBUTE_ADMIN_STORAGE_STATE',
  1024
);
const seededAttributeLabel = requiredEnvironment(
  'RUSTOK_ATTRIBUTE_ADMIN_SEEDED_LABEL',
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

test('attributes page filter narrows the attribute table', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const response = await page.goto(`${baseUrl}/dashboard/product/attributes`);
    expect(response, 'attributes page must respond').not.toBeNull();
    expect(
      response!.status(),
      'attributes page must not return an HTTP error'
    ).toBeLessThan(400);

    const fragment = seededAttributeLabel.slice(
      0,
      Math.min(4, seededAttributeLabel.length)
    );
    const missTerm = `${fragment}-no-such-attribute`;

    await page
      .getByPlaceholder('Filter attributes by label or code...')
      .fill(fragment);
    await expect(
      page.getByText(seededAttributeLabel, { exact: false }).first()
    ).toBeVisible();

    await page
      .getByPlaceholder('Filter attributes by label or code...')
      .fill(missTerm);
    await expect(
      page.getByText(seededAttributeLabel, { exact: false })
    ).toHaveCount(0);
  } finally {
    await close();
  }
});

test('schema authoring attribute picker searches the attribute list', async ({
  browser
}) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    await page.goto(`${baseUrl}/dashboard/product/attributes`);

    const fragment = seededAttributeLabel.slice(
      0,
      Math.min(4, seededAttributeLabel.length)
    );
    const missTerm = `${fragment}-no-such-binding`;

    await page.locator('#schema-bind-attribute').scrollIntoViewIfNeeded();
    await page.locator('#schema-bind-attribute').click();
    await page.getByPlaceholder('Search attributes...').fill(fragment);
    await expect(
      page.getByRole('option', { name: fragment }).first()
    ).toBeVisible();

    await page.getByPlaceholder('Search attributes...').fill(missTerm);
    await expect(page.getByText('No results found.')).toBeVisible();
  } finally {
    await close();
  }
});

test('attribute is created through the admin dialog', async ({ browser }) => {
  const { page, close } = await authenticatedAdminPage(browser);
  try {
    const uniqueLabel = `E2E Attribute ${Date.now()}`;
    const uniqueCode = `e2e_attribute_${Date.now()}`;

    await page.goto(`${baseUrl}/dashboard/product/attributes`);
    await page.getByRole('button', { name: 'Add Attribute' }).click();
    await expect(
      page.getByRole('dialog').getByText('Create Product Attribute')
    ).toBeVisible();

    await page.locator('#attr-label').fill(uniqueLabel);
    await page.locator('#attr-code').fill(uniqueCode);
    await page.getByRole('button', { name: 'Create Attribute' }).click();
    await expect(page.getByText('Attribute created successfully')).toBeVisible({
      timeout: 30_000
    });

    // The attributes page is a server component: reload to verify the new
    // attribute actually persisted.
    await page.goto(`${baseUrl}/dashboard/product/attributes`);
    await page
      .getByPlaceholder('Filter attributes by label or code...')
      .fill(uniqueCode);
    await expect(
      page.getByText(uniqueLabel, { exact: false }).first()
    ).toBeVisible();
  } finally {
    await close();
  }
});
