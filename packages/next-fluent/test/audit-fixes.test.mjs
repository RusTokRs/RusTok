import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  createFluentBundle,
  createTranslator,
  createI18nMiddleware,
  validateI18nConfig,
  canonicalizeLocale,
  matchSupportedLocale,
  resolveAcceptLanguage,
  forLocale,
  getLocale,
  createI18n,
} from '../dist/index.js';

test('NF-01: Multi-bundle fallback chain resolves bundle-by-bundle without collision', () => {
  const ruBundle = createFluentBundle('ru', `
save = Сохранить
edit = Редактировать
  `);

  const enBundle = createFluentBundle('en', `
save = Save
edit = Edit
cancel = Cancel
delete = Delete
  `);

  const deBundle = createFluentBundle('de', `
delete = Löschen
help = Hilfe
  `);

  const t = createTranslator(ruBundle, {
    fallbackBundles: [enBundle, deBundle],
  });

  // 1. Primary bundle key
  assert.equal(t('save'), 'Сохранить');
  assert.equal(t('edit'), 'Редактировать');

  // 2. Secondary fallback bundle key
  assert.equal(t('cancel'), 'Cancel');
  assert.equal(t('delete'), 'Delete');

  // 3. Tertiary fallback bundle key
  assert.equal(t('help'), 'Hilfe');

  // 4. Key existence across all bundles
  assert.equal(t.has('save'), true);
  assert.equal(t.has('cancel'), true);
  assert.equal(t.has('help'), true);
  assert.equal(t.has('nonexistent'), false);

  // 5. Raw values from fallback
  assert.equal(t.raw('help'), 'Hilfe');
});

test('NF-02: Middleware propagates request headers to Server Components', async () => {
  const middleware = createI18nMiddleware({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    localePrefix: 'always',
  });

  const mockRequest = {
    url: 'https://example.com/ru/dashboard',
    nextUrl: {
      pathname: '/ru/dashboard',
      search: '',
    },
    cookies: {
      get: () => undefined,
      set: () => {},
    },
    headers: new Headers({
      'user-agent': 'TestAgent',
    }),
  };

  const response = await middleware(mockRequest);
  assert.equal(response.status, 200);

  // Both response header AND downstream request header must be populated
  assert.equal(response.headers.get('x-rustok-effective-locale'), 'ru');
  assert.equal(response.request.headers.get('x-rustok-effective-locale'), 'ru');
  assert.equal(response.request.headers.get('user-agent'), 'TestAgent');
});

test('NF-03: Configuration validation throws on missing or invalid defaultLocale', () => {
  assert.throws(
    () => validateI18nConfig({ locales: ['en', 'ru'], defaultLocale: 'fr' }),
    /defaultLocale.*must be included in "locales"/
  );

  assert.throws(
    () => validateI18nConfig({ locales: [], defaultLocale: 'en' }),
    /"locales" must be a non-empty array/
  );

  assert.throws(
    () => validateI18nConfig({ locales: ['invalid---tag'], defaultLocale: 'invalid---tag' }),
    /Invalid locale tag/
  );

  // Valid configuration should not throw
  assert.doesNotThrow(() =>
    validateI18nConfig({ locales: ['en', 'ru'], defaultLocale: 'en' })
  );
});

test('NF-04: localePrefix strategies (always, as-needed, never)', async () => {
  const makeReq = (pathname, cookieVal, acceptLang) => ({
    url: `https://example.com${pathname}`,
    nextUrl: { pathname, search: '' },
    cookies: {
      get: () => (cookieVal ? { value: cookieVal } : undefined),
      set: () => {},
    },
    headers: new Headers(acceptLang ? { 'accept-language': acceptLang } : {}),
  });

  // 1. 'always' strategy: / -> redirects to /en
  const mwAlways = createI18nMiddleware({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    localePrefix: 'always',
  });
  const resAlwaysRoot = await mwAlways(makeReq('/', undefined, undefined));
  assert.equal(resAlwaysRoot.status, 307);
  assert.equal(resAlwaysRoot.headers.get('location'), 'https://example.com/en');

  // 2. 'as-needed' strategy:
  const mwAsNeeded = createI18nMiddleware({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    localePrefix: 'as-needed',
  });
  // a) default locale path /en/about should redirect to /about
  const resAsNeededPrefixedDefault = await mwAsNeeded(makeReq('/en/about', undefined, undefined));
  assert.equal(resAsNeededPrefixedDefault.status, 307);
  assert.equal(resAsNeededPrefixedDefault.headers.get('location'), 'https://example.com/about');

  // b) default locale path /about should succeed directly without redirect
  const resAsNeededUnprefixed = await mwAsNeeded(makeReq('/about', undefined, undefined));
  assert.equal(resAsNeededUnprefixed.status, 200);
  assert.equal(resAsNeededUnprefixed.headers.get('x-rustok-effective-locale'), 'en');

  // c) non-default locale /ru/about should succeed directly with ru
  const resAsNeededRu = await mwAsNeeded(makeReq('/ru/about', undefined, undefined));
  assert.equal(resAsNeededRu.status, 200);
  assert.equal(resAsNeededRu.headers.get('x-rustok-effective-locale'), 'ru');

  // 3. 'never' strategy:
  const mwNever = createI18nMiddleware({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    localePrefix: 'never',
  });
  // a) /ru/about should strip prefix and redirect to /about
  const resNeverPrefixed = await mwNever(makeReq('/ru/about', undefined, undefined));
  assert.equal(resNeverPrefixed.status, 307);
  assert.equal(resNeverPrefixed.headers.get('location'), 'https://example.com/about');

  // b) /about should succeed with resolved locale from cookie
  const resNeverCookie = await mwNever(makeReq('/about', 'ru', undefined));
  assert.equal(resNeverCookie.status, 200);
  assert.equal(resNeverCookie.headers.get('x-rustok-effective-locale'), 'ru');
});

