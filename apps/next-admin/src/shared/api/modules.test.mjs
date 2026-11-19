import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';

const source = await readFile(new URL('./modules.ts', import.meta.url), 'utf8');
const js =
  stripTypeScriptTypes(source)
    .replace(
      /import \{[^}]*\} from ['\"]\.\/graphql['\"];?/,
      "const { graphqlRequest } = require('./graphql');"
    )
    .replace(
      /import \{[^}]*\} from ['\"]\.\/http['\"];?/,
      "const { readHttpError } = require('./http');"
    )
    .replace(/export async function /g, 'async function ')
    .replace(/export const /g, 'const ') +
  '\nmodule.exports = { fetchEnabledModules, fetchRegistryPublishRequestStatus, holdRegistryPublishRequest, rollbackModuleTransition, updateModuleSettings, yankRegistryRelease, unyankRegistryRelease };';

function client(
  graphqlResult = { rollbackModuleTransition: { operationId: 'operation' } }
) {
  const requests = [];
  const graphqlCalls = [];
  const mockModule = { exports: {} };
  const fetch = async (url, options) => {
    requests.push({ url, ...options });
    return { ok: true, json: async () => ({ accepted: true }) };
  };
  const graphqlRequest = async (...args) => {
    graphqlCalls.push(args);
    return graphqlResult;
  };
  const readHttpError = async () => 'http-error';
  const require = (name) => {
    if (name === './graphql') return { graphqlRequest };
    if (name === './http') return { readHttpError };
    assert.fail(`unexpected require: ${name}`);
  };
  const crypto = { randomUUID: () => 'generated-key' };
  new Function(
    'module',
    'exports',
    'require',
    'fetch',
    'crypto',
    'process',
    js
  )(mockModule, mockModule.exports, require, fetch, crypto, {
    env: { NEXT_PUBLIC_API_URL: 'https://api.example.test' }
  });
  return { api: mockModule.exports, requests, graphqlCalls };
}

test('availability rejects malformed policy responses rather than treating them as empty', async () => {
  for (const enabledModules of [
    null,
    undefined,
    'forum',
    [1],
    [''],
    ['forum\n'],
    ['forum', 'forum']
  ]) {
    const { api } = client({ enabledModules });
    await assert.rejects(
      api.fetchEnabledModules({ token: 'token', tenantSlug: 'tenant' })
    );
  }
  const { api } = client({ enabledModules: ['forum'] });
  assert.deepEqual(
    await api.fetchEnabledModules({ token: 'token', tenantSlug: 'tenant' }),
    ['forum']
  );
  const empty = client({ enabledModules: [] });
  assert.deepEqual(
    await empty.api.fetchEnabledModules({
      token: 'token',
      tenantSlug: 'tenant'
    }),
    []
  );
});

