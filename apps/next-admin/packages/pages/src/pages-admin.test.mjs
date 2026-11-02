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

// Load api/pages.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/pages.ts', import.meta.url),
  'utf8'
);
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace("import { graphqlRequest } from '@/lib/graphql';", '')
    .replace("export { builderUrlForPage, storefrontUrlForPage } from './urls';", '')
    .replaceAll('export ', '')
);

const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      PAGE_LIST_FIELDS,
      PAGES_LIST_QUERY,
      CREATE_PAGE_MUTATION,
      UNPUBLISH_PAGE_MUTATION,
      DELETE_PAGE_MUTATION,
      listPages,
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
compileFunction(
  `${cleanNavSource}\nnavModule.pagesNavItems = pagesNavItems;`,
  ['navModule']
)(navModule);
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

test('storefrontUrlForPage constructs draft preview link with secret for draft pages', () => {
  const url = storefrontUrlForPage(
    'http://localhost:3000',
    'spring-sale',
    'ru',
    'draft',
    'secret123'
  );
  assert.equal(
    url,
    'http://localhost:3000/api/draft?slug=spring-sale&locale=ru&secret=secret123'
  );
});

test('storefrontUrlForPage falls back to canonical URL when draft has no secret', () => {
  const url = storefrontUrlForPage(
    'http://localhost:3000',
    'promo',
    'en',
    'draft'
  );
  assert.equal(url, 'http://localhost:3000/en/pages/promo');
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
  assert.match(api.PAGES_LIST_QUERY, /pages\(filter: \$filter, tenantId: \$tenantId\)/);
  assert.match(api.PAGES_LIST_QUERY, /ListGqlPagesFilter/);
  assert.match(api.CREATE_PAGE_MUTATION, /createPage\(input: \$input, tenantId: \$tenantId\)/);
  assert.match(api.CREATE_PAGE_MUTATION, /CreateGqlPageInput!/);
  assert.match(api.UNPUBLISH_PAGE_MUTATION, /unpublishPage\(id: \$id, expectedVersion: \$expectedVersion, tenantId: \$tenantId\)/);
  assert.match(api.DELETE_PAGE_MUTATION, /deletePage\(id: \$id, tenantId: \$tenantId\)/);
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
  assert.equal(capturedVars.input.translations[0].title, 'About Us');
  assert.equal(capturedVars.input.translations[0].locale, 'en');
});

test('unpublishPage and deletePage enforce non-empty page id', async () => {
  const api = createApiModule(async () => ({
    unpublishPage: { id: 'p-1', version: 2, status: 'draft' },
    deletePage: true
  }));

  await assert.rejects(async () => api.unpublishPage(''), /Page id cannot be empty/);
  await assert.rejects(async () => api.unpublishPage('   '), /Page id cannot be empty/);
  await assert.rejects(async () => api.deletePage(''), /Page id cannot be empty/);
  await assert.rejects(async () => api.deletePage('   '), /Page id cannot be empty/);

  const unpublished = await api.unpublishPage('p-1', 1);
  assert.equal(unpublished.status, 'draft');

  const deleted = await api.deletePage('p-1');
  assert.equal(deleted, true);
});
