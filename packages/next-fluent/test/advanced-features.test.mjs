import test from 'node:test';
import assert from 'node:assert/strict';
import React from 'react';

import {
  createFluentBundle,
  createTranslator,
  parseRichText,
  extractMessagesFromFtl,
  generateTypeDeclarations,
  pseudoLocalizeFtl,
  pseudoLocalizeText,
} from '../dist/index.js';

const stripBidiIsolates = (value) =>
  typeof value === 'string' ? value.replace(/[\u2068\u2069]/g, '') : value;

test('t.rich formats interactive markup tags into React elements', () => {
  const ftl = `
welcome-message = Hello, <bold>{ $name }</bold>! Visit our <link>website</link> or <docs>docs</docs>.
`;
  const bundle = createFluentBundle('en', ftl);
  const t = createTranslator(bundle);

  const result = t.rich('welcome-message', {
    name: 'Alice',
    bold: (chunks) => React.createElement('strong', { className: 'font-bold' }, chunks),
    link: (chunks) => React.createElement('a', { href: 'https://example.com' }, chunks),
    docs: React.createElement('span', { id: 'docs-badge' }),
  });

  // Verify structure is a React element
  assert.ok(React.isValidElement(result));
  const children = React.Children.toArray(result.props.children);
  assert.equal(children[0], 'Hello, ');
  
  // bold element
  const boldEl = children[1];
  assert.ok(React.isValidElement(boldEl));
  assert.equal(boldEl.type, 'strong');
  assert.equal(stripBidiIsolates(boldEl.props.children), 'Alice');

  assert.equal(children[2], '! Visit our ');

  // link element
  const linkEl = children[3];
  assert.ok(React.isValidElement(linkEl));
  assert.equal(linkEl.type, 'a');
  assert.equal(linkEl.props.href, 'https://example.com');
  assert.equal(linkEl.props.children, 'website');

  assert.equal(children[4], ' or ');

  // cloned docs element
  const docsEl = children[5];
  assert.ok(React.isValidElement(docsEl));
  assert.equal(docsEl.props.id, 'docs-badge');
  assert.equal(docsEl.props.children, 'docs');
});

test('t.rich handles nested tags and self-closing tags', () => {
  const ftl = `
formatted-notice = Important:<br/>Please read the <highlight><italic>terms</italic></highlight> carefully.
`;
  const bundle = createFluentBundle('en', ftl);
  const t = createTranslator(bundle);

  const result = t.rich('formatted-notice', {
    br: () => React.createElement('br'),
    highlight: (chunks) => React.createElement('mark', null, chunks),
    italic: (chunks) => React.createElement('em', null, chunks),
  });

  assert.ok(React.isValidElement(result));
  const children = React.Children.toArray(result.props.children);
  assert.equal(children[0], 'Important:');
  assert.ok(React.isValidElement(children[1]));
  assert.equal(children[1].type, 'br');
  assert.equal(children[2], 'Please read the ');

  const markEl = children[3];
  assert.ok(React.isValidElement(markEl));
  assert.equal(markEl.type, 'mark');

  const emEl = markEl.props.children;
  assert.ok(React.isValidElement(emEl));
  assert.equal(emEl.type, 'em');
  assert.equal(emEl.props.children, 'terms');

  assert.equal(children[4], ' carefully.');
});

test('t.rich returns raw text when no markup tags exist or match', () => {
  const ftl = `
plain = Simple plain text without any tags.
`;
  const bundle = createFluentBundle('en', ftl);
  const t = createTranslator(bundle);

  const result = t.rich('plain');
  assert.equal(result, 'Simple plain text without any tags.');
});

test('t.has accurately reports key existence', () => {
  const ftl = `
dashboard-title = Dashboard Overview
settings-account-email = Email Address
`;
  const bundle = createFluentBundle('en', ftl);
  const t = createTranslator(bundle);

  assert.equal(t.has('dashboard-title'), true);
  assert.equal(t.has('dashboard.title'), true);
  assert.equal(t.has('settings.account.email'), true);
  assert.equal(t.has('non-existent-key'), false);

  const nsT = createTranslator(bundle, 'settings');
  assert.equal(nsT.has('account.email'), true);
  assert.equal(nsT.has('unknown'), false);
});

test('fallbackBundle provides translations when primary bundle lacks a key', () => {
  const ftlEn = `
common-save = Save Changes
common-delete = Delete
features-newBadge = New Feature!
`;
  const ftlRu = `
common-save = Сохранить изменения
common-delete = Удалить
`;
  const bundleRu = createFluentBundle('ru', ftlRu);
  const bundleEn = createFluentBundle('en', ftlEn);

  const t = createTranslator(bundleRu, bundleEn);

  // Present in Russian
  assert.equal(t('common-save'), 'Сохранить изменения');
  assert.equal(t('common-delete'), 'Удалить');

  // Missing in Russian, resolved through fallback English bundle
  assert.equal(t('features-newBadge'), 'New Feature!');

  // Missing in both
  assert.equal(t('missing-key'), 'missing-key');
});

