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

// Load api/cache.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/cache.ts', import.meta.url),
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
      DEFAULT_CACHE_SETTINGS,
      CACHE_HEALTH_QUERY,
      PLATFORM_SETTINGS_QUERY,
      UPDATE_PLATFORM_SETTINGS_MUTATION,
      getCacheHealth,
      getCacheSettings,
      updateCacheSettings
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
compileFunction(`${cleanNavSource}\nnavModule.cacheNavItems = cacheNavItems;`, [
  'navModule'
])(navModule);
const { cacheNavItems } = navModule;

test('cacheNavItems specifies admin access and correct dashboard route', () => {
  assert.equal(cacheNavItems.length, 1);
  const root = cacheNavItems[0];
  assert.equal(root.title, 'System');
  assert.equal(root.url, '#');
  assert.deepEqual(root.access, { role: 'admin' });
  assert.equal(root.items.length, 1);
  assert.equal(root.items[0].title, 'Cache');
  assert.equal(root.items[0].url, '/dashboard/cache');
});

test('DEFAULT_CACHE_SETTINGS defines expected fallbacks', () => {
  const api = createApiModule(async () => ({}));
  assert.equal(api.DEFAULT_CACHE_SETTINGS.mode, 'in-memory');
  assert.equal(api.DEFAULT_CACHE_SETTINGS.redis_url, '');
  assert.equal(api.DEFAULT_CACHE_SETTINGS.redis_host, '127.0.0.1');
  assert.equal(api.DEFAULT_CACHE_SETTINGS.redis_port, 6379);
  assert.equal(api.DEFAULT_CACHE_SETTINGS.redis_password, '');
  assert.equal(api.DEFAULT_CACHE_SETTINGS.redis_db, 0);
});

test('GraphQL queries and mutations expose required schema fields', () => {
  const api = createApiModule(async () => ({}));

  // Health query fields
  assert.match(api.CACHE_HEALTH_QUERY, /cacheHealth/);
  assert.match(api.CACHE_HEALTH_QUERY, /redisConfigured/);
  assert.match(api.CACHE_HEALTH_QUERY, /redisHealthy/);
  assert.match(api.CACHE_HEALTH_QUERY, /redisError/);
  assert.match(api.CACHE_HEALTH_QUERY, /backend/);

  // Platform settings query fields
  assert.match(
    api.PLATFORM_SETTINGS_QUERY,
    /platformSettings\(category: \$category\)/
  );
  assert.match(api.PLATFORM_SETTINGS_QUERY, /category/);
  assert.match(api.PLATFORM_SETTINGS_QUERY, /settings/);
  assert.match(api.PLATFORM_SETTINGS_QUERY, /schemaVersion/);

  // Mutation fields
  assert.match(
    api.UPDATE_PLATFORM_SETTINGS_MUTATION,
    /updatePlatformSettings\(input: \$input\)/
  );
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /success/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /category/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /settings/);
  assert.match(api.UPDATE_PLATFORM_SETTINGS_MUTATION, /schemaVersion/);
});

test('getCacheHealth propagates token and tenantSlug and returns health payload', async () => {
  const mockHealth = {
    redisConfigured: true,
    redisHealthy: true,
    redisError: null,
    backend: 'redis'
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /cacheHealth/);
    assert.deepEqual(vars, {});
    assert.equal(token, 'auth-token');
    assert.equal(tenant, 'tenant-alpha');
    return { cacheHealth: mockHealth };
  });

  const res = await api.getCacheHealth({
    token: 'auth-token',
    tenantSlug: 'tenant-alpha'
  });
  assert.deepEqual(res, mockHealth);
});

test('getCacheSettings parses valid JSON and merges with defaults', async () => {
  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /platformSettings/);
    assert.deepEqual(vars, { category: 'cache' });
    assert.equal(token, 'token-123');
    assert.equal(tenant, 'my-tenant');
    return {
      platformSettings: {
        category: 'cache',
        settings: JSON.stringify({
          mode: 'redis',
          redis_host: '10.0.0.5',
          redis_port: 6380
        }),
        schemaVersion: 7
      }
    };
  });

  const res = await api.getCacheSettings({
    token: 'token-123',
    tenantSlug: 'my-tenant'
  });

  assert.equal(res.schemaVersion, 7);
  assert.equal(res.settings.mode, 'redis');
  assert.equal(res.settings.redis_host, '10.0.0.5');
  assert.equal(res.settings.redis_port, 6380);
  assert.equal(res.settings.redis_db, 0); // Preserved from defaults
  assert.equal(res.settings.redis_password, ''); // Preserved from defaults
});

test('getCacheSettings falls back to DEFAULT_CACHE_SETTINGS on invalid JSON', async () => {
  const api = createApiModule(async () => ({
    platformSettings: {
      category: 'cache',
      settings: 'invalid-json-content{',
      schemaVersion: 4
    }
  }));

  const res = await api.getCacheSettings();
  assert.deepEqual(res, {
    settings: api.DEFAULT_CACHE_SETTINGS,
    schemaVersion: 4
  });
});

test('updateCacheSettings serializes payload and returns merged settings', async () => {
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
        category: 'cache',
        settings: JSON.stringify({
          mode: 'hybrid',
          redis_host: 'redis.internal',
          redis_port: 6379,
          redis_db: 2
        }),
        schemaVersion: 9
      }
    };
  };

  const api = createApiModule(mockGraphqlRequest);
  const updated = await api.updateCacheSettings(
    { mode: 'hybrid', redis_host: 'redis.internal', redis_db: 2 },
    8,
    { token: 'secret-token', tenantSlug: 'tenant-beta' }
  );

  assert.equal(updated.schemaVersion, 9);
  assert.equal(updated.settings.mode, 'hybrid');
  assert.equal(updated.settings.redis_host, 'redis.internal');
  assert.equal(updated.settings.redis_db, 2);
  assert.equal(updated.settings.redis_port, 6379);

  assert.match(calledQuery, /updatePlatformSettings/);
  assert.equal(calledVars.input.category, 'cache');
  assert.equal(calledVars.input.expectedSchemaVersion, 8);
  assert.deepEqual(JSON.parse(calledVars.input.settings), {
    mode: 'hybrid',
    redis_host: 'redis.internal',
    redis_db: 2
  });
  assert.equal(calledToken, 'secret-token');
  assert.equal(calledTenant, 'tenant-beta');
});

test('updateCacheSettings returns defaults if backend returns unparseable response', async () => {
  const api = createApiModule(async () => ({
    updatePlatformSettings: {
      success: true,
      category: 'cache',
      settings: 'broken-json',
      schemaVersion: 6
    }
  }));

  const res = await api.updateCacheSettings({ mode: 'redis' }, 5);
  assert.deepEqual(res, {
    settings: api.DEFAULT_CACHE_SETTINGS,
    schemaVersion: 6
  });
});

test('updateCacheSettings propagates a stale-revision conflict without replacing the snapshot', async () => {
  const conflict = new Error(
    'CONFLICT: Settings were changed concurrently; re-read the category and retry'
  );
  const api = createApiModule(async (_query, variables) => {
    assert.equal(variables.input.expectedSchemaVersion, 7);
    throw conflict;
  });

  await assert.rejects(
    api.updateCacheSettings({ mode: 'redis' }, 7),
    (error) => error === conflict
  );
});

test('cache settings reads reject a missing revision instead of inventing a CAS baseline', async () => {
  const api = createApiModule(async () => ({
    platformSettings: { category: 'cache', settings: '{}' }
  }));

  await assert.rejects(api.getCacheSettings(), TypeError);
});
