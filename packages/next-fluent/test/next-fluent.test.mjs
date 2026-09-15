import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createFluentBundle,
  createTranslator,
  normalizeLocaleTag,
  matchSupportedLocale,
  resolveAcceptLanguage,
  withKebabKey,
} from '../dist/index.js';

import { createI18nMiddleware } from '../dist/middleware.js';

test('normalizeLocaleTag handles tags correctly', () => {
  assert.equal(normalizeLocaleTag('en'), 'en');
  assert.equal(normalizeLocaleTag('en_US'), 'en-US');
  assert.equal(normalizeLocaleTag('ru_RU'), 'ru-RU');
  assert.equal(normalizeLocaleTag('pt_BR'), 'pt-BR');
  assert.equal(normalizeLocaleTag(''), undefined);
  assert.equal(normalizeLocaleTag(null), undefined);
});

test('matchSupportedLocale matches exact and base languages', () => {
  const locales = ['en', 'ru'];
  assert.equal(matchSupportedLocale('en', locales), 'en');
  assert.equal(matchSupportedLocale('ru', locales), 'ru');
  assert.equal(matchSupportedLocale('ru-RU', locales), 'ru');
  assert.equal(matchSupportedLocale('en-US', locales), 'en');
  assert.equal(matchSupportedLocale('fr', locales), undefined);
});

test('resolveAcceptLanguage parses quality values', () => {
  const locales = ['en', 'ru'];
  assert.equal(
    resolveAcceptLanguage('ru-RU,ru;q=0.9,en;q=0.8', locales),
    'ru'
  );
  assert.equal(
    resolveAcceptLanguage('fr-FR,fr;q=0.9,en;q=0.8', locales),
    'en'
  );
  assert.equal(resolveAcceptLanguage(null, locales), undefined);
});

test('withKebabKey converts dots to hyphens', () => {
  assert.equal(withKebabKey('app.nav.dashboard'), 'app-nav-dashboard');
  assert.equal(withKebabKey('richText.bold'), 'richText-bold');
  assert.equal(withKebabKey('already-kebab'), 'already-kebab');
});

test('Russian plural forms (one/few/many) evaluate correctly', () => {
  const ftlRu = `
cart-items = { $count ->
    [one] { $count } товар
    [few] { $count } товара
   *[other] { $count } товаров
}
`;
  const bundle = createFluentBundle('ru', ftlRu);
  const t = createTranslator(bundle);

  assert.equal(t('cart-items', { count: 1 }), '1 товар');
  assert.equal(t('cart-items', { count: 2 }), '2 товара');
  assert.equal(t('cart-items', { count: 3 }), '3 товара');
  assert.equal(t('cart-items', { count: 4 }), '4 товара');
  assert.equal(t('cart-items', { count: 5 }), '5 товаров');
  assert.equal(t('cart-items', { count: 11 }), '11 товаров');
  assert.equal(t('cart-items', { count: 21 }), '21 товар');
  assert.equal(t('cart-items', { count: 24 }), '24 товара');
});

test('English plural forms evaluate correctly', () => {
  const ftlEn = `
items-count = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
`;
  const bundle = createFluentBundle('en', ftlEn);
  const t = createTranslator(bundle);

  assert.equal(t('items-count', { count: 1 }), '1 item');
  assert.equal(t('items-count', { count: 2 }), '2 items');
  assert.equal(t('items-count', { count: 0 }), '0 items');
});

test('Namespaced translations and variable interpolation', () => {
  const ftl = `
app-nav-dashboard = Dashboard
app-nav-welcome = Welcome, { $name }!
richText-bold = Bold
richText-error-serialize = Preparation failed
`;
  const bundle = createFluentBundle('en', ftl);

  const tNav = createTranslator(bundle, 'app.nav');
  assert.equal(tNav('dashboard'), 'Dashboard');
  assert.equal(tNav('welcome', { name: 'RusToK' }), 'Welcome, RusToK!');

  const tRich = createTranslator(bundle, 'richText');
  assert.equal(tRich('bold'), 'Bold');
  assert.equal(tRich('error.serialize'), 'Preparation failed');
});

test('t.raw formats attributes and JSON arrays', () => {
  const ftl = `
Storefront-features =
    .item0 = SSR + SEO out of the box
    .item1 = Fast access to GraphQL API
    .item2 = Composable storefronts

Storefront-chips = ["React Query", "Zod", "Tailwind"]
`;
  const bundle = createFluentBundle('en', ftl);
  const t = createTranslator(bundle, 'Storefront');

  const features = t.raw('features');
  assert.deepEqual(features, [
    'SSR + SEO out of the box',
    'Fast access to GraphQL API',
    'Composable storefronts',
  ]);

  const chips = t.raw('chips');
  assert.deepEqual(chips, ['React Query', 'Zod', 'Tailwind']);
});

test('Middleware correctly redirects unlocalized routes to locale prefix', async () => {
  const middleware = createI18nMiddleware({
    locales: ['en', 'ru'],
    defaultLocale: 'en',
  });

  const mockRequest = {
    url: 'https://rustok.local/',
    nextUrl: {
      pathname: '/',
      search: '',
    },
    cookies: {
      get: () => undefined,
    },
    headers: {
      get: (header) => (header === 'accept-language' ? 'ru' : null),
    },
  };

  const response = await middleware(mockRequest);
  assert.equal(response.status, 307);
  assert.equal(response.headers.get('location'), 'https://rustok.local/ru');
  assert.equal(response.headers.get('x-rustok-effective-locale'), 'ru');
});
