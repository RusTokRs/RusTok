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

test('sign-in uses the host locale for both the catalog and the language selector', async ({
  page
}) => {
  await page.goto('/auth/sign-in?locale=ru', { waitUntil: 'domcontentloaded' });
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
  await expect(
    page.getByRole('heading', { name: 'Вход в админ-панель' })
  ).toBeVisible();

  await page.getByRole('combobox').click();
  await page.getByRole('option', { name: 'English' }).click();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(
    page.getByRole('heading', { name: 'Sign in to admin' })
  ).toBeVisible();

  await page.reload({ waitUntil: 'domcontentloaded' });
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});