test('capped availability uses one complete authorized policy snapshot, not a merged list', async () => {
  const server = await readFile(
    new URL('../../../../server/src/graphql/queries.rs', import.meta.url),
    'utf8'
  );
  assert.match(server, /limit\.unwrap_or\(100\)\.clamp\(1, 100\)/);
  assert.match(
    server,
    /async fn module_effective_policy[\s\S]*?ensure_modules_read_permission\(ctx\)/
  );
  assert.match(source, /enabledModules\(limit: 100\)/);
  const { api } = client();
  const modules = Array.from({ length: 101 }, (_, index) => `module_${index}`);
  const belowLimit = client({ enabledModules: modules.slice(0, 99) });
  assert.deepEqual(
    await belowLimit.api.fetchEnabledModules(),
    modules.slice(0, 99)
  );
  await assert.rejects(
    client({ enabledModules: modules }).api.fetchEnabledModules(),
    /Invalid/
  );

  const revision = `sha256:${'a'.repeat(64)}`;
  const decisions = Array.from({ length: 130 }, (_, index) => ({
    moduleSlug: `new_${index}`,
    enabled: index < 125,
    policyRevision: revision
  }));
  const calls = [];
  const opts = {
    token: 'token',
    tenantSlug: 'tenant',
    tenantId: 'tenant-id',
    locale: 'ru',
    graphqlUrl: '/graphql',
    graphql: async (...args) => {
      calls.push(args);
      return args[0].includes('CompleteModulePolicy')
        ? { moduleEffectivePolicy: { policyRevision: revision, decisions } }
        : { enabledModules: modules.slice(0, 100) };
    }
  };
  const enabled = await api.fetchEnabledModules(opts);
  assert.deepEqual(
    enabled,
    decisions.slice(0, 125).map((decision) => decision.moduleSlug)
  );
  assert.equal(calls.length, 2);
  for (const args of calls) {
    assert.equal(args[2], 'token');
    assert.equal(args[3], 'tenant');
    assert.deepEqual(args[4], {
      tenantId: 'tenant-id',
      locale: 'ru',
      graphqlUrl: '/graphql'
    });
  }
  opts.graphql = async (query) =>
    query.includes('CompleteModulePolicy')
      ? {
          moduleEffectivePolicy: {
            policyRevision: revision,
            decisions: decisions.slice(0, 100)
          }
        }
      : { enabledModules: modules.slice(0, 100) };
  assert.equal((await api.fetchEnabledModules(opts)).length, 100);

  // A changed owner snapshot is authoritative on its own, even if empty.
  opts.graphql = async (query) =>
    query.includes('CompleteModulePolicy')
      ? { moduleEffectivePolicy: { policyRevision: revision, decisions: [] } }
      : { enabledModules: modules.slice(0, 100) };
  assert.deepEqual(await api.fetchEnabledModules(opts), []);
});

test('capped availability fails closed on denied, partial or malformed full policy', async () => {
  const { api } = client();
  const modules = Array.from({ length: 100 }, (_, index) => `module_${index}`);
  const revision = `sha256:${'b'.repeat(64)}`;
  const decision = {
    moduleSlug: 'forum',
    enabled: true,
    policyRevision: revision
  };
  const invalid = [
    null,
    {},
    { policyRevision: '', decisions: [] },
    { policyRevision: revision, decisions: null },
    { policyRevision: revision, decisions: [decision, decision] },
    {
      policyRevision: revision,
      decisions: [{ ...decision, policyRevision: 'other' }]
    },
    {
      policyRevision: revision,
      decisions: [{ ...decision, moduleSlug: 'forum\n' }]
    },
    { policyRevision: revision, decisions: [{ ...decision, enabled: 'true' }] }
  ];
  for (const policy of invalid) {
    await assert.rejects(
      api.fetchEnabledModules({
        graphql: async (query) =>
          query.includes('CompleteModulePolicy')
            ? { moduleEffectivePolicy: policy }
            : { enabledModules: modules }
      }),
      /may be incomplete/
    );
  }
  await assert.rejects(
    api.fetchEnabledModules({
      graphql: async (query) => {
        if (query.includes('CompleteModulePolicy'))
          throw new Error('permission denied');
        return { enabledModules: modules };
      }
    }),
    /may be incomplete/
  );
});

test('settings write forwards its frozen request and transport scope and validates the response', async () => {
  const { api } = client();
  const calls = [];
  const response = {
    moduleSlug: 'forum',
    settings: '{"n":1}',
    enabled: true,
    revision: 9
  };
  const opts = {
    token: 'token',
    tenantSlug: 'tenant',
    tenantId: 'tenant-id',
    locale: 'ru',
    graphqlUrl: '/api/graphql',
    graphql: async (...args) => {
      calls.push(args);
      return { updateModuleSettings: response };
    }
  };
  assert.equal(
    await api.updateModuleSettings('forum', '{"n":1}', 8, 'key', opts),
    response
  );
  assert.deepEqual(calls[0][1], {
    moduleSlug: 'forum',
    settings: '{"n":1}',
    expectedRevision: 8,
    idempotencyKey: 'key'
  });
  assert.equal(calls[0][2], 'token');
  assert.equal(calls[0][3], 'tenant');
  assert.deepEqual(calls[0][4], {
    tenantId: 'tenant-id',
    locale: 'ru',
    graphqlUrl: '/api/graphql'
  });
  for (const invalid of [
    null,
    { ...response, moduleSlug: 'other' },
    { ...response, revision: 7 },
    { ...response, revision: NaN },
    { ...response, settings: null },
    { ...response, enabled: null }
  ]) {
    await assert.rejects(
      api.updateModuleSettings('forum', '{}', 8, 'key', {
        graphql: async () => ({ updateModuleSettings: invalid })
      }),
      /response is invalid/
    );
  }
});

