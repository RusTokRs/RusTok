/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import { builderUrlForPage, storefrontUrlForPage } from './api/urls.ts';
import { createPageFromStarter, LayoutSelectionError } from './model/create-page.ts';

// Load api/pages.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/pages.ts', import.meta.url),
  'utf8'
);
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace("import { graphqlRequest } from '@/lib/graphql';", '')
    .replace(
      "export { builderUrlForPage, storefrontUrlForPage } from './urls';",
      ''
    )
    .replaceAll('export ', '')
);

const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      PAGE_LIST_FIELDS,
      PAGES_LIST_QUERY,
      PAGE_STARTERS_QUERY,
      PAGE_LAYOUT_KEYS_QUERY,
      INSTANTIATE_PAGE_STARTER_MUTATION,
      SAVE_STARTER_FROM_PAGE_MUTATION,
      UPDATE_STARTER_METADATA_MUTATION,
      DELETE_STARTER_MUTATION,
      CREATE_PAGE_MUTATION,
      UNPUBLISH_PAGE_MUTATION,
      DELETE_PAGE_MUTATION,
      listPages,
      listPageStarters,
      listPageLayoutKeys,
      instantiatePageStarter,
      saveStarterFromPage,
      updateStarterMetadata,
      deleteStarter,
      createPage,
      unpublishPage,
      deletePage
    });`,
    ['graphqlRequest', 'exports']
  )(mockGraphqlRequest, exports);
  return exports;
};

// Load nav.ts by stripping type import
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace("import type { NavItem } from '@/types';", '')
    .replaceAll('export ', '')
);
const navModule = {};
compileFunction(`${cleanNavSource}\nnavModule.pagesNavItems = pagesNavItems;`, [
  'navModule'
])(navModule);
const { pagesNavItems } = navModule;

test('builderUrlForPage constructs deep link to Leptos admin editor', () => {
  const url = builderUrlForPage('http://localhost:5150', 'page-123');
  assert.equal(url, 'http://localhost:5150/modules/pages?pageId=page-123');

  const urlWithTrailingSlash = builderUrlForPage(
    'http://localhost:5150/',
    'page-456'
  );
  assert.equal(
    urlWithTrailingSlash,
    'http://localhost:5150/modules/pages?pageId=page-456'
  );
});

test('storefrontUrlForPage constructs canonical storefront link for published pages', () => {
  const url = storefrontUrlForPage(
    'http://localhost:3000',
    'about',
    'en',
    'published'
  );
  assert.equal(url, 'http://localhost:3000/en/pages/about');

  const urlRu = storefrontUrlForPage(
    'http://localhost:3000/',
    'contacts',
    'ru',
    'published'
  );
  assert.equal(urlRu, 'http://localhost:3000/ru/pages/contacts');

  // Trims leading slashes from slug
  const urlLeadingSlash = storefrontUrlForPage(
    'http://localhost:3000',
    '/faq',
    'en',
    'published'
  );
  assert.equal(urlLeadingSlash, 'http://localhost:3000/en/pages/faq');
});

test('storefrontUrlForPage has no storefront URL for a page that is not published', () => {
  // The storefront renders the published landing only; the working copy is
  // previewed in the builder.
  for (const status of ['draft', 'archived']) {
    assert.equal(
      storefrontUrlForPage('http://localhost:3000', 'promo', 'en', status),
      null
    );
  }
});

test('storefrontUrlForPage defaults to the published English route', () => {
  assert.equal(
    storefrontUrlForPage('http://localhost:3000', 'promo'),
    'http://localhost:3000/en/pages/promo'
  );
});

test('pagesNavItems specifies correct navigation structure and shortcuts', () => {
  assert.equal(pagesNavItems.length, 1);
  const root = pagesNavItems[0];
  assert.equal(root.title, 'Pages');
  assert.equal(root.group, 'modulePlugins');
  assert.equal(root.icon, 'page');
  assert.equal(root.items.length, 1);
  assert.equal(root.items[0].title, 'All Pages');
  assert.equal(root.items[0].url, '/dashboard/pages');
  assert.deepEqual(root.items[0].shortcut, ['p', 'l']);
});

test('GraphQL queries and mutations expose required schema fields', () => {
  const api = createApiModule(async () => ({}));
  assert.match(
    api.PAGES_LIST_QUERY,
    /pages\(filter: \$filter, tenantId: \$tenantId\)/
  );
  assert.match(api.PAGES_LIST_QUERY, /ListGqlPagesFilter/);
  assert.match(
    api.CREATE_PAGE_MUTATION,
    /createPage\(input: \$input, tenantId: \$tenantId\)/
  );
  assert.match(api.CREATE_PAGE_MUTATION, /CreateGqlPageInput!/);
  assert.match(
    api.UNPUBLISH_PAGE_MUTATION,
    /unpublishPage\(id: \$id, expectedVersion: \$expectedVersion, tenantId: \$tenantId\)/
  );
  assert.match(
    api.DELETE_PAGE_MUTATION,
    /deletePage\(id: \$id, tenantId: \$tenantId\)/
  );
});

test('listPages clamps pagination bounds and forwards tenant context', async () => {
  let capturedVars = null;
  let capturedToken = null;
  let capturedSlug = null;

  const api = createApiModule(async (query, vars, token, slug) => {
    capturedVars = vars;
    capturedToken = token;
    capturedSlug = slug;
    return {
      pages: {
        items: [
          {
            id: 'page-1',
            status: 'draft',
            template: 'landing',
            title: 'Welcome',
            slug: 'welcome',
            channelSlugs: ['default'],
            updatedAt: '2026-10-31T12:00:00Z'
          }
        ],
        total: 1
      }
    };
  });

  const res = await api.listPages(
    { locale: 'en', page: -2, perPage: 250 },
    { token: 'jwt-token', tenantSlug: 'demo', tenantId: 'tenant-1' }
  );

  assert.equal(res.items.length, 1);
  assert.equal(res.total, 1);
  assert.deepEqual(capturedVars, {
    filter: { locale: 'en', page: 1, perPage: 100 },
    tenantId: 'tenant-1'
  });
  assert.equal(capturedToken, 'jwt-token');
  assert.equal(capturedSlug, 'demo');
});

test('createPage enforces non-empty title/locale and enforces publish=false', async () => {
  let capturedVars = null;

  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      createPage: {
        id: 'p-1',
        version: 1,
        status: 'draft',
        template: 'default',
        translation: { locale: 'en', title: 'Home', slug: 'home' }
      }
    };
  });

  await assert.rejects(
    async () => api.createPage({ locale: 'en', title: '   ' }),
    /Page title cannot be empty/
  );
  await assert.rejects(
    async () => api.createPage({ locale: '  ', title: 'Valid Title' }),
    /Page locale cannot be empty/
  );

  const created = await api.createPage(
    { locale: 'en', title: 'About Us', slug: 'about-us' },
    { tenantId: 'tenant-1' }
  );

  assert.equal(created.id, 'p-1');
  assert.equal(capturedVars.input.publish, false);
  assert.equal(capturedVars.input.checkedLayout, true);
  assert.equal(capturedVars.input.translations[0].title, 'About Us');
  assert.equal(capturedVars.input.translations[0].locale, 'en');
});

test('unpublishPage and deletePage enforce non-empty page id', async () => {
  const api = createApiModule(async () => ({
    unpublishPage: { id: 'p-1', version: 2, status: 'draft' },
    deletePage: true
  }));

  await assert.rejects(
    async () => api.unpublishPage(''),
    /Page id cannot be empty/
  );
  await assert.rejects(
    async () => api.unpublishPage('   '),
    /Page id cannot be empty/
  );
  await assert.rejects(
    async () => api.deletePage(''),
    /Page id cannot be empty/
  );
  await assert.rejects(
    async () => api.deletePage('   '),
    /Page id cannot be empty/
  );

  const unpublished = await api.unpublishPage('p-1', 1);
  assert.equal(unpublished.status, 'draft');

  const deleted = await api.deletePage('p-1');
  assert.equal(deleted, true);
});

test('starter creation uses the selected tenant revision and sends the cloned document', async () => {
  const starter = {
    key: 'team', name: 'Team', category: 'about', description: null,
    defaultTemplateKey: 'default', revision: 'revision-1'
  };
  const document = { flyDocumentKind: 'page', pages: [{ id: 'page-1' }] };
  const calls = [];
  const api = createApiModule(async (query, vars, token, slug) => {
    calls.push({ query, vars, token, slug });
    if (query === api.PAGE_STARTERS_QUERY) return { pageStarters: [starter] };
    if (query === api.INSTANTIATE_PAGE_STARTER_MUTATION) return { instantiatePageStarter: JSON.stringify(document) };
    return { createPage: { id: 'p-1', version: 1, status: 'draft', template: 'default', translation: null } };
  });
  const opts = { tenantId: 'tenant-1', tenantSlug: 'demo', token: 'token' };
  const catalog = await api.listPageStarters(opts);
  const cloned = await api.instantiatePageStarter(catalog[0], 'en', opts);
  await api.createPage({ title: 'Team', locale: 'en', document: cloned }, opts);
  assert.deepEqual(calls[1].vars, { key: 'team', expectedRevision: 'revision-1', locale: 'en', tenantId: 'tenant-1' });
  assert.equal(calls[1].slug, 'demo');
  assert.equal(calls[1].token, 'token');
  assert.deepEqual(calls[2].vars.input.body, { locale: 'en', document });
  assert.equal(calls[2].vars.input.publish, false);
});

test('starter instantiation rejects a non-object response', async () => {
  const api = createApiModule(async () => ({ instantiatePageStarter: '[]' }));
  await assert.rejects(
    () => api.instantiatePageStarter({ key: 'blank', revision: null }, 'en'),
    /invalid document/
  );
});


test('save starter from page forwards exact source locale and tenant and returns the saved revision', async () => {
  const calls = [];
  const snapshot = { sourceLocale: 'ru', sourceState: 'draft', sourceRevision: 'rev' };
  const api = createApiModule(async (...args) => { calls.push(args); return { savePageStarterFromPage: snapshot }; });
  const input = { pageId: 'p', locale: 'ru', key: 'example', name: 'Example', category: 'custom' };
  const result = await api.saveStarterFromPage(input, { token: 'token', tenantSlug: 'demo', tenantId: 'tenant' });
  assert.deepEqual(result, snapshot);
  assert.equal(calls[0][0], api.SAVE_STARTER_FROM_PAGE_MUTATION);
  assert.deepEqual(calls[0][1], { input, tenantId: 'tenant' });
  assert.equal(calls[0][2], 'token');
  assert.equal(calls[0][3], 'demo');
});


test('starter metadata and delete send the observed tenant revision and do not send a document', async () => {
  const calls = [];
  const api = createApiModule(async (query, vars, token, slug) => {
    calls.push({ query, vars, token, slug });
    return query === api.UPDATE_STARTER_METADATA_MUTATION
      ? { updatePageStarterMetadata: 'revision-2' }
      : { deletePageStarter: true };
  });
  const opts = { tenantId: 'tenant', tenantSlug: 'demo', token: 'token' };
  const input = { key: 'team', name: 'Team', category: 'custom', description: null, defaultTemplateKey: 'default', expectedRevision: 'revision-1' };
  assert.equal(await api.updateStarterMetadata(input, opts), 'revision-2');
  assert.equal(await api.deleteStarter('team', 'revision-2', opts), true);
  assert.deepEqual(calls[0].vars, { input, tenantId: 'tenant' });
  assert.ok(!('contentJson' in calls[0].vars.input));
  assert.deepEqual(calls[1].vars, { key: 'team', expectedRevision: 'revision-2', tenantId: 'tenant' });
  assert.equal(calls[1].token, 'token');
  assert.equal(calls[1].slug, 'demo');
  await assert.rejects(() => api.updateStarterMetadata({ ...input, expectedRevision: '' }, opts), /revision/);
  await assert.rejects(() => api.deleteStarter('team', '', opts), /revision/);
  assert.equal(calls.length, 2);
});


test('layout catalog request uses exact locale and tenant and rejects malformed results', async () => {
  const calls = [];
  const api = createApiModule(async (...args) => { calls.push(args); return { pageTemplateKeys: ['default', 'site'] }; });
  const opts = { token: 'token', tenantId: 'tenant', tenantSlug: 'demo' };
  assert.deepEqual(await api.listPageLayoutKeys('ru', opts), ['default', 'site']);
  assert.equal(calls[0][0], api.PAGE_LAYOUT_KEYS_QUERY);
  assert.deepEqual(calls[0][1], { locale: 'ru', tenantId: 'tenant' });
  assert.equal(calls[0][2], 'token');
  assert.equal(calls[0][3], 'demo');
  const broken = createApiModule(async () => ({ pageTemplateKeys: ['default', null] }));
  await assert.rejects(() => broken.listPageLayoutKeys('ru', opts), /Invalid layout catalog/);
  await assert.rejects(() => api.listPageLayoutKeys(' ', opts), /locale/);
});

test('starter flow uses the explicit layout and checks the fresh catalog before create', async () => {
  const calls = [];
  const starter = { key: 'team', name: 'Team', category: 'about', description: null, defaultTemplateKey: 'suggested', revision: 'rev' };
  const input = { starter, locale: 'ru', title: 'Team', layoutKey: 'site', visibleLayoutKeys: ['default', 'site'] };
  const document = { pages: [] };
  const ports = {
    contextMatches: () => true,
    instantiate: async (item, locale) => { calls.push(['instantiate', item, locale]); return document; },
    listLayouts: async (locale) => { calls.push(['layouts', locale]); return ['default', 'site']; },
    create: async (page) => { calls.push(['create', page]); }
  };
  const messages = { layout: 'layout error', context: 'context error', contextAfterCreate: 'context changed after create' };
  await createPageFromStarter(input, ports, messages);
  assert.deepEqual(calls, [ ['instantiate', starter, 'ru'], ['layouts', 'ru'], ['create', { locale: 'ru', title: 'Team', template: 'site', document }] ]);
  calls.length = 0;
  await assert.rejects(() => createPageFromStarter({ ...input, layoutKey: 'missing' }, ports, messages), LayoutSelectionError);
  assert.equal(calls.length, 0);
  ports.listLayouts = async () => { calls.push(['layouts']); return ['default']; };
  await assert.rejects(() => createPageFromStarter(input, ports, messages), LayoutSelectionError);
  assert.deepEqual(calls.map((entry) => entry[0]), ['instantiate', 'layouts']);
});

test('starter flow stops on catalog errors or a changed auth/locale context', async () => {
  const starter = { key: 'blank', revision: null };
  const input = { starter, locale: 'en', title: 'Page', layoutKey: 'default', visibleLayoutKeys: ['default'] };
  const messages = { layout: 'layout error', context: 'context error', contextAfterCreate: 'context changed after create' };
  let created = 0, active = true;
  const ports = {
    contextMatches: () => active,
    instantiate: async () => ({}),
    listLayouts: async () => { throw new Error('catalog offline'); },
    create: async () => { created++; }
  };
  await assert.rejects(() => createPageFromStarter(input, ports, messages), /catalog offline/);
  ports.instantiate = async () => { active = false; return {}; };
  await assert.rejects(() => createPageFromStarter(input, ports, messages), /context error/);
  assert.equal(created, 0);
  active = true;
  ports.instantiate = async () => ({});
  ports.listLayouts = async () => { active = false; return ['default']; };
  await assert.rejects(() => createPageFromStarter(input, ports, messages), /context error/);
  assert.equal(created, 0);
});


test('starter flow reports a possibly completed create when context changes during the write', async () => {
  let active = true;
  const ports = {
    contextMatches: () => active,
    instantiate: async () => ({}),
    listLayouts: async () => ['default'],
    create: async () => { active = false; }
  };
  const input = { starter: { key: 'blank', revision: null }, locale: 'en', title: 'Page', layoutKey: 'default', visibleLayoutKeys: ['default'] };
  await assert.rejects(() => createPageFromStarter(input, ports, { layout: 'layout', context: 'context', contextAfterCreate: 'may be created' }), /may be created/);
});
