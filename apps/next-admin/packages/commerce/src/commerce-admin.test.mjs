/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';

// Load api.ts by injecting mock graphqlRequest
const apiSource = readFileSync(new URL('./api.ts', import.meta.url), 'utf8');
const cleanApiSource = stripTypeScriptTypes(
  apiSource
    .replace(/import\s+type\s+\{[^}]*\}\s+from\s+'\.\/types';/g, '')
    .replace(/import\s+\{[^}]*\}\s+from\s+'@\/lib\/graphql';/g, '')
    .replace(/export\s+type\s+\{[^}]*\};/g, '')
    .replaceAll('export ', '')
);

const createApiModule = (mockGraphqlRequest) => {
  const exports = {};
  compileFunction(
    `${cleanApiSource}\nObject.assign(exports, {
      SHIPPING_PROFILES_QUERY,
      SHIPPING_PROFILE_QUERY,
      CREATE_SHIPPING_PROFILE_MUTATION,
      UPDATE_SHIPPING_PROFILE_MUTATION,
      DEACTIVATE_SHIPPING_PROFILE_MUTATION,
      REACTIVATE_SHIPPING_PROFILE_MUTATION,
      PREVIEW_CART_PROMOTION_MUTATION,
      APPLY_CART_PROMOTION_MUTATION,
      ORDER_CHANGES_QUERY,
      ORDER_CHANGE_QUERY,
      CREATE_ORDER_RETURN_DECISION_MUTATION,
      APPLY_ORDER_CHANGE_MUTATION,
      CANCEL_ORDER_CHANGE_MUTATION,
      ORDERS_QUERY,
      ORDER_QUERY,
      PAYMENT_COLLECTIONS_QUERY,
      REFUNDS_QUERY,
      FULFILLMENTS_QUERY,
      listShippingProfiles,
      getShippingProfile,
      createShippingProfile,
      updateShippingProfile,
      deactivateShippingProfile,
      reactivateShippingProfile,
      previewCartPromotion,
      applyCartPromotion,
      listOrderChanges,
      getOrderChange,
      applyOrderChange,
      cancelOrderChange,
      createOrderReturnDecision,
      listOrders,
      getOrderDetail,
      listPaymentCollections,
      listRefunds,
      listFulfillments
    });`,
    ['graphqlRequest', 'exports']
  )(mockGraphqlRequest, exports);
  return exports;
};

// Load commerceNavItems from nav.ts
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace(/import\s+type\s+\{\s*NavItem\s*\}\s+from\s+'@\/types';/g, '')
    .replaceAll('export ', '')
);

const indexModule = {};
compileFunction(
  `${cleanNavSource}\nindexModule.commerceNavItems = commerceNavItems;`,
  ['indexModule']
)(indexModule);
const { commerceNavItems } = indexModule;

test('commerceNavItems specifies manager role, plugin group, and all 5 sub-routes', () => {
  assert.equal(commerceNavItems.length, 1);
  const root = commerceNavItems[0];
  assert.equal(root.title, 'Commerce');
  assert.equal(root.url, '#');
  assert.equal(root.group, 'modulePlugins');
  assert.deepEqual(root.access, { role: 'manager' });

  assert.equal(root.items.length, 8);
  const items = root.items.map((i) => ({ title: i.title, url: i.url }));
  assert.deepEqual(items, [
    { title: 'Orders', url: '/dashboard/commerce/orders' },
    { title: 'Payments', url: '/dashboard/commerce/payments' },
    { title: 'Refunds', url: '/dashboard/commerce/refunds' },
    { title: 'Fulfillments', url: '/dashboard/commerce/fulfillments' },
    {
      title: 'Shipping Profiles',
      url: '/dashboard/commerce/shipping-profiles'
    },
    { title: 'Cart Promotions', url: '/dashboard/commerce/promotions' },
    { title: 'Order Changes', url: '/dashboard/commerce/order-changes' },
    { title: 'Return Decisions', url: '/dashboard/commerce/return-decisions' }
  ]);
});

