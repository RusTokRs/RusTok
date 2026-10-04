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

const STOREFRONT_ORDERS_QUERY = `
  query StorefrontOrders($page: Int, $perPage: Int, $status: String) {
    storefrontOrders(page: $page, perPage: $perPage, status: $status) {
      items {
        ${ORDER_FIELDS}
      }
      total
      page
      perPage
      hasNext
    }
  }
`;

export async function fetchStorefrontOrder(
  graphql: OrderGraphqlExecutor,
  orderId: string,
  tenantSlug?: string | null,
  token?: string | null,
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
      token: token ?? undefined,
    });

    return response.data?.storefrontOrder ?? null;
  } catch (error) {
    console.error("Failed to fetch storefront order:", error);
    return null;
  }
}

export async function fetchStorefrontOrders(
  graphql: OrderGraphqlExecutor,
  params: import("./types").OrdersFilterParams = {},
  tenantSlug?: string | null,
  token?: string | null,
): Promise<import("./types").OrderListResponse | null> {
  try {
    const response = await graphql<{
      storefrontOrders: import("./types").OrderListResponse;
    }, { page?: number; perPage?: number; status?: string }>({
      query: STOREFRONT_ORDERS_QUERY,
      variables: {
        page: params.page,
        perPage: params.perPage,
        status: params.status,
      },
      tenant: tenantSlug ?? undefined,
      token: token ?? params.token ?? undefined,
    });

    return response.data?.storefrontOrders ?? null;
  } catch (error) {
    console.error("Failed to fetch storefront orders:", error);
    return null;
  }
}

const LOCAL_STORAGE_ORDER_HISTORY_KEY = "rustok_customer_recent_orders";

export function getLocalRecentOrderIds(): string[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = localStorage.getItem(LOCAL_STORAGE_ORDER_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((id) => typeof id === "string" && id.trim())
      : [];
  } catch {
    return [];
  }
}

export function saveLocalRecentOrderId(orderId: string): void {
  if (typeof window === "undefined") return;
  const cleanId = orderId.trim();
  if (!cleanId) return;
  try {
    const current = getLocalRecentOrderIds();
    const updated = [cleanId, ...current.filter((id) => id !== cleanId)].slice(0, 25);
    localStorage.setItem(LOCAL_STORAGE_ORDER_HISTORY_KEY, JSON.stringify(updated));
  } catch {
    // ignore localStorage exceptions
  }
}

export async function fetchGuestRecentOrders(
  graphql: OrderGraphqlExecutor,
  orderIds: string[],
  tenantSlug?: string | null,
): Promise<Order[]> {
  if (!orderIds || orderIds.length === 0) return [];
  const results = await Promise.all(
    orderIds.map((id) => fetchStorefrontOrder(graphql, id, tenantSlug))
  );
  return results.filter((order): order is Order => order !== null);
}

