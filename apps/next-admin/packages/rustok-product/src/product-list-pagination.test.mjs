import test from 'node:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_PRODUCT_ADMIN_PAGE_SIZE,
  MAX_PRODUCT_ADMIN_PAGE,
  MAX_PRODUCT_ADMIN_PAGE_SIZE,
  normalizeProductAdminPagination
} from './product-list-pagination.ts';

test('product admin pagination retains valid server page requests', () => {
  assert.deepEqual(
    normalizeProductAdminPagination({ page: '3', perPage: '50' }),
    { page: 3, perPage: 50 }
  );
  assert.deepEqual(normalizeProductAdminPagination({}), {
    page: 1,
    perPage: DEFAULT_PRODUCT_ADMIN_PAGE_SIZE
  });
});

test('product admin pagination bounds excessive and nonrepresentable inputs', () => {
  assert.deepEqual(
    normalizeProductAdminPagination({ page: Number.MAX_SAFE_INTEGER, perPage: 999 }),
    { page: MAX_PRODUCT_ADMIN_PAGE, perPage: MAX_PRODUCT_ADMIN_PAGE_SIZE }
  );
  assert.deepEqual(
    normalizeProductAdminPagination({ page: '9999999999999999999999999', perPage: '101' }),
    { page: MAX_PRODUCT_ADMIN_PAGE, perPage: MAX_PRODUCT_ADMIN_PAGE_SIZE }
  );
});

test('product admin pagination refuses malformed, negative, zero and fractional values', () => {
  for (const invalid of ['0', '-3', '1.5', '1e3', 'NaN', '', 0, -1, 1.5, Infinity]) {
    assert.equal(normalizeProductAdminPagination({ page: invalid }).page, 1);
    assert.equal(
      normalizeProductAdminPagination({ perPage: invalid }).perPage,
      DEFAULT_PRODUCT_ADMIN_PAGE_SIZE
    );
  }
});