test('API functions enforce fail-closed credential checks', async () => {
  const api = createApiModule(async () => ({}));
  const emptyOpts = {};

  await assert.rejects(
    () => api.listOrders(emptyOpts),
    /Sign in again to view orders\./
  );

  await assert.rejects(
    () => api.getOrderDetail(emptyOpts, 'ord-1'),
    /Sign in again to view orders\./
  );

  await assert.rejects(
    () => api.listPaymentCollections(emptyOpts),
    /Sign in again to manage payments\./
  );

  await assert.rejects(
    () => api.listRefunds(emptyOpts),
    /Sign in again to manage refunds\./
  );

  await assert.rejects(
    () => api.listFulfillments(emptyOpts),
    /Sign in again to manage fulfillments\./
  );

  await assert.rejects(
    () => api.listShippingProfiles(emptyOpts),
    /Sign in again to manage shipping profiles\./
  );

  await assert.rejects(
    () =>
      api.previewCartPromotion(emptyOpts, 'cart-1', {
        sourceId: 'p1',
        kind: 'PERCENT',
        scope: 'CART'
      }),
    /Sign in again to manage promotions\./
  );

  await assert.rejects(
    () => api.listOrderChanges(emptyOpts),
    /Sign in again to manage order changes\./
  );

  await assert.rejects(
    () =>
      api.createOrderReturnDecision(emptyOpts, 'ord-1', {
        returnRequest: {},
        decision: { action: 'return_only' }
      }),
    /Sign in again to manage return decisions\./
  );
});

test('listOrders correctly passes variables, headers, and extracts orders list', async () => {
  const mockOrders = {
    total: 1,
    page: 1,
    perPage: 10,
    hasNext: false,
    items: [
      {
        id: 'ord-123',
        customerId: 'cust-456',
        status: 'PAID',
        currencyCode: 'USD',
        totalAmount: '49.99',
        trackingNumber: 'TRACK-1',
        carrier: 'DHL',
        createdAt: '2026-10-31T00:00:00Z',
        confirmedAt: null,
        paidAt: '2026-10-31T00:05:00Z',
        shippedAt: null,
        deliveredAt: null,
        cancelledAt: null,
        lineItems: []
      }
    ]
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /CommerceAdminOrders/);
    assert.equal(vars.tenantId, 'ten-uuid-1');
    assert.deepEqual(vars.filter, { status: 'PAID' });
    assert.equal(token, 'jwt-token');
    assert.equal(tenant, 'store-slug');
    return { orders: mockOrders };
  });

  const res = await api.listOrders(
    { token: 'jwt-token', tenantSlug: 'store-slug', tenantId: 'ten-uuid-1' },
    { status: 'PAID' }
  );

  assert.deepEqual(res, mockOrders);
});

test('getOrderDetail requests the admin order detail projection and passes null through', async () => {
  const mockDetail = {
    order: {
      id: 'ord-123',
      customerId: 'cust-456',
      status: 'SHIPPED',
      currencyCode: 'USD',
      subtotalAmount: '40.00',
      adjustmentTotal: '0.00',
      shippingTotal: '5.00',
      totalAmount: '49.99',
      taxTotal: '4.99',
      taxIncluded: true,
      lineItems: [],
      adjustments: [],
      taxLines: []
    },
    paymentCollection: {
      id: 'pc-1',
      status: 'captured',
      currencyCode: 'USD',
      amount: '49.99',
      payments: [],
      refunds: []
    },
    fulfillment: null
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    assert.match(query, /CommerceAdminOrder/);
    assert.match(query, /order\(tenantId: \$tenantId, id: \$id\)/);
    assert.match(query, /paymentCollection/);
    assert.match(query, /fulfillment/);
    assert.equal(vars.tenantId, 'ten-uuid-1');
    assert.equal(vars.id, 'ord-123');
    assert.equal(token, 'jwt-token');
    assert.equal(tenant, 'store-slug');
    return { order: mockDetail };
  });

  const res = await api.getOrderDetail(
    { token: 'jwt-token', tenantSlug: 'store-slug', tenantId: 'ten-uuid-1' },
    'ord-123'
  );

  assert.deepEqual(res, mockDetail);

  const nullApi = createApiModule(async () => ({ order: null }));
  const missing = await nullApi.getOrderDetail(
    { token: 'jwt-token', tenantSlug: 'store-slug', tenantId: 'ten-uuid-1' },
    'ord-404'
  );
  assert.equal(missing, null);
});

