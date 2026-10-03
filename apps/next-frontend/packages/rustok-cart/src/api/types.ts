/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface CartLineItem {
  id: string;
  cartId: string;
  productId?: string | null;
  variantId?: string | null;
  sku?: string | null;
  title: string;
  quantity: number;
  unitPrice: string;
  totalPrice: string;
  currencyCode: string;
}

export interface CartAdjustment {
  id: string;
  cartId: string;
  lineItemId?: string | null;
  sourceType: string;
  sourceId?: string | null;
  adjustedAmount: string;
}

export interface Cart {
  id: string;
  currencyCode: string;
  subtotalAmount: string;
  adjustmentTotal: string;
  shippingTotal: string;
  totalAmount: string;
  taxTotal: string;
  status: string;
  lineItems: CartLineItem[];
  adjustments: CartAdjustment[];
}
