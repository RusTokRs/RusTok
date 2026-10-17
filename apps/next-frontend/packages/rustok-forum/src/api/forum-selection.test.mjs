import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';
import test from 'node:test';

const source = readFileSync(new URL('./forum.ts', import.meta.url), 'utf8');
function gateway(name, constant, transport) {
  const start = source.indexOf(`export async function ${name}(`);
  const end = source.indexOf('\nexport async function ', start + 1);
  const body = source
    .slice(start, end)
    .replace('export async function', 'async function');
  const query = source.match(
    new RegExp(`const ${constant} = \`([^\`]+)\`;`),
  )[1];
  return compileFunction(`${stripTypeScriptTypes(body)}\nreturn ${name};`, [
    'storefrontGraphql',
    'assertGraphqlResult',
    constant,
  ])(
    transport,
    (context, result) => {
      if (result.error) throw new Error(context);
    },
    query,
  );
}

for (const [name, constant, resultKey] of [
  [
    'fetchStorefrontTopics',
    'STOREFRONT_TOPICS_QUERY',
    'forumStorefrontAudienceTopics',
  ],
  [
    'fetchStorefrontUnreadTopics',
    'STOREFRONT_UNREAD_TOPICS_QUERY',
    'forumStorefrontUnreadTopics',
  ],
]) {
  test(`${name}: every selection and cursor is forwarded to the same owner`, async () => {
    const calls = [];
    const selected = [{ id: 'old-but-selected', replyCount: 90 }];
    const load = gateway(name, constant, async (request) => {
      calls.push(request);
      return {
        data: { [resultKey]: { items: selected, nextCursor: 'second' } },
      };
    });
    const options = {
      tenantId: 'tenant',
      tenantSlug: 'shop',
      token: 'signed',
      categoryId: 'category',
      locale: 'ru',
      search: 'запрос',
      order: 'TOP',
      solvedOnly: true,
    };
    const first = await load(options);
    const second = await load({ ...options, after: first.nextCursor });
    assert.deepEqual(
      second.items,
      selected,
      'preserve owner order and contents',
    );
    for (const { query, tenant, token, variables } of calls) {
      assert.match(query, /\$order: ForumTopicListOrder/);
      for (const field of ['search', 'order', 'solvedOnly'])
        assert.ok(query.includes(`${field}: $${field}`));
      assert.equal(tenant, 'shop');
      assert.equal(token, 'signed');
      assert.deepEqual(
        { ...variables, after: undefined },
        {
          tenantId: 'tenant',
          categoryId: 'category',
          locale: 'ru',
          after: undefined,
          search: 'запрос',
          order: 'TOP',
          solvedOnly: true,
          [name.includes('Unread') ? 'limit' : 'perPage']: undefined,
        },
      );
    }
    assert.equal(calls[0].variables.after, undefined);
    assert.equal(calls[1].variables.after, 'second');
    if (name.includes('Unread'))
      assert.match(calls[0].query, /unreadOnly: true/);
  });
  test(`${name}: omitted selectors mean latest, not a client-specific default`, async () => {
    let variables;
    const load = gateway(name, constant, async (request) => {
      variables = request.variables;
      return { data: { [resultKey]: { items: [], nextCursor: null } } };
    });
    await load({ token: 'signed' });
    assert.equal(variables.order, 'LATEST');
    assert.equal(variables.solvedOnly, false);
    assert.equal(variables.search, undefined);
  });
  test(`${name}: owner failures stay failures rather than an empty selection`, async () => {
    const load = gateway(name, constant, async () => ({
      error: 'unavailable',
    }));
    await assert.rejects(load({ token: 'signed', search: 'needle' }), /forum/);
  });
}