test('listPaymentCollections, listRefunds, and listFulfillments dispatch typed list queries', async () => {
  const opts = { token: 'jwt-token', tenantSlug: 'store-slug', tenantId: 'ten-uuid-1' };
  const calls = [];

  const api = createApiModule(async (query, vars, token, tenant) => {
    calls.push(query);
    assert.equal(vars.tenantId, 'ten-uuid-1');
    assert.equal(token, 'jwt-token');
    assert.equal(tenant, 'store-slug');
    if (query.includes('CommerceAdminPaymentCollections')) {
      assert.deepEqual(vars.filter, { status: 'captured', orderId: 'ord-1' });
      return {
        paymentCollections: {
          items: [{ id: 'pc-1', status: 'captured', payments: [], refunds: [] }],
          total: 1,
          page: 1,
          perPage: 20,
          hasNext: false
        }
      };
    }
    if (query.includes('CommerceAdminRefunds')) {
      assert.deepEqual(vars.filter, { status: 'refunded' });
      return {
        refunds: {
          items: [{ id: 'ref-1', status: 'refunded', amount: '9.99' }],
          total: 1,
          page: 1,
          perPage: 20,
          hasNext: false
        }
      };
    }
    assert.match(query, /CommerceAdminFulfillments/);
    assert.deepEqual(vars.filter, { orderId: 'ord-1' });
    return {
      fulfillments: {
        items: [{ id: 'ff-1', orderId: 'ord-1', status: 'shipped', items: [] }],
        total: 1,
        page: 1,
        perPage: 20,
        hasNext: false
      }
    };
  });

  const collections = await api.listPaymentCollections(opts, {
    status: 'captured',
    orderId: 'ord-1'
  });
  assert.equal(collections.total, 1);
  assert.equal(collections.items[0].id, 'pc-1');

  const refunds = await api.listRefunds(opts, { status: 'refunded' });
  assert.equal(refunds.items[0].id, 'ref-1');

  const fulfillments = await api.listFulfillments(opts, { orderId: 'ord-1' });
  assert.equal(fulfillments.items[0].orderId, 'ord-1');

  assert.equal(calls.length, 3);
  assert.match(calls[0], /paymentCollections\(tenantId: \$tenantId, filter: \$filter\)/);
  assert.match(calls[1], /refunds\(tenantId: \$tenantId, filter: \$filter\)/);
  assert.match(calls[2], /fulfillments\(tenantId: \$tenantId, filter: \$filter\)/);
});

test('listShippingProfiles and createShippingProfile pass correct payload and headers', async () => {
  const mockProfile = {
    id: 'sp-1',
    slug: 'standard',
    active: true,
    metadata: '{}',
    createdAt: null,
    updatedAt: null,
    translations: [
      { locale: 'en', name: 'Standard Shipping', description: null }
    ]
  };

  const api = createApiModule(async (query, vars, token, tenant) => {
    if (query.includes('CommerceAdminShippingProfiles')) {
      assert.equal(vars.tenantId, 'ten-uuid-1');
      assert.equal(token, 'jwt-token');
      assert.equal(tenant, 'store-slug');
      return {
        shippingProfiles: {
          items: [mockProfile],
          total: 1,
          page: 1,
          perPage: 10,
          hasNext: false
        }
      };
    }
    if (query.includes('CommerceAdminCreateShippingProfile')) {
      assert.equal(vars.tenantId, 'ten-uuid-1');
      assert.equal(vars.input.slug, 'express');
      return { createShippingProfile: { ...mockProfile, slug: 'express' } };
    }
    throw new Error('Unexpected query');
  });

  const opts = {
    token: 'jwt-token',
    tenantSlug: 'store-slug',
    tenantId: 'ten-uuid-1'
  };
  const list = await api.listShippingProfiles(opts, { page: 1, perPage: 10 });
  assert.equal(list.total, 1);
  assert.equal(list.items[0].slug, 'standard');

  const created = await api.createShippingProfile(opts, {
    slug: 'express',
    translations: [{ locale: 'en', name: 'Express', description: null }]
  });
  assert.equal(created.slug, 'express');
});

