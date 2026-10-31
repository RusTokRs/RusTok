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

// Load api/forms.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/forms.ts', import.meta.url),
  'utf8'
);
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace("import { graphqlRequest } from '@/lib/graphql';", '')
    .replaceAll('export ', '')
);

const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      FORM_SUBMISSIONS_QUERY,
      UPDATE_FORM_SUBMISSION_STATE_MUTATION,
      listFormSubmissions,
      updateFormSubmissionState
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
  `${cleanNavSource}\nnavModule.formsNavItems = formsNavItems;`,
  ['navModule']
)(navModule);
const { formsNavItems } = navModule;

test('formsNavItems specifies correct dashboard route and plugin group', () => {
  assert.equal(formsNavItems.length, 1);
  const root = formsNavItems[0];
  assert.equal(root.title, 'Forms');
  assert.equal(root.url, '/dashboard/forms');
  assert.equal(root.group, 'modulePlugins');
  assert.equal(root.icon, 'post');
});

test('GraphQL queries and mutations expose required schema fields and async-graphql types', () => {
  const api = createApiModule(async () => ({}));

  // Query fields
  assert.match(api.FORM_SUBMISSIONS_QUERY, /formSubmissions\(filter: \$filter\)/);
  assert.match(api.FORM_SUBMISSIONS_QUERY, /ListGqlFormSubmissionsFilter/);
  assert.match(api.FORM_SUBMISSIONS_QUERY, /id formId locale pageId payload state createdAt handledAt/);

  // Mutation fields
  assert.match(api.UPDATE_FORM_SUBMISSION_STATE_MUTATION, /updateFormSubmissionState\(id: \$id, input: \$input\)/);
  assert.match(api.UPDATE_FORM_SUBMISSION_STATE_MUTATION, /UpdateGqlFormSubmissionStateInput/);
  assert.match(api.UPDATE_FORM_SUBMISSION_STATE_MUTATION, /id formId locale pageId payload state createdAt handledAt/);
});

test('listFormSubmissions passes filter arguments and auth context to gateway', async () => {
  const mockSubmissions = [
    {
      id: 'sub-1',
      formId: 'contact-us',
      locale: 'en',
      pageId: 'page-10',
      payload: { email: 'user@example.com', message: 'Hello' },
      state: 'new',
      createdAt: '2026-10-31T10:00:00Z',
      handledAt: null
    }
  ];

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /FormSubmissions/);
    assert.deepEqual(vars, {
      filter: {
        formId: 'contact-us',
        state: 'new',
        page: 2,
        perPage: 10
      }
    });
    assert.equal(token, 'auth-token');
    assert.equal(tenant, 'my-tenant');
    return { formSubmissions: mockSubmissions };
  });

  const res = await api.listFormSubmissions(
    { formId: 'contact-us', state: 'new', page: 2, perPage: 10 },
    { token: 'auth-token', tenantSlug: 'my-tenant' }
  );

  assert.deepEqual(res, mockSubmissions);
});

test('updateFormSubmissionState passes id, input, and auth context to gateway', async () => {
  const updatedSubmission = {
    id: 'sub-1',
    formId: 'contact-us',
    locale: 'en',
    pageId: 'page-10',
    payload: { email: 'user@example.com' },
    state: 'handled',
    createdAt: '2026-10-31T10:00:00Z',
    handledAt: '2026-10-31T10:05:00Z'
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /UpdateFormSubmissionState/);
    assert.deepEqual(vars, {
      id: 'sub-1',
      input: { state: 'handled' }
    });
    assert.equal(token, 'secret-token');
    assert.equal(tenant, 'tenant-slug');
    return { updateFormSubmissionState: updatedSubmission };
  });

  const res = await api.updateFormSubmissionState(
    'sub-1',
    'handled',
    { token: 'secret-token', tenantSlug: 'tenant-slug' }
  );

  assert.deepEqual(res, updatedSubmission);
  assert.equal(res.state, 'handled');
});

