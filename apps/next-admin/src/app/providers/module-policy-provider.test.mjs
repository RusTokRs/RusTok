import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
import * as jsx from 'react/jsx-runtime';

async function providers(session, fetchEnabledModules) {
  const policyError = () => null;
  const clientProvider = () => null;
  const notFound = () => {
    throw new Error('NOT_FOUND');
  };
  const mocks = {
    '@/auth': { auth: async () => session },
    '@/shared/api/modules': { fetchEnabledModules },
    '@/shared/lib/enabled-modules-context': {
      EnabledModulesClientProvider: clientProvider
    },
    './module-policy-error': { ModulePolicyError: policyError },
    'next/navigation': { notFound },
    'react/jsx-runtime': jsx
  };
  async function load(name) {
    const source = await readFile(
      new URL(`./${name}.tsx`, import.meta.url),
      'utf8'
    );
    const code = ts.transpileModule(source, {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        jsx: ts.JsxEmit.ReactJSX
      }
    }).outputText;
    const loaded = { exports: {} };
    new Function('module', 'exports', 'require', 'console', code)(
      loaded,
      loaded.exports,
      (specifier) => {
        assert.ok(
          Object.hasOwn(mocks, specifier),
          `unexpected import ${specifier}`
        );
        return mocks[specifier];
      },
      { error() {} }
    );
    return loaded.exports;
  }
  return {
    ...(await load('enabled-modules-provider')),
    ...(await load('module-guard')),
    policyError,
    clientProvider
  };
}

test('missing session scope produces unavailability instead of an empty policy or 404', async () => {
  for (const session of [
    null,
    { user: { rustokToken: '', tenantSlug: 'tenant' } },
    { user: { rustokToken: 'token', tenantSlug: null } }
  ]) {
    let calls = 0;
    const { EnabledModulesProvider, ModuleGuard, policyError } =
      await providers(session, async () => {
        calls++;
        return [];
      });
    assert.equal(
      (await EnabledModulesProvider({ children: 'private' })).type,
      policyError
    );
    assert.equal(
      (await ModuleGuard({ slug: 'forum', children: 'private' })).type,
      policyError
    );
    assert.equal(calls, 0);
  }
});

test('failed policy read stays unavailable; only a successful policy decides access', async () => {
  const session = { user: { rustokToken: 'token', tenantSlug: 'tenant' } };
  const failed = await providers(session, async () => {
    throw new Error('offline');
  });
  assert.equal(
    (await failed.EnabledModulesProvider({ children: 'private' })).type,
    failed.policyError
  );
  assert.equal(
    (await failed.ModuleGuard({ slug: 'forum', children: 'private' })).type,
    failed.policyError
  );

  const allowed = await providers(session, async () => ['forum']);
  assert.equal(
    (await allowed.EnabledModulesProvider({ children: 'private' })).type,
    allowed.clientProvider
  );
  assert.equal(
    (await allowed.ModuleGuard({ slug: 'forum', children: 'private' })).props
      .children,
    'private'
  );
  const denied = await providers(session, async () => []);
  await assert.rejects(
    denied.ModuleGuard({ slug: 'forum', children: 'private' }),
    /NOT_FOUND/
  );
});