test('NF-05: Server locale resolution enforces allow-list boundary against malicious inputs', async () => {
  // 1. matchSupportedLocale rejects directory traversal and arbitrary paths
  assert.equal(matchSupportedLocale('../../etc/passwd', ['en', 'ru']), undefined);
  assert.equal(matchSupportedLocale('../../../locales/evil', ['en', 'ru']), undefined);
  assert.equal(matchSupportedLocale('javascript:alert(1)', ['en', 'ru']), undefined);

  // 2. getLocale falls back safely to defaultLocale
  const locale = await getLocale({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
  });
  assert.equal(locale, 'en');
});

test('NF-06: BCP 47 canonicalization handles complex subtags and invalid tags safely', () => {
  assert.equal(canonicalizeLocale('zh_hant_tw'), 'zh-Hant-TW');
  assert.equal(canonicalizeLocale('ru_RU'), 'ru-RU');
  assert.equal(canonicalizeLocale('en_US'), 'en-US');
  assert.equal(canonicalizeLocale('../../malicious'), undefined);
  assert.equal(canonicalizeLocale(''), undefined);

  // matchSupportedLocale with canonicalization
  assert.equal(matchSupportedLocale('zh_hant_tw', ['zh-Hant-TW', 'en']), 'zh-Hant-TW');
  assert.equal(matchSupportedLocale('en-GB', ['en', 'ru']), 'en');
  assert.equal(matchSupportedLocale('../../hack', ['en', 'ru']), undefined);
});

test('NF-11: forLocale provides static SSG translations without request headers', async () => {
  const t = await forLocale('en');
  assert.ok(typeof t === 'function');
  assert.ok(typeof t.raw === 'function');
  assert.ok(typeof t.rich === 'function');
  assert.ok(typeof t.has === 'function');
});

test('NF-16: resolveAcceptLanguage handles whitespace, q-factors, and q=0 accurately', () => {
  const locales = ['en', 'ru', 'de'];

  // Whitespace in ; q=0.8
  const headerWithSpaces = 'fr; q=0.5, ru; q=0.9, en; q=0.8';
  assert.equal(resolveAcceptLanguage(headerWithSpaces, locales), 'ru');

  // q=0 (disallowed)
  const headerZeroQ = 'ru; q=0, de; q=0.7, en; q=0.5';
  assert.equal(resolveAcceptLanguage(headerZeroQ, locales), 'de');

  // Wildcard *
  assert.equal(resolveAcceptLanguage('*', locales), 'en');
});

test('createI18n factory returns cohesive, functional runtime instance', async () => {
  const i18n = createI18n({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
    localePrefix: 'as-needed',
    loadMessages: (locale) => (locale === 'ru' ? 'welcome = Добро пожаловать' : 'welcome = Welcome'),
  });

  assert.equal(i18n.config.defaultLocale, 'en');
  assert.ok(typeof i18n.middleware === 'function');
  assert.ok(typeof i18n.getLocale === 'function');
  assert.ok(typeof i18n.getTranslations === 'function');
  assert.ok(typeof i18n.forLocale === 'function');

  const tRu = await i18n.forLocale('ru');
  assert.equal(tRu('welcome'), 'Добро пожаловать');

  const tEn = await i18n.forLocale('en');
  assert.equal(tEn('welcome'), 'Welcome');
});

test('forLocale supports direct in-memory messages and fallbackMessages', async () => {
  const t = await forLocale('ru', {
    messages: 'greet = Привет, {$name}!',
    fallbackLocale: 'en',
    fallbackMessages: 'farewell = Goodbye!\ngreet = Hello, {$name}!',
  });

  assert.equal(t('greet', { name: 'Иван' }), 'Привет, Иван!');
  assert.equal(t('farewell'), 'Goodbye!');
});

