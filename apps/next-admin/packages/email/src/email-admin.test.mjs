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

// Load api/email.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/email.ts', import.meta.url),
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
      DEFAULT_EMAIL_SETTINGS,
      PLATFORM_SETTINGS_QUERY,
      UPDATE_PLATFORM_SETTINGS_MUTATION,
      getEmailSettings,
      updateEmailSettings
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
  `${cleanNavSource}\nnavModule.emailNavItems = emailNavItems;`,
  ['navModule']
)(navModule);
const { emailNavItems } = navModule;

test('emailNavItems specifies admin access, shortcut, and correct dashboard route', () => {
  assert.equal(emailNavItems.length, 1);
  const root = emailNavItems[0];
  assert.equal(root.title, 'Platform Settings');
  assert.equal(root.url, '#');
  assert.equal(root.icon, 'settings');
  assert.deepEqual(root.access, { role: 'admin' });
  assert.equal(root.items.length, 1);
  assert.equal(root.items[0].title, 'Email');
  assert.equal(root.items[0].url, '/dashboard/email');
  assert.deepEqual(root.items[0].shortcut, ['e', 'm']);
});

test('DEFAULT_EMAIL_SETTINGS defines expected fallbacks', () => {
  const api = createApiModule(async () => ({}));
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.enabled, false);
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.provider, 'smtp');
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.from, 'noreply@example.com');
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.resetBaseUrl, 'https://example.com');
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.smtpHost, '');
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.smtpPort, 587);
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.smtpUsername, '');
  assert.equal(api.DEFAULT_EMAIL_SETTINGS.smtpPassword, '');
});

test('GraphQL queries and mutations expose required schema fields', () => {
  const api = createApiModule(async () => ({}));

  // Query fields
  assert.match(api.PLATFORM_SETTINGS_QUERY, /platformSettings\(category: \$category\)/);
  assert.match(api.PLATFORM_SETTINGS_QUERY, /category/);
  assert.match(api.PLATFORM_SETTINGS_QUERY, /settings/);

  // Mutation fields
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /updatePlatformSettings\(input: \$input\)/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /success/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /category/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /settings/);
});

test('getEmailSettings parses valid JSON, merges with defaults, and propagates auth headers', async () => {
  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /platformSettings/);
    assert.deepEqual(vars, { category: 'email' });
    assert.equal(token, 'auth-token-123');
    assert.equal(tenant, 'tenant-slug');
    return {
      platformSettings: {
        category: 'email',
        settings: JSON.stringify({
          enabled: true,
          smtpHost: 'mail.provider.org',
          smtpPort: 465,
          smtpUsername: 'sender@provider.org'
        })
      }
    };
  });

  const res = await api.getEmailSettings({
    token: 'auth-token-123',
    tenantSlug: 'tenant-slug'
  });

  assert.equal(res.enabled, true);
  assert.equal(res.smtpHost, 'mail.provider.org');
  assert.equal(res.smtpPort, 465);
  assert.equal(res.smtpUsername, 'sender@provider.org');
  assert.equal(res.from, 'noreply@example.com'); // Preserved from defaults
  assert.equal(res.resetBaseUrl, 'https://example.com'); // Preserved from defaults
});

test('getEmailSettings falls back to DEFAULT_EMAIL_SETTINGS on unparseable JSON', async () => {
  const api = createApiModule(async () => ({
    platformSettings: {
      category: 'email',
      settings: 'unparseable-data['
    }
  }));

  const res = await api.getEmailSettings();
  assert.deepEqual(res, api.DEFAULT_EMAIL_SETTINGS);
});

test('updateEmailSettings serializes payload and returns merged settings', async () => {
  let calledQuery = null;
  let calledVars = null;
  let calledToken = null;
  let calledTenant = null;

  const mockGraphqlRequest = async (query, vars, token, tenant) => {
    calledQuery = query;
    calledVars = vars;
    calledToken = token;
    calledTenant = tenant;
    return {
      updatePlatformSettings: {
        success: true,
        category: 'email',
        settings: JSON.stringify({
          enabled: true,
          from: 'admin@rustok.io',
          smtpHost: 'smtp.rustok.io',
          smtpPort: 587
        })
      }
    };
  };

  const api = createApiModule(mockGraphqlRequest);
  const updated = await api.updateEmailSettings(
    { enabled: true, from: 'admin@rustok.io', smtpHost: 'smtp.rustok.io' },
    { token: 'secret-token', tenantSlug: 'tenant-prod' }
  );

  assert.equal(updated.enabled, true);
  assert.equal(updated.from, 'admin@rustok.io');
  assert.equal(updated.smtpHost, 'smtp.rustok.io');
  assert.equal(updated.smtpPort, 587);

  assert.match(calledQuery, /updatePlatformSettings/);
  assert.equal(calledVars.input.category, 'email');
  assert.deepEqual(
    JSON.parse(calledVars.input.settings),
    { enabled: true, from: 'admin@rustok.io', smtpHost: 'smtp.rustok.io' }
  );
  assert.equal(calledToken, 'secret-token');
  assert.equal(calledTenant, 'tenant-prod');
});

test('updateEmailSettings returns defaults if backend returns unparseable response', async () => {
  const api = createApiModule(async () => ({
    updatePlatformSettings: {
      success: true,
      category: 'email',
      settings: 'corrupt-json'
    }
  }));

  const res = await api.updateEmailSettings({ enabled: true });
  assert.deepEqual(res, api.DEFAULT_EMAIL_SETTINGS);
});

