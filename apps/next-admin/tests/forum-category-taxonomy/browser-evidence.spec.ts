import { expect, test, type Browser, type Page } from '@playwright/test';

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
  return parsed.toString();
}

function requestedLocaleAppearsInPath(url: string, locale: string): boolean {
  return new URL(url).pathname
    .split('/')
    .filter(Boolean)
    .some((segment) => segment === locale);
}

function regexEscape(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

async function navigate(page: Page, url: string, label: string): Promise<void> {
  const response = await page.goto(url);
  expect(response, `${label} must produce an HTTP response`).not.toBeNull();
  expect(
    response!.status(),
    `${label} must not return an HTTP error`
  ).toBeLessThan(400);
}

async function authenticatedAdminPage(
  browser: Browser
): Promise<{ page: Page; close: () => Promise<void> }> {
  const storageState = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE',
    16_384
  );
  const context = await browser.newContext({ storageState });
  const page = await context.newPage();
  return { page, close: () => context.close() };
}

function adminCategoryCard(page: Page, name: string) {
  return page.locator('article').filter({
    has: page.locator('h3[data-forum-target-localized]', { hasText: name })
  });
}

function storefrontCategoryCard(
  page: Page,
  canonicalPath: string,
  name: string
) {
  return page
    .locator(`aside a[href="${canonicalPath}"]`)
    .filter({ hasText: name });
}

test('Forum Category admin renders Taxonomy-owned RTL hierarchy, order and presentation', async ({
  browser
}) => {
  const url = requiredUrl('RUSTOK_FORUM_CATEGORY_ADMIN_RTL_E2E_URL');
  const requestedLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_RTL_REQUESTED_LOCALE'
  );
  const effectiveLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_RTL_EFFECTIVE_LOCALE'
  );
  const rootName = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_ROOT_NAME');
  const rootSlug = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_ROOT_SLUG');
  const childName = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_CHILD_NAME');
  const childSlug = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_CHILD_SLUG');
  const icon = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_ROOT_ICON');
  const accentClass = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_ACCENT_CLASS'
  );

  expect(requestedLocaleAppearsInPath(url, requestedLocale)).toBe(true);

  const admin = await authenticatedAdminPage(browser);
  try {
    await navigate(admin.page, url, 'Forum Category RTL admin route');

    const rootCard = adminCategoryCard(admin.page, rootName);
    const childCard = adminCategoryCard(admin.page, childName);
    await expect(rootCard).toHaveCount(1);
    await expect(childCard).toHaveCount(1);

    await expect(
      rootCard.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('lang', effectiveLocale);
    await expect(
      rootCard.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('dir', 'auto');
    await expect(rootCard.locator('h3[data-forum-target-localized]')).toHaveCSS(
      'direction',
      'rtl'
    );
    await expect(
      rootCard.locator('[data-forum-route-identifier]')
    ).toHaveAttribute('dir', 'ltr');
    await expect(rootCard.locator('[data-forum-route-identifier]')).toHaveText(
      `#${rootSlug}`
    );
    await expect(rootCard).toContainText('depth 0 · position 0');
    await expect(rootCard).toContainText(icon);
    await expect(
      rootCard.locator('span[class*="inset-y-0"][class*="left-0"]')
    ).toHaveClass(new RegExp(regexEscape(accentClass)));

    await expect(
      childCard.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('lang', effectiveLocale);
    await expect(
      childCard.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('dir', 'auto');
    await expect(
      childCard.locator('h3[data-forum-target-localized]')
    ).toHaveCSS('direction', 'rtl');
    await expect(
      childCard.locator('[data-forum-route-identifier]')
    ).toHaveAttribute('dir', 'ltr');
    await expect(childCard.locator('[data-forum-route-identifier]')).toHaveText(
      `#${childSlug}`
    );
    await expect(childCard).toContainText('depth 1 · position 0');

    const categoryNames = await admin.page
      .locator('article h3[data-forum-target-localized]')
      .allTextContents();
    expect(categoryNames.indexOf(rootName)).toBeGreaterThanOrEqual(0);
    expect(categoryNames.indexOf(childName)).toBeGreaterThan(
      categoryNames.indexOf(rootName)
    );
  } finally {
    await admin.close();
  }
});

test('Forum Category admin exposes requested-to-effective Taxonomy locale fallback', async ({
  browser
}) => {
  const url = requiredUrl('RUSTOK_FORUM_CATEGORY_ADMIN_FALLBACK_E2E_URL');
  const requestedLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_REQUESTED_LOCALE'
  );
  const effectiveLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_EFFECTIVE_LOCALE'
  );
  const name = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_NAME');
  const slug = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_SLUG');

  expect(requestedLocale).not.toBe(effectiveLocale);
  expect(requestedLocaleAppearsInPath(url, requestedLocale)).toBe(true);

  const admin = await authenticatedAdminPage(browser);
  try {
    await navigate(admin.page, url, 'Forum Category fallback admin route');
    const card = adminCategoryCard(admin.page, name);
    await expect(card).toHaveCount(1);
    await expect(
      card.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('lang', effectiveLocale);
    await expect(
      card.locator('h3[data-forum-target-localized]')
    ).toHaveAttribute('dir', 'auto');
    await expect(card.locator('[data-forum-route-identifier]')).toHaveText(
      `#${slug}`
    );
    await expect(card.locator('[data-forum-route-identifier]')).toHaveAttribute(
      'dir',
      'ltr'
    );
  } finally {
    await admin.close();
  }
});

test('Forum Category storefront renders Taxonomy-owned RTL copy and canonical routes in owner order', async ({
  page
}) => {
  const url = requiredUrl('RUSTOK_FORUM_CATEGORY_STOREFRONT_RTL_E2E_URL');
  const requestedLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_RTL_REQUESTED_LOCALE'
  );
  const effectiveLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_RTL_EFFECTIVE_LOCALE'
  );
  const rootName = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_ROOT_NAME');
  const rootSlug = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_ROOT_SLUG');
  const childName = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_CHILD_NAME');
  const rootPath = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_ROOT_CANONICAL_PATH'
  );
  const childPath = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_CHILD_CANONICAL_PATH'
  );
  const accentClass = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_ACCENT_CLASS'
  );

  expect(requestedLocaleAppearsInPath(url, requestedLocale)).toBe(true);
  await navigate(page, url, 'Forum Category RTL storefront route');

  const rootCard = storefrontCategoryCard(page, rootPath, rootName);
  const childCard = storefrontCategoryCard(page, childPath, childName);
  await expect(rootCard).toHaveCount(1);
  await expect(childCard).toHaveCount(1);
  await expect(
    rootCard.locator('h4[data-forum-target-localized]')
  ).toHaveAttribute('lang', effectiveLocale);
  await expect(
    rootCard.locator('h4[data-forum-target-localized]')
  ).toHaveAttribute('dir', 'auto');
  await expect(rootCard.locator('h4[data-forum-target-localized]')).toHaveCSS(
    'direction',
    'rtl'
  );
  await expect(rootCard.locator('[data-forum-route-identifier]')).toHaveText(
    `#${rootSlug}`
  );
  await expect(
    rootCard.locator('[data-forum-route-identifier]')
  ).toHaveAttribute('dir', 'ltr');
  await expect(
    rootCard.locator('span[class*="inset-y-0"][class*="left-0"]')
  ).toHaveClass(new RegExp(regexEscape(accentClass)));

  await expect(
    childCard.locator('h4[data-forum-target-localized]')
  ).toHaveAttribute('lang', effectiveLocale);
  await expect(
    childCard.locator('h4[data-forum-target-localized]')
  ).toHaveAttribute('dir', 'auto');
  await expect(childCard.locator('h4[data-forum-target-localized]')).toHaveCSS(
    'direction',
    'rtl'
  );

  const categoryNames = await page
    .locator('aside h4[data-forum-target-localized]')
    .allTextContents();
  expect(categoryNames.indexOf(rootName)).toBeGreaterThanOrEqual(0);
  expect(categoryNames.indexOf(childName)).toBeGreaterThan(
    categoryNames.indexOf(rootName)
  );
});

