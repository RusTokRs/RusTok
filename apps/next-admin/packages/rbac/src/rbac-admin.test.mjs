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

// Load api/roles.ts by injecting mock graphqlRequest
const apiSource = readFileSync(
  new URL('./api/roles.ts', import.meta.url),
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
      ROLES_QUERY,
      PLATFORM_PERMISSIONS_QUERY,
      CREATE_ROLE_MUTATION,
      UPDATE_ROLE_MUTATION,
      DELETE_ROLE_MUTATION,
      ASSIGN_USER_ROLE_MUTATION,
      listRoles,
      listPlatformPermissions,
      createRole,
      updateRole,
      deleteRole,
      assignUserRole
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
  `${cleanNavSource}\nnavModule.rbacNavItems = rbacNavItems;`,
  ['navModule']
)(navModule);
const { rbacNavItems } = navModule;

test('rbacNavItems specifies correct dashboard route, shortcut, and admin access requirement', () => {
  assert.equal(rbacNavItems.length, 1);
  const root = rbacNavItems[0];
  assert.equal(root.title, 'Access Control');
  assert.deepEqual(root.access, { role: 'admin' });
  assert.equal(root.items.length, 1);
  const sub = root.items[0];
  assert.equal(sub.title, 'Roles & Permissions');
  assert.equal(sub.url, '/dashboard/roles');
  assert.deepEqual(sub.shortcut, ['r', 'p']);
});

test('GraphQL queries and mutations expose required RBAC schema types and operations', () => {
  const api = createApiModule(async () => ({}));
  assert.match(api.ROLES_QUERY, /query Roles/);
  assert.match(
    api.ROLES_QUERY,
    /slug[\s\S]+displayName[\s\S]+permissions[\s\S]+inactivePermissions/
  );
  assert.match(api.PLATFORM_PERMISSIONS_QUERY, /query PlatformPermissions/);
  assert.match(
    api.PLATFORM_PERMISSIONS_QUERY,
    /resource[\s\S]+action[\s\S]+moduleSlug[\s\S]+moduleStatus/
  );
  assert.match(
    api.CREATE_ROLE_MUTATION,
    /mutation CreateRole\(\$input: CreateRoleInput!\)/
  );
  assert.match(
    api.UPDATE_ROLE_MUTATION,
    /mutation UpdateRole\(\$input: UpdateRoleInput!\)/
  );
  assert.match(
    api.DELETE_ROLE_MUTATION,
    /mutation DeleteRole\(\$slug: String!\)/
  );
  assert.match(
    api.ASSIGN_USER_ROLE_MUTATION,
    /mutation AssignUserRole\(\$input: AssignUserRoleInput!\)/
  );
});

test('createRole validates input bounds and rejects reserved system role slugs', async () => {
  let capturedVars = null;
  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      createRole: {
        success: true,
        role: {
          slug: vars.input.slug,
          displayName: vars.input.name,
          permissions: vars.input.permissions
        }
      }
    };
  });

  // Rejects empty or whitespace name
  await assert.rejects(
    async () => api.createRole({ name: '', slug: 'moderator', permissions: [] }),
    /Role name is required/
  );
  await assert.rejects(
    async () => api.createRole({ name: '   ', slug: 'moderator', permissions: [] }),
    /Role name is required/
  );

  // Rejects short slug
  await assert.rejects(
    async () => api.createRole({ name: 'Editor', slug: 'e', permissions: [] }),
    /Role slug must be at least 2 characters/
  );

  // Rejects reserved built-in slugs
  for (const reserved of ['super_admin', 'admin', 'manager', 'customer']) {
    await assert.rejects(
      async () => api.createRole({ name: 'My Role', slug: reserved, permissions: [] }),
      new RegExp(`'${reserved}' is a reserved platform built-in role slug`)
    );
  }

  // Successfully normalizes valid input
  const res = await api.createRole({
    name: '  Content Moderator  ',
    slug: '  CONTENT_MODERATOR  ',
    description: '  Handles forum flags  ',
    permissions: ['forum:read', 'forum:moderate']
  });

  assert.equal(res.success, true);
  assert.deepEqual(capturedVars, {
    input: {
      name: 'Content Moderator',
      slug: 'content_moderator',
      description: 'Handles forum flags',
      permissions: ['forum:read', 'forum:moderate']
    }
  });
});

test('updateRole validates slug and preserves super_admin permission immutability', async () => {
  let capturedVars = null;
  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      updateRole: {
        success: true,
        role: { slug: vars.input.slug, displayName: vars.input.name ?? '' }
      }
    };
  });

  await assert.rejects(
    async () => api.updateRole({ slug: '   ' }),
    /Role slug is required/
  );

  // When updating super_admin, permissions MUST be undefined to prevent revoking root permissions
  await api.updateRole({
    slug: 'super_admin',
    name: 'Root Administrator',
    permissions: [] // Attempting to clear permissions
  });
  assert.equal(capturedVars.input.permissions, undefined);
  assert.equal(capturedVars.input.name, 'Root Administrator');

  // Normal custom role update preserves permissions
  await api.updateRole({
    slug: 'moderator',
    name: 'Lead Moderator',
    permissions: ['content:delete']
  });
  assert.deepEqual(capturedVars.input.permissions, ['content:delete']);
});

test('deleteRole protects super_admin identity from deletion', async () => {
  let capturedVars = null;
  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      deleteRole: { success: true, slug: vars.slug }
    };
  });

  await assert.rejects(
    async () => api.deleteRole(''),
    /Role slug is required/
  );

  await assert.rejects(
    async () => api.deleteRole('super_admin'),
    /Cannot delete protected system role 'super_admin'/
  );

  await assert.rejects(
    async () => api.deleteRole('  SUPER_ADMIN  '),
    /Cannot delete protected system role 'super_admin'/
  );

  const res = await api.deleteRole('custom_role');
  assert.equal(res.success, true);
  assert.equal(capturedVars.slug, 'custom_role');
});

test('assignUserRole validates user UUID format and role slug', async () => {
  let capturedVars = null;
  const api = createApiModule(async (query, vars) => {
    capturedVars = vars;
    return {
      assignUserRole: {
        success: true,
        userId: vars.input.userId,
        role: vars.input.roleSlug
      }
    };
  });

  await assert.rejects(
    async () => api.assignUserRole({ userId: '', roleSlug: 'manager' }),
    /User ID is required/
  );

  await assert.rejects(
    async () => api.assignUserRole({ userId: 'not-a-valid-uuid', roleSlug: 'manager' }),
    /User ID must be a valid UUID/
  );

  await assert.rejects(
    async () =>
      api.assignUserRole({
        userId: '123e4567-e89b-12d3-a456-426614174000',
        roleSlug: '   '
      }),
    /Role slug is required/
  );

  const validUuid = '123e4567-e89b-12d3-a456-426614174000';
  const res = await api.assignUserRole({
    userId: `  ${validUuid}  `,
    roleSlug: '  MANAGER  '
  });

  assert.equal(res.success, true);
  assert.deepEqual(capturedVars, {
    input: {
      userId: validUuid,
      roleSlug: 'manager'
    }
  });
});