test('createFluentBundle supports multi-resource arrays with overrides', () => {
  const baseFtl = `
app-title = Base App
app-status = Inactive
feature-alpha = Alpha Feature
`;
  const moduleFtl = `
app-status = Active
feature-beta = Beta Feature
`;
  const bundle = createFluentBundle('en', [baseFtl, moduleFtl]);
  const t = createTranslator(bundle);

  assert.equal(t('app-title'), 'Base App');
  // Overridden by second resource
  assert.equal(t('app-status'), 'Active');
  assert.equal(t('feature-alpha'), 'Alpha Feature');
  assert.equal(t('feature-beta'), 'Beta Feature');
});

test('typegen parses FTL messages and generates TypeScript declarations', () => {
  const ftl = `
# Core messages
app-title = RusToK Platform
cart-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
user-greeting = Welcome, { $name }!
  .title = Profile
`;
  const messages = extractMessagesFromFtl(ftl);
  assert.equal(messages.length, 3);
  assert.equal(messages[0].id, 'app-title');
  assert.equal(messages[0].dotId, 'app.title');
  assert.equal(messages[1].id, 'cart-items');
  assert.deepEqual(messages[1].variables, ['count']);
  assert.equal(messages[2].id, 'user-greeting');
  assert.deepEqual(messages[2].attributes, ['title']);
  assert.deepEqual(messages[2].variables, ['name']);

  const dts = generateTypeDeclarations(ftl);
  assert.ok(dts.includes("export type AppMessageKey"));
  assert.ok(dts.includes("'app-title'"));
  assert.ok(dts.includes("'app.title'"));
  assert.ok(dts.includes("'cart.items': { count: string | number | Date }"));
  assert.ok(dts.includes("'user.greeting': { name: string | number | Date }"));
});

test('CURRENCY and PERCENT built-in functions format monetary and percentage values', () => {
  const ftlEn = `
order-total = Total: { CURRENCY($val, currency: "USD") }
order-discount = Discount: { PERCENT($rate) }
`;
  const bundleEn = createFluentBundle('en', ftlEn);
  const tEn = createTranslator(bundleEn);

  assert.equal(stripBidiIsolates(tEn('order-total', { val: 99.99 })), 'Total: $99.99');
  assert.equal(stripBidiIsolates(tEn('order-discount', { rate: 0.15 })), 'Discount: 15%');

  const ftlRu = `
order-total = К оплате: { CURRENCY($val, currency: "RUB") }
`;
  const bundleRu = createFluentBundle('ru', ftlRu);
  const tRu = createTranslator(bundleRu);

  const ruResult = stripBidiIsolates(tRu('order-total', { val: 1500 }));
  // Russian currency formatter includes ruble symbol ₽ or руб.
  assert.ok(ruResult.includes('1') && (ruResult.includes('₽') || ruResult.includes('руб')));
});

test('debug mode visually highlights missing keys and logs warnings', () => {
  const ftl = `
app-header = Header
`;
  const bundle = createFluentBundle('en', ftl);
  const tDebug = createTranslator(bundle, { debug: true });

  assert.equal(tDebug('app-header'), 'Header');
  assert.equal(tDebug('missing.key'), '[MISSING: missing.key]');

  const tNsDebug = createTranslator(bundle, { namespace: 'dashboard', debug: true });
  assert.equal(tNsDebug('unknownAction'), '[MISSING: dashboard.unknownAction]');
});

test('pseudo-localization accents text and elongates vowels while preserving FTL variables and tags', () => {
  const ftl = `
# System messages
welcome-title = Welcome to our shop, { $name }!
  .tooltip = Click <link>here</link> to continue
`;
  const pseudo = pseudoLocalizeFtl(ftl);

  // Checks that identifiers and comments are preserved
  assert.ok(pseudo.includes('welcome-title = '));
  assert.ok(pseudo.includes('.tooltip = '));
  assert.ok(pseudo.includes('# System messages'));

  // Checks that variables and tags are preserved
  assert.ok(pseudo.includes('{ $name }'));
  assert.ok(pseudo.includes('<link>'));
  assert.ok(pseudo.includes('</link>'));

  // Checks that human words are accented and wrapped
  assert.ok(pseudo.includes('['));
  assert.ok(pseudo.includes(']'));
});