test('Forum Category storefront falls back copy while linking to effective-locale canonical route', async ({
  page
}) => {
  const url = requiredUrl('RUSTOK_FORUM_CATEGORY_STOREFRONT_FALLBACK_E2E_URL');
  const requestedLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_REQUESTED_LOCALE'
  );
  const effectiveLocale = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_EFFECTIVE_LOCALE'
  );
  const name = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_NAME');
  const slug = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_SLUG');
  const canonicalPath = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_CANONICAL_PATH'
  );

  expect(requestedLocale).not.toBe(effectiveLocale);
  expect(requestedLocaleAppearsInPath(url, requestedLocale)).toBe(true);
  await navigate(page, url, 'Forum Category fallback storefront route');

  const card = storefrontCategoryCard(page, canonicalPath, name);
  await expect(card).toHaveCount(1);
  await expect(card.locator('h4[data-forum-target-localized]')).toHaveAttribute(
    'lang',
    effectiveLocale
  );
  await expect(card.locator('h4[data-forum-target-localized]')).toHaveAttribute(
    'dir',
    'auto'
  );
  await expect(card.locator('[data-forum-route-identifier]')).toHaveText(
    `#${slug}`
  );
  await expect(card.locator('[data-forum-route-identifier]')).toHaveAttribute(
    'dir',
    'ltr'
  );
});

test('Forum Category storefront alias redirects to the Taxonomy canonical route', async ({
  page
}) => {
  const aliasUrl = requiredUrl(
    'RUSTOK_FORUM_CATEGORY_STOREFRONT_ALIAS_E2E_URL'
  );
  const canonicalUrl = requiredUrl(
    'RUSTOK_FORUM_CATEGORY_STOREFRONT_CANONICAL_E2E_URL'
  );
  const canonicalPath = requiredEnvironment(
    'RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_CANONICAL_PATH'
  );
  const name = requiredEnvironment('RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_NAME');

  expect(aliasUrl).not.toBe(canonicalUrl);
  await navigate(page, aliasUrl, 'Forum Category alias storefront route');
  expect(page.url()).toBe(canonicalUrl);
  await expect(storefrontCategoryCard(page, canonicalPath, name)).toHaveCount(
    1
  );
});
