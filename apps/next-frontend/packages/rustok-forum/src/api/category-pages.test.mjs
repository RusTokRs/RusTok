import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { collectForumCategoryPages, readForumCategoryConnection } from './category-pages.ts';

test('category query reads the GraphQL connection pageInfo, not a nonexistent total field', () => {
  const api = readFileSync(new URL('./forum.ts', import.meta.url), 'utf8');
  const connection = readFileSync(new URL('../../../../../../crates/modules/rustok-forum/src/graphql/connection.rs', import.meta.url), 'utf8');
  const pageInfo = readFileSync(new URL('../../../../../../crates/libs/rustok-api/src/graphql/common.rs', import.meta.url), 'utf8');
  const query = api.match(/const STOREFRONT_CATEGORIES_QUERY = `([^`]+)`;/)?.[1];
  assert.ok(query);
  assert.match(connection, /pub page_info: PageInfo/);
  assert.match(pageInfo, /pub total_count: i64/);
  assert.match(query, /pageInfo\s*\{\s*totalCount\s*\}/);
  assert.doesNotMatch(query, /\{\s*total\s/);
  assert.deepEqual(readForumCategoryConnection({
    items: [{ id: 'section' }], pageInfo: { totalCount: 101 },
  }), { items: [{ id: 'section' }], total: 101 });
  assert.deepEqual(readForumCategoryConnection(null), { items: [], total: 0 });
});

test('navigation and composer receive every visible category, beyond the first server page', async () => {
  for (const total of [0, 1, 50, 100, 101, 512]) {
    const all = Array.from({ length: total }, (_, n) => ({ id: `category-${n}` }));
    const offsets = [];
    const page = await collectForumCategoryPages(async (offset, limit) => {
      offsets.push(offset);
      assert.equal(limit, 100);
      return { items: all.slice(offset, offset + limit), total };
    });
    assert.deepEqual(page, { items: all, total });
    assert.deepEqual(offsets, Array.from({ length: Math.max(1, Math.ceil(total / 100)) }, (_, n) => n * 100));
  }
});

test('partial, duplicate or changing category pages fail instead of silently hiding sections', async () => {
  await assert.rejects(collectForumCategoryPages(async (offset) => ({
    items: offset ? [] : Array.from({ length: 100 }, (_, n) => ({ id: String(n) })),
    total: 101,
  })), /incomplete/);
  await assert.rejects(collectForumCategoryPages(async (offset) => ({
    items: Array.from({ length: offset ? 1 : 100 }, (_, n) => ({ id: String(n) })),
    total: 101,
  })), /duplicate/);
  await assert.rejects(collectForumCategoryPages(async (offset) => ({
    items: Array.from({ length: offset ? 1 : 100 }, (_, n) => ({ id: String(offset + n) })),
    total: offset ? 102 : 101,
  })), /total changed/);
  await assert.rejects(collectForumCategoryPages(async () => ({ items: [], total: 513 })), /total changed/);
});