test('registry status and decisions preserve the request identity and replay key', async () => {
  const { api, requests } = client();
  const opts = { token: 'token', tenantSlug: 'tenant' };
  await api.fetchRegistryPublishRequestStatus('request/unsafe', opts);
  await api.holdRegistryPublishRequest(
    'request/unsafe',
    'reason',
    'security',
    false,
    opts,
    'same-key'
  );
  await api.holdRegistryPublishRequest(
    'request/unsafe',
    'reason',
    'security',
    false,
    opts,
    'same-key'
  );
  assert.equal(
    requests[0].url,
    'https://api.example.test/v2/catalog/publish/request%2Funsafe'
  );
  assert.equal(
    requests[1].url,
    'https://api.example.test/v2/catalog/publish/request%2Funsafe/hold'
  );
  assert.equal(requests[1].headers['Idempotency-Key'], 'same-key');
  assert.equal(requests[2].headers['Idempotency-Key'], 'same-key');
  assert.equal(requests[1].headers.Authorization, 'Bearer token');
  assert.deepEqual(JSON.parse(requests[1].body), {
    schema_version: 1,
    dry_run: false,
    reason: 'reason',
    reason_code: 'security'
  });
});

test('transition rollback sends the owner operation and the supplied replay key', async () => {
  const { api, graphqlCalls } = client();
  await api.rollbackModuleTransition('operation', 'reason', 'replay-key', {
    token: 'token',
    tenantSlug: 'tenant'
  });
  assert.match(
    graphqlCalls[0][0],
    /rollbackModuleTransition\(operationId: \$operationId/
  );
  assert.deepEqual(graphqlCalls[0][1], {
    operationId: 'operation',
    reason: 'reason',
    idempotencyKey: 'replay-key'
  });
  assert.equal(graphqlCalls[0][2], 'token');
  assert.equal(graphqlCalls[0][3], 'tenant');
});

test('yank and unyank registry release post to their canonical REST endpoints with payload', async () => {
  const { api, requests } = client();
  const opts = { token: 'token-123', tenantSlug: 'tenant-456' };

  await api.yankRegistryRelease(
    'mod-a',
    '1.2.3',
    'vulnerability detected',
    'cve_patch',
    false,
    opts,
    'yank-key'
  );

  await api.unyankRegistryRelease(
    'mod-a',
    '1.2.3',
    'false alarm resolved',
    'reinstated',
    false,
    opts,
    'unyank-key'
  );

  assert.equal(requests[0].url, 'https://api.example.test/v2/catalog/yank');
  assert.equal(requests[0].method, 'POST');
  assert.equal(requests[0].headers['Idempotency-Key'], 'yank-key');
  assert.equal(requests[0].headers.Authorization, 'Bearer token-123');
  assert.deepEqual(JSON.parse(requests[0].body), {
    schema_version: 1,
    dry_run: false,
    slug: 'mod-a',
    version: '1.2.3',
    reason: 'vulnerability detected',
    reason_code: 'cve_patch'
  });

  assert.equal(requests[1].url, 'https://api.example.test/v2/catalog/unyank');
  assert.equal(requests[1].method, 'POST');
  assert.equal(requests[1].headers['Idempotency-Key'], 'unyank-key');
  assert.equal(requests[1].headers.Authorization, 'Bearer token-123');
  assert.deepEqual(JSON.parse(requests[1].body), {
    schema_version: 1,
    dry_run: false,
    slug: 'mod-a',
    version: '1.2.3',
    reason: 'false alarm resolved',
    reason_code: 'reinstated'
  });
});
