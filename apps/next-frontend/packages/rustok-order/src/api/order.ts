/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { storefrontGraphql } from "@/shared/lib/graphql";
import type { Order } from "./types";

export type OrderGraphqlExecutor = typeof storefrontGraphql;

const ORDER_FIELDS = `
  id
  status
  currencyCode
  subtotalAmount
  adjustmentTotal
  shippingTotal
  totalAmount
  taxTotal
  taxIncluded
  metadata
  paymentId
  paymentMethod
  trackingNumber
  carrier
  cancellationReason
  deliveredSignature
  createdAt
  updatedAt
  confirmedAt
  paidAt
  shippedAt
  deliveredAt
  cancelledAt
  lineItems {
    id
    productId
    variantId
    sku
    title
    quantity
    unitPrice
    totalPrice
    currencyCode
  }
  adjustments {
    id
    lineItemId
    sourceType
    sourceId
    amount
    currencyCode
  }
`;

const STOREFRONT_ORDER_QUERY = `
  query StorefrontOrder($id: UUID!) {
    storefrontOrder(id: $id) {
      ${ORDER_FIELDS}
    }
  }
`;

export async function fetchStorefrontOrder(
  graphql: OrderGraphqlExecutor,
  orderId: string,
  tenantSlug?: string | null,
): Promise<Order | null> {
  const cleanId = orderId.trim();
  if (!cleanId) return null;

  try {
    const response = await graphql<{
      storefrontOrder: Order | null;
    }, { id: string }>({
      query: STOREFRONT_ORDER_QUERY,
      variables: { id: cleanId },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.storefrontOrder ?? null;
  } catch (error) {
    console.error("Failed to fetch storefront order:", error);
    return null;
  }
}
