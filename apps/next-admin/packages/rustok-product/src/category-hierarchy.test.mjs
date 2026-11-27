import test from 'node:test';
import assert from 'node:assert/strict';
import { buildCategoryHierarchyOptions } from './category-hierarchy.ts';

test('category labels and depth follow current parent IDs, not stale paths', () => {
  const categories = [
    {
      id: 'parent',
      parentId: null,
      code: 'new-parent',
      slug: 'new-parent',
      name: 'New Parent',
      path: 'old-parent'
    },
    {
      id: 'child',
      parentId: 'parent',
      code: 'child',
      slug: 'child',
      name: 'Child',
      path: 'old-parent/child'
    }
  ];

  assert.deepEqual(buildCategoryHierarchyOptions(categories), [
    { id: 'parent', label: 'new-parent', depth: 0 },
    { id: 'child', label: 'new-parent/child', depth: 1 }
  ]);
});

test('category hierarchy tolerates missing parents and cycles without looping', () => {
  const options = buildCategoryHierarchyOptions([
    { id: 'a', parentId: 'b', code: 'a', slug: 'a', name: 'A' },
    { id: 'b', parentId: 'a', code: 'b', slug: 'b', name: 'B' },
    { id: 'orphan', parentId: 'missing', code: 'orphan', slug: 'orphan', name: 'Orphan' }
  ]);

  assert.deepEqual(options, [
    { id: 'a', label: 'b/a', depth: 1 },
    { id: 'b', label: 'a/b', depth: 1 },
    { id: 'orphan', label: 'Orphan', depth: 0 }
  ]);
});
