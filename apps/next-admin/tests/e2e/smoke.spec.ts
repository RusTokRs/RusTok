import { expect, test } from '@playwright/test';

test('next admin host renders without browser runtime errors', async ({
  page
}) => {
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));

  const response = await page.goto(
    process.env.RUSTOK_NEXT_ADMIN_E2E_SMOKE_PATH ?? '/auth/sign-in'
  );
  expect(response?.status()).toBeLessThan(400);
  await expect(page.locator('body')).toBeVisible();
  await expect.poll(() => pageErrors).toEqual([]);
});
