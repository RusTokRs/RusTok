import test from 'node:test';
import assert from 'node:assert/strict';
import { buildCategoryHierarchyOptions } from './category-hierarchy.ts';

test('category labels and depth follow current parent IDs, not stale paths', () => {
  const categories = [
    {
      id: 'parent',
      parentId: null,
      code: 'new-parent',
      name: 'New Parent',
      path: 'old-parent'
    },
    {
      id: 'child',
      parentId: 'parent',
      code: 'child',
      name: 'Child',
      path: 'old-parent/child'
    }
  ];

  assert.deepEqual(buildCategoryHierarchyOptions(categories), [
    { id: 'parent', label: 'New Parent', depth: 0 },
    { id: 'child', label: 'New Parent / Child', depth: 1 }
  ]);
});

test('category hierarchy tolerates missing parents and cycles without looping', () => {
  const options = buildCategoryHierarchyOptions([
    { id: 'a', parentId: 'b', code: 'a', name: 'A' },
    { id: 'b', parentId: 'a', code: 'b', name: 'B' },
    { id: 'orphan', parentId: 'missing', code: 'orphan', name: 'Orphan' }
  ]);

  assert.deepEqual(options, [
    { id: 'a', label: 'B / A', depth: 1 },
    { id: 'b', label: 'A / B', depth: 1 },
    { id: 'orphan', label: 'Orphan', depth: 0 }
  ]);
});
