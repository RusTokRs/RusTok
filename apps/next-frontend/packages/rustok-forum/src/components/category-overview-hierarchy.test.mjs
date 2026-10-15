import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { buildCategoryOverviewHierarchy, filterCategoryOverviewHierarchy } from './category-overview-hierarchy.ts';

const category = (id, name, parentId, topicCount = 1, replyCount = 2) => ({
  id, name, slug: id, parentId, topicCount, replyCount,
});

test('overview includes grandchildren and deeper visible sections and their totals', () => {
  const tree = [
    category('root', 'Community', null),
    category('child', 'Guides', 'root', 2, 3),
    category('leaf', 'How to', 'child', 3, 4),
    category('deep', 'Setup', 'leaf', 4, 5),
    category('other', 'Other', null, 5, 6),
  ];
  const overview = buildCategoryOverviewHierarchy(tree);
  assert.deepEqual(overview.map((group) => group.category.id), ['root', 'other']);
  assert.deepEqual(overview[0].subcategories.map((item) => item.id), ['child', 'leaf', 'deep']);
  assert.deepEqual(overview[0].subcategories.map((item) => item.displayName), [
    'Community / Guides', 'Community / Guides / How to', 'Community / Guides / How to / Setup',
  ]);
  assert.equal(overview[0].totalTopicCount, 10);
  assert.equal(overview[0].totalReplyCount, 14);
  assert.equal(overview[1].totalTopicCount, 5);
});

test('a missing parent and a malformed cycle cannot silently discard visible categories', () => {
  const overview = buildCategoryOverviewHierarchy([
    category('orphan', 'Orphan', 'missing'),
    category('cycle-a', 'A', 'cycle-b'),
    category('cycle-b', 'B', 'cycle-a'),
  ]);
  assert.deepEqual(overview.flatMap((group) => [
    group.category.id, ...group.subcategories.map((child) => child.id),
  ]).sort(), ['cycle-a', 'cycle-b', 'orphan']);
});

test('matching a root slug preserves every descendant even if one child also matches', () => {
  const hierarchy = buildCategoryOverviewHierarchy([
    { ...category('root', 'Community', null), slug: 'help' },
    { ...category('related', 'API', 'root'), slug: 'help-api' },
    category('other', 'Announcements', 'root'),
    category('unrelated', 'Elsewhere', null),
  ]);
  const rootMatch = filterCategoryOverviewHierarchy(hierarchy, '  HELP ');
  assert.deepEqual(rootMatch.map((item) => item.category.id), ['root']);
  assert.deepEqual(rootMatch[0].subcategories.map((item) => item.id), ['related', 'other']);
  assert.equal(rootMatch[0].totalTopicCount, 3);

  const childMatch = filterCategoryOverviewHierarchy(hierarchy, 'api');
  assert.deepEqual(childMatch[0].subcategories.map((item) => item.id), ['related']);
  assert.deepEqual(filterCategoryOverviewHierarchy(hierarchy, 'missing'), []);
});

test('a category no longer visible to the current viewer stops filtering the feed', () => {
  const section = readFileSync(new URL('./forum-section.tsx', import.meta.url), 'utf8');
  assert.match(section, /if \(!selectedCategoryId \|\| categoryPage\?\.key !== viewerKey\) return;/);
  assert.match(section, /categoryPage\.items\.some\(\(category\) => category\.id === selectedCategoryId\)/);
  assert.match(section, /setSelectedCategoryId\(null\)/);
});
