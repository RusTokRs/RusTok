import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
import React from 'react';
import * as jsx from 'react/jsx-runtime';

async function dashboard(fetchEnabledModules) {
  const calls = [];
  const component = () => null;
  const mocks = {
    'next/link': { default: component },
    react: { default: React },
    'next/navigation': {
      redirect: () => {
        throw new Error('redirect');
      }
    },
    '@/auth': {
      auth: async () => ({
        user: { rustokToken: 'token', tenantSlug: 'tenant' }
      })
    },
    '@/shared/api/graphql': {
      graphqlRequest: async (query) =>
        query.includes('DashboardStats')
          ? { dashboardStats: { totalUsers: 7, usersChange: 0 } }
          : { recentActivity: [] }
    },
    '@/shared/api/modules': {
      fetchEnabledModules: async (opts) => {
        calls.push(opts);
        return fetchEnabledModules();
      }
    },
    '@/shared/ui/shadcn/badge': { Badge: component },
    '@/shared/ui/shadcn/button': { Button: component },
    '@/shared/ui/shadcn/card': {
      Card: component,
      CardContent: component,
      CardDescription: component,
      CardFooter: component,
      CardHeader: component,
      CardTitle: component
    },
    '@/widgets/app-shell': { PageContainer: component },
    '@/widgets/stats-card': { StatCard: component },
    '@/modules': { getAdminComponentsForSlot: () => [] },
    '@tabler/icons-react': {
      IconArrowUpRight: component,
      IconPackage: component,
      IconUsers: component
    },
    'react/jsx-runtime': jsx
  };
  const source = await readFile(new URL('./page.tsx', import.meta.url), 'utf8');
  const code = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX,
      esModuleInterop: true
    }
  }).outputText;
  const loaded = { exports: {} };
  new Function('module', 'exports', 'require', code)(
    loaded,
    loaded.exports,
    (name) => {
      assert.ok(Object.hasOwn(mocks, name), `unexpected import ${name}`);
      return mocks[name];
    }
  );
  return { page: loaded.exports.default, calls };
}

function text(element) {
  if (Array.isArray(element)) return element.map(text).join(' ');
  if (element && typeof element === 'object')
    return text(element.props?.children);
  return typeof element === 'string' || typeof element === 'number'
    ? String(element)
    : '';
}

test('dashboard shows policy failures instead of fabricated zero statistics', async () => {
  const { page, calls } = await dashboard(() => {
    throw new Error('policy unavailable');
  });
  const content = text(await page());
  assert.match(content, /Dashboard data is unavailable/);
  assert.match(content, /policy unavailable/);
  assert.doesNotMatch(
    content,
    /modules currently enabled|Total users|No recent activity yet/
  );
  assert.deepEqual(calls, [{ token: 'token', tenantSlug: 'tenant' }]);
});

test('dashboard renders the validated enabled module count', async () => {
  const { page } = await dashboard(() => ['forum', 'blog']);
  const content = text(await page());
  assert.match(content, /2\s+modules currently enabled/);
  assert.match(content, /forum/);
  assert.match(content, /blog/);
});
