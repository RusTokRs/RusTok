import assert from 'node:assert/strict';
import test from 'node:test';
import { graphqlRequest } from './graphql.ts';

const query =
  'query AdminScopeProbe($slug: String!) { scopeProbe(slug: $slug) }';

test('concurrent identical admin reads retain tenant, actor and locale isolation', async () => {
  const originalFetch = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (_url, options) => {
    const headers = new Headers(options.headers);
    const scope = [
      headers.get('X-Tenant-Slug'),
      headers.get('X-Tenant-ID'),
      headers.get('Authorization'),
      headers.get('Accept-Language')
    ].join('|');
    calls.push({ scope, cache: options.cache });
    // All calls remain in flight while Apollo would normally deduplicate them.
    await new Promise((resolve) => setTimeout(resolve, 20));
    return new Response(JSON.stringify({ data: { scopeProbe: scope } }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' }
    });
  };
  try {
    const inputs = [
      { token: 'actor-a', slug: 'tenant-a', id: 'id-a', locale: 'en' },
      { token: 'actor-a', slug: 'tenant-b', id: 'id-b', locale: 'en' },
      { token: 'actor-b', slug: 'tenant-a', id: 'id-a', locale: 'en' },
      { token: 'actor-a', slug: 'tenant-a', id: 'id-a', locale: 'ru' }
    ];
    const results = await Promise.all(
      inputs.map(({ token, slug, id, locale }) =>
        graphqlRequest(query, { slug: 'same-query' }, token, slug, {
          tenantId: id,
          locale,
          graphqlUrl: 'https://admin-scope-probe.invalid/api/graphql'
        })
      )
    );
    assert.equal(calls.length, inputs.length);
    for (const [index, input] of inputs.entries()) {
      const expected = `${input.slug}|${input.id}|Bearer ${input.token}|${input.locale}`;
      assert.equal(results[index].scopeProbe, expected);
      assert.equal(calls[index].scope, expected);
      assert.equal(calls[index].cache, 'no-store');
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('partial GraphQL errors do not become successful policy results', async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () =>
    new Response(
      JSON.stringify({
        data: { scopeProbe: 'partial' },
        errors: [
          { message: 'Permission denied', extensions: { code: 'FORBIDDEN' } }
        ]
      }),
      { status: 200, headers: { 'Content-Type': 'application/json' } }
    );
  try {
    await assert.rejects(
      graphqlRequest(query, { slug: 'same-query' }, 'token', 'tenant', {
        graphqlUrl: 'https://admin-partial-error.invalid/api/graphql'
      }),
      (error) => error.code === 'FORBIDDEN'
    );
  } finally {
    globalThis.fetch = originalFetch;
  }
});
