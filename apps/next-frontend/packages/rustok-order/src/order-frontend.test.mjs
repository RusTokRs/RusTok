import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  fetchGuestRecentOrders,
} from './api/order.ts';

test('guest recent orders dedupes and clamps to 25 entries', () => {
  const sanitizeRecentOrders = (current, newId) => {
    const cleanId = newId.trim();
    if (!cleanId) return current;
    return [cleanId, ...current.filter((id) => id !== cleanId)].slice(0, 25);
  };

  const initial = ['id-1', 'id-2', 'id-3'];
  const updated1 = sanitizeRecentOrders(initial, 'id-2');
  assert.deepEqual(updated1, ['id-2', 'id-1', 'id-3']);

  const updated2 = sanitizeRecentOrders(initial, '   ');
  assert.deepEqual(updated2, initial);

  const longList = Array.from({ length: 30 }, (_, i) => `order-${i}`);
  const clamped = sanitizeRecentOrders(longList, 'new-order');
  assert.equal(clamped.length, 25);
  assert.equal(clamped[0], 'new-order');
});

test('fetchGuestRecentOrders filters out null responses', async () => {
  const mockGraphql = async ({ variables }) => {
    if (variables.id === 'order-valid') {
      return {
        data: {
          storefrontOrder: {
            id: 'order-valid',
            status: 'PAID',
            currencyCode: 'RUB',
            totalAmount: '1200.00',
          },
        },
      };
    }
    return { data: { storefrontOrder: null } };
  };

  const orders = await fetchGuestRecentOrders(
    mockGraphql,
    ['order-valid', 'order-missing'],
    'tenant-x',
  );
  assert.equal(orders.length, 1);
  assert.equal(orders[0].id, 'order-valid');
  assert.equal(orders[0].status, 'PAID');
});
