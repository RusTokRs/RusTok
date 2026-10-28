import assert from 'node:assert/strict';
import { test } from 'node:test';
import { orderThreadedComments } from './comments-order.ts';

const comment = (id, parentCommentId = null) => ({
  id,
  parentCommentId,
  effectiveLocale: 'en',
  authorId: null,
  contentPreview: id,
  createdAt: '2026-10-28T00:00:00Z'
});

test('renders all depths in parent-child order, not only immediate replies', () => {
  const ordered = orderThreadedComments([
    comment('grandchild', 'child'),
    comment('root'),
    comment('child', 'root')
  ]);
  assert.deepEqual(
    ordered.map(({ comment: item, depth }) => [item.id, depth]),
    [['root', 0], ['child', 1], ['grandchild', 2]]
  );
});

test('promotes page-orphan replies to visible roots', () => {
  const ordered = orderThreadedComments([
    comment('orphan', 'parent-on-previous-page'),
    comment('child', 'orphan')
  ]);
  assert.deepEqual(
    ordered.map(({ comment: item, depth }) => [item.id, depth]),
    [['orphan', 0], ['child', 1]]
  );
});

test('never hides cyclic or self-parented projections', () => {
  const ordered = orderThreadedComments([
    comment('a', 'b'),
    comment('b', 'a'),
    comment('self', 'self')
  ]);
  assert.deepEqual(new Set(ordered.map(({ comment: item }) => item.id)), new Set(['a', 'b', 'self']));
  assert.equal(ordered.length, 3);
});
