import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { stripTypeScriptTypes } from 'node:module';

const source = await readFile(new URL('./modules.ts', import.meta.url), 'utf8');
const js =
  stripTypeScriptTypes(source)
    .replace(
      /import \{ graphqlRequest,?\s*\} from ['\"]\.\/graphql['\"];?/,
      "const { graphqlRequest } = require('./graphql');"
    )
    .replace(/export async function /g, 'async function ')
    .replace(/export const /g, 'const ') +
  '\nmodule.exports = { fetchRegistryPublishRequestStatus, holdRegistryPublishRequest, rollbackModuleTransition };';

function client() {
  const requests = [];
  const graphqlCalls = [];
  const mockModule = { exports: {} };
  const fetch = async (url, options) => {
    requests.push({ url, ...options });
    return { ok: true, json: async () => ({ accepted: true }) };
  };
  const graphqlRequest = async (...args) => {
    graphqlCalls.push(args);
    return { rollbackModuleTransition: { operationId: 'operation' } };
  };
  const require = (name) => {
    assert.equal(name, './graphql');
    return { graphqlRequest };
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
