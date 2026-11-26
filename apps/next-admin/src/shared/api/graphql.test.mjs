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

test('browser GraphQL requests use same-origin BFF without bearer or token-derived tenant ID', async () => {
  const originalFetch=globalThis.fetch;
  const hadWindow=Object.prototype.hasOwnProperty.call(globalThis,'window');
  const originalWindow=globalThis.window;
  let observed;
  globalThis.window={};
  globalThis.fetch=async (url,options)=>{
    observed={url,headers:new Headers(options.headers)};
    return new Response(JSON.stringify({data:{scopeProbe:'browser'}}),{status:200,headers:{'Content-Type':'application/json'}});
  };
  try {
    await graphqlRequest(query,{slug:'x'},'secret-access-token','tenant-a',{
      graphqlUrl:'https://attacker.example.test/graphql'
    });
    assert.equal(observed.url,'/api/rustok/graphql');
    assert.equal(observed.headers.get('authorization'),null);
    assert.equal(observed.headers.get('x-tenant-id'),null);
    assert.equal(observed.headers.get('x-tenant-slug'),'tenant-a');
  } finally {
    globalThis.fetch=originalFetch;
    if(hadWindow) Object.defineProperty(globalThis,'window',{configurable:true,writable:true,value:originalWindow});
    else delete globalThis.window;
  }
});
