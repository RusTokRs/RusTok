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

// Load api/media.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/media.ts', import.meta.url),
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
      formatBytes,
      MEDIA_LIST_QUERY,
      MEDIA_USAGE_QUERY,
      DELETE_MEDIA_MUTATION,
      listMedia,
      mediaUsage,
      deleteMedia
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
  `${cleanNavSource}\nnavModule.mediaNavItems = mediaNavItems;`,
  ['navModule']
)(navModule);
const { mediaNavItems } = navModule;

test('mediaNavItems specifies correct dashboard route and plugin group', () => {
  assert.equal(mediaNavItems.length, 1);
  const root = mediaNavItems[0];
  assert.equal(root.title, 'Media');
  assert.equal(root.url, '/dashboard/media');
  assert.equal(root.group, 'modulePlugins');
  assert.equal(root.icon, 'media');
});

test('formatBytes handles boundary conditions and units', () => {
  const api = createApiModule(async () => ({}));
  assert.equal(api.formatBytes(-50), '0 B');
  assert.equal(api.formatBytes(0), '0 B');
  assert.equal(api.formatBytes(512), '512 B');
  assert.equal(api.formatBytes(1024), '1.0 KB');
  assert.equal(api.formatBytes(1536), '1.5 KB');
  assert.equal(api.formatBytes(1024 * 1024), '1.0 MB');
  assert.equal(api.formatBytes(2.5 * 1024 * 1024), '2.5 MB');
  assert.equal(api.formatBytes(1024 * 1024 * 1024), '1.0 GB');
  assert.equal(api.formatBytes(3.2 * 1024 * 1024 * 1024), '3.2 GB');
});

test('GraphQL documents expose required schema queries and mutations', () => {
  const api = createApiModule(async () => ({}));
  assert.match(api.MEDIA_LIST_QUERY, /query MediaList\(\$tenantId: UUID!, \$pagination: PaginationInput!\)/);
  assert.match(api.MEDIA_LIST_QUERY, /media\(tenantId: \$tenantId, pagination: \$pagination\)/);
  assert.match(api.MEDIA_USAGE_QUERY, /query MediaUsage\(\$tenantId: UUID!\)/);
  assert.match(api.MEDIA_USAGE_QUERY, /mediaUsage\(tenantId: \$tenantId\)/);
  assert.match(api.DELETE_MEDIA_MUTATION, /mutation DeleteMedia\(\$tenantId: UUID!, \$id: UUID!\)/);
  assert.match(api.DELETE_MEDIA_MUTATION, /deleteMedia\(tenantId: \$tenantId, id: \$id\)/);
});

test('listMedia enforces tenant context and clamps pagination bounds', async () => {
  let calledQuery = null;
  let calledVars = null;
  let calledToken = null;
  let calledSlug = null;

  const api = createApiModule(async (query, vars, token, slug) => {
    calledQuery = query;
    calledVars = vars;
    calledToken = token;
    calledSlug = slug;
    return {
      media: {
        items: [
          {
            id: 'media-1',
            tenantId: '00000000-0000-0000-0000-000000000001',
            ownerModule: 'blog',
            filename: 'header.png',
            originalName: 'header.png',
            mimeType: 'image/png',
            size: 2048,
            publicUrl: 'https://cdn.example.com/header.png',
            width: 1920,
            height: 1080,
            createdAt: '2026-10-31T00:00:00Z'
          }
        ],
        total: 1
      }
    };
  });

  // Enforces tenantId
  await assert.rejects(
    async () => api.listMedia({}),
    /Media queries require a tenant id in the session/
  );

  // Normal call
  const result = await api.listMedia(
    { tenantId: '00000000-0000-0000-0000-000000000001', token: 'jwt-123', tenantSlug: 'demo' },
    { page: 2, perPage: 10 }
  );

  assert.equal(result.items.length, 1);
  assert.equal(result.total, 1);
  assert.deepEqual(calledVars, {
    tenantId: '00000000-0000-0000-0000-000000000001',
    pagination: { offset: 10, limit: 10 }
  });
  assert.equal(calledToken, 'jwt-123');
  assert.equal(calledSlug, 'demo');

  // Clamps negative / excessive pagination
  await api.listMedia(
    { tenantId: '00000000-0000-0000-0000-000000000001' },
    { page: -5, perPage: 500 }
  );
  assert.deepEqual(calledVars.pagination, { offset: 0, limit: 100 });
});

test('mediaUsage returns aggregated counts and bytes', async () => {
  const api = createApiModule(async (query, vars) => {
    assert.deepEqual(vars, { tenantId: 't-1' });
    return {
      mediaUsage: { fileCount: 42, totalBytes: 1048576 }
    };
  });

  await assert.rejects(
    async () => api.mediaUsage({}),
    /Media queries require a tenant id in the session/
  );

  const usage = await api.mediaUsage({ tenantId: 't-1' });
  assert.equal(usage.fileCount, 42);
  assert.equal(usage.totalBytes, 1048576);
});

test('deleteMedia dispatches deletion mutation with tenantId and asset ID', async () => {
  const api = createApiModule(async (query, vars) => {
    assert.deepEqual(vars, { tenantId: 't-1', id: 'asset-99' });
    return { deleteMedia: true };
  });

  await assert.rejects(
    async () => api.deleteMedia('asset-99', {}),
    /Media queries require a tenant id in the session/
  );

  const ok = await api.deleteMedia('asset-99', { tenantId: 't-1' });
  assert.equal(ok, true);
});
