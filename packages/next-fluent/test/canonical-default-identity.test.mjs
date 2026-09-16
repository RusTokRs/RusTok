import test from 'node:test';
import assert from 'node:assert/strict';

import { createI18n } from '../dist/index.js';
import { createI18nMiddleware } from '../dist/middleware.js';
import {
  configureServerI18n,
  getLocale,
  getRequestConfig,
} from '../dist/server.js';
import { matchConfiguredLocaleIdentity } from '../dist/utils.js';

function requestFor(pathname, acceptLanguage = null) {
  return {
    url: `https://rustok.local${pathname}`,
    nextUrl: {
      pathname,
      search: '',
    },
    cookies: {
      get: () => undefined,
    },
    headers: {
      get: (name) => (name === 'accept-language' ? acceptLanguage : null),
    },
  };
}

test('configured identity matching is exact-canonical and never structural fallback', () => {
  assert.equal(
    matchConfiguredLocaleIdentity('en_US', ['en-US', 'en', 'ru']),
    'en-US'
  );
  assert.equal(
    matchConfiguredLocaleIdentity('EN-us', ['en-US', 'en', 'ru']),
    'en-US'
  );
  assert.equal(
    matchConfiguredLocaleIdentity('en-GB', ['en', 'ru']),
    undefined
  );
});

test('middleware uses the configured default spelling for redirects and as-needed comparison', async () => {
  const always = createI18nMiddleware({
    locales: ['en-US', 'ru'],
    defaultLocale: 'en_US',
    localePrefix: 'always',
  });
  const rootResponse = await always(requestFor('/'));
  assert.equal(rootResponse.status, 307);
  assert.equal(rootResponse.headers.get('location'), 'https://rustok.local/en-US');
  assert.equal(rootResponse.headers.get('x-rustok-effective-locale'), 'en-US');

  const asNeeded = createI18nMiddleware({
    locales: ['en-US', 'ru'],
    defaultLocale: 'en_US',
    localePrefix: 'as-needed',
  });
  const prefixedDefault = await asNeeded(requestFor('/en-US/dashboard'));
  assert.equal(prefixedDefault.status, 307);
  assert.equal(prefixedDefault.headers.get('location'), 'https://rustok.local/dashboard');
  assert.equal(prefixedDefault.headers.get('x-rustok-effective-locale'), 'en-US');
});

test('createI18n passes configured default identity to the message loader', async () => {
  let loadedLocale;
  createI18n({
    locales: ['en-US', 'ru'],
    defaultLocale: 'en_US',
    loadMessages: async (locale) => {
      loadedLocale = locale;
      return 'title = Title';
    },
  });

  const requestConfig = getRequestConfig();
  assert.ok(requestConfig);
  const result = await requestConfig({ locale: undefined });
  assert.equal(loadedLocale, 'en-US');
  assert.equal(result.locale, 'en-US');
});

test('server configuration validates prospective locale/default state atomically', async () => {
  configureServerI18n({
    locales: ['en-US', 'ru'],
    defaultLocale: 'en_US',
  });

  assert.throws(
    () => configureServerI18n({ locales: ['ru'] }),
    /defaultLocale.*must be included|must be included in "locales"/
  );

  assert.equal(await getLocale(), 'en-US');
});
