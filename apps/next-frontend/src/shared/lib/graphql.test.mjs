import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { storefrontGraphql } from './graphql.ts';

const query = 'query ForumTest { forumStorefrontAudienceTopics { nextCursor } }';

test('Apollo 4 GraphQL errors survive errorPolicy all, even with partial data', async () => {
  const original = globalThis.fetch;
  const headers = [];
  const caches = [];
  globalThis.fetch = async (_request, init) => {
    headers.push(new Headers(init.headers));
    caches.push(init.cache);
    return new Response(JSON.stringify({
      data: { forumStorefrontAudienceTopics: null },
      errors: [{ message: 'Audience denied', path: ['forumStorefrontAudienceTopics'] }],
    }), { status: 200, headers: { 'Content-Type': 'application/json' } });
  };
  try {
    const response = await storefrontGraphql({
      query, baseUrl: 'http://test-forum-graphql.invalid', tenant: 'demo', token: 'user-token',
    });
    assert.deepEqual(response.errors, [{ message: 'Audience denied' }]);
    assert.equal(response.data?.forumStorefrontAudienceTopics, null);
    assert.equal(headers[0].get('Authorization'), 'Bearer user-token');
    assert.equal(headers[0].get('X-Tenant-Slug'), 'demo');
    assert.equal(caches[0], 'no-store');
  } finally {
    globalThis.fetch = original;
  }
});

test('an anonymous request never borrows the token from a previous request', async () => {
  const original = globalThis.fetch;
  const headers = [];
  globalThis.fetch = async (_request, init) => {
    headers.push(new Headers(init.headers));
    return new Response(JSON.stringify({ data: { forumStorefrontAudienceTopics: { nextCursor: null } } }), {
      status: 200, headers: { 'Content-Type': 'application/json' },
    });
  };
  try {
    const authorized = await storefrontGraphql({
      query, baseUrl: 'http://test-forum-isolation.invalid', token: 'one-user',
    });
    const guest = await storefrontGraphql({
      query, baseUrl: 'http://test-forum-isolation.invalid',
    });
    assert.equal(authorized.errors, undefined);
    assert.equal(guest.errors, undefined);
    assert.equal(headers[0].get('Authorization'), 'Bearer one-user');
    assert.equal(headers[1].get('Authorization'), null);
  } finally {
    globalThis.fetch = original;
  }
});