test('previewCartPromotion and applyCartPromotion dispatch typed mutations', async () => {
  const api = createApiModule(async (query, vars) => {
    if (query.includes('CommerceAdminPreviewCartPromotion')) {
      assert.equal(vars.cartId, 'cart-99');
      assert.equal(vars.input.kind, 'PERCENTAGE_DISCOUNT');
      return {
        previewAdminCartPromotion: {
          kind: 'PERCENTAGE_DISCOUNT',
          scope: 'CART',
          lineItemId: null,
          currencyCode: 'USD',
          baseAmount: '100.00',
          adjustmentAmount: '15.00',
          adjustedAmount: '85.00'
        }
      };
    }
    if (query.includes('CommerceAdminApplyCartPromotion')) {
      assert.equal(vars.cartId, 'cart-99');
      return {
        applyAdminCartPromotion: {
          id: 'cart-99',
          currencyCode: 'USD',
          shippingTotal: '5.00',
          adjustmentTotal: '15.00',
          totalAmount: '90.00',
          adjustments: []
        }
      };
    }
    throw new Error('Unexpected query');
  });

  const opts = { token: 'tok', tenantSlug: 'slug', tenantId: 'ten-1' };
  const preview = await api.previewCartPromotion(opts, 'cart-99', {
    sourceId: 'promo1',
    kind: 'PERCENTAGE_DISCOUNT',
    scope: 'CART'
  });
  assert.equal(preview.adjustmentAmount, '15.00');

  const applied = await api.applyCartPromotion(opts, 'cart-99', {
    sourceId: 'promo1',
    kind: 'PERCENTAGE_DISCOUNT',
    scope: 'CART'
  });
  assert.equal(applied.totalAmount, '90.00');
});

test('GraphQL queries and mutations define expected schema contracts', () => {
  const api = createApiModule(async () => ({}));

  assert.match(
    api.ORDERS_QUERY,
    /orders\(tenantId: \$tenantId, filter: \$filter\)/
  );
  assert.match(api.ORDERS_QUERY, /totalAmount/);
  assert.match(api.ORDERS_QUERY, /lineItems/);

  assert.match(
    api.CREATE_SHIPPING_PROFILE_MUTATION,
    /\$input: CreateShippingProfileInput!/
  );
  assert.match(
    api.UPDATE_SHIPPING_PROFILE_MUTATION,
    /\$input: UpdateShippingProfileInput!/
  );
  assert.match(
    api.SHIPPING_PROFILES_QUERY,
    /shippingProfiles\(tenantId: \$tenantId/
  );
  assert.match(api.SHIPPING_PROFILES_QUERY, /translations/);

  assert.match(api.ORDER_CHANGES_QUERY, /orderChanges\(tenantId: \$tenantId/);
  assert.match(api.ORDER_CHANGES_QUERY, /changeType/);
  assert.match(api.ORDER_CHANGES_QUERY, /status/);

  assert.match(
    api.CREATE_ORDER_RETURN_DECISION_MUTATION,
    /createOrderReturnDecision/
  );
  assert.match(api.CREATE_ORDER_RETURN_DECISION_MUTATION, /orderReturn/);
  assert.match(api.CREATE_ORDER_RETURN_DECISION_MUTATION, /refund/);
});
