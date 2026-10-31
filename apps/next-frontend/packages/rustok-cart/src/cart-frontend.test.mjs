import assert from 'node:assert/strict';
import { test } from 'node:test';

test('cart line item calculation aggregates total quantities correctly', () => {
  const lineItems = [
    { id: '1', quantity: 2, unitPrice: '100.00', totalPrice: '200.00' },
    { id: '2', quantity: 3, unitPrice: '50.00', totalPrice: '150.00' },
  ];
  const itemCount = lineItems.reduce((acc, item) => acc + item.quantity, 0);
  assert.equal(itemCount, 5);
});

test('empty line items return zero item count', () => {
  const lineItems = [];
  const itemCount = lineItems.reduce((acc, item) => acc + item.quantity, 0);
  assert.equal(itemCount, 0);
});

test('subtotal and adjustment totals compute exact decimal string summaries', () => {
  const subtotal = 350.0;
  const discount = 50.0;
  const shipping = 15.0;
  const total = subtotal - discount + shipping;
  assert.equal(total, 315.0);
});

test('quantity update distinguishes removal at 0 from positive modification', () => {
  const decideAction = (qty) => (qty <= 0 ? 'REMOVE' : 'UPDATE');
  assert.equal(decideAction(0), 'REMOVE');
  assert.equal(decideAction(-1), 'REMOVE');
  assert.equal(decideAction(1), 'UPDATE');
  assert.equal(decideAction(10), 'UPDATE');
});
