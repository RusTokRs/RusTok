/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export interface OrderLineItem {
  id: string;
  orderId?: string;
  productId?: string | null;
  variantId?: string | null;
  sku?: string | null;
  title: string;
  quantity: number;
  unitPrice: string;
  totalPrice: string;
  currencyCode: string;
}

export interface OrderAdjustment {
  id: string;
  lineItemId?: string | null;
  sourceType: string;
  sourceId?: string | null;
  amount: string;
  currencyCode: string;
}

export type OrderStatus =
  | "PENDING"
  | "CONFIRMED"
  | "PROCESSING"
  | "SHIPPED"
  | "DELIVERED"
  | "CANCELLED";

export interface OrderCustomerDetails {
  fullName?: string;
  email?: string;
  phone?: string;
}

export interface OrderShippingAddress {
  countryCode?: string;
  city?: string;
  streetAddress?: string;
  postalCode?: string;
}

export interface OrderMetadataParsed {
  customer?: OrderCustomerDetails;
  shippingAddress?: OrderShippingAddress;
  shippingMethod?: {
    id?: string;
    name?: string;
    amount?: string;
  };
  paymentMethod?: string;
  notes?: string;
}

export interface Order {
  id: string;
  status: OrderStatus | string;
  currencyCode: string;
  subtotalAmount: string;
  adjustmentTotal: string;
  shippingTotal: string;
  totalAmount: string;
  taxTotal: string;
  taxIncluded: boolean;
  metadata?: string | null;
  paymentId?: string | null;
  paymentMethod?: string | null;
  trackingNumber?: string | null;
  carrier?: string | null;
  cancellationReason?: string | null;
  deliveredSignature?: string | null;
  createdAt: string;
  updatedAt: string;
  confirmedAt?: string | null;
  paidAt?: string | null;
  shippedAt?: string | null;
  deliveredAt?: string | null;
  cancelledAt?: string | null;
  lineItems: OrderLineItem[];
  adjustments?: OrderAdjustment[];
}

export interface OrderListResponse {
  items: Order[];
  total: number;
  page: number;
  perPage: number;
  hasNext: boolean;
}

export interface OrdersFilterParams {
  page?: number;
  perPage?: number;
  status?: string;
  token?: string;
}

