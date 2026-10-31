import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  UNREAD_COUNT_QUERY,
  GROUP_SUMMARIES_QUERY,
  GROUP_ITEMS_QUERY,
  GROUP_STATE_MUTATION,
  loadInboxSnapshot,
  applyNotificationGroupState,
} from './api.ts';

test('exported GraphQL documents contain required fields and variable bindings', () => {
  assert.match(UNREAD_COUNT_QUERY, /notificationInboxUnreadCount[\s\S]+unreadCount/);
  assert.match(GROUP_SUMMARIES_QUERY, /notificationInboxGroupSummaries[\s\S]+groupKey[\s\S]+itemCount[\s\S]+unreadCount/);
  assert.match(GROUP_ITEMS_QUERY, /notificationInboxGroupItems[\s\S]+groupKey[\s\S]+items/);
  assert.match(GROUP_STATE_MUTATION, /notificationInboxApplyGroupState[\s\S]+scanned[\s\S]+changed/);
});

test('loadInboxSnapshot resolves unreadCount and groups from mock executor', async () => {
  const mockGraphql = async ({ query }) => {
    if (query.includes('notificationInboxUnreadCount')) {
      return { data: { notificationInboxUnreadCount: { unreadCount: 3 } } };
    }
    if (query.includes('notificationInboxGroupSummaries')) {
      return {
        data: {
          notificationInboxGroupSummaries: {
            groups: [
              { groupKey: 'orders', itemCount: 2, unreadCount: 1, latestItem: null },
              { groupKey: 'comments', itemCount: 1, unreadCount: 1, latestItem: null },
            ],
            nextCursor: 'cur-1',
            hasMore: true,
          },
        },
      };
    }
    return { data: null };
  };

  const snapshot = await loadInboxSnapshot(mockGraphql, 'token-123', 'tenant-a');
  assert.equal(snapshot.unreadCount, 3);
  assert.equal(snapshot.groups.length, 2);
  assert.equal(snapshot.nextCursor, 'cur-1');
  assert.equal(snapshot.hasMore, true);
});

test('loadInboxSnapshot falls back gracefully on network error', async () => {
  const failingGraphql = async () => {
    throw new Error('Network error');
  };

  const snapshot = await loadInboxSnapshot(failingGraphql, 'token-123', 'tenant-a');
  assert.equal(snapshot.unreadCount, 0);
  assert.deepEqual(snapshot.groups, []);
  assert.equal(snapshot.nextCursor, null);
  assert.equal(snapshot.hasMore, false);
});

test('applyNotificationGroupState propagates command parameters and returns result', async () => {
  let capturedVariables = null;
  const mockGraphql = async ({ variables }) => {
    capturedVariables = variables;
    return {
      data: {
        notificationInboxApplyGroupState: {
          scanned: 5,
          changed: 3,
          nextCursor: null,
          hasMore: false,
        },
      },
    };
  };

  const result = await applyNotificationGroupState(
    mockGraphql,
    {
      groupKey: 'orders',
      action: 'MARK_READ',
      limit: 10,
      idempotencyKey: 'idem-456',
    },
    'token-789',
    'tenant-b',
  );

  assert.equal(result.scanned, 5);
  assert.equal(result.changed, 3);
  assert.equal(capturedVariables.groupKey, 'orders');
  assert.equal(capturedVariables.action, 'MARK_READ');
  assert.equal(capturedVariables.limit, 10);
  assert.equal(capturedVariables.idempotencyKey, 'idem-456');
});
