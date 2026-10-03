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
  shippingProfileSlug?: string | null;
  sellerId?: string | null;
}

export interface CartAdjustment {
  id: string;
  cartId: string;
  lineItemId?: string | null;
  sourceType: string;
  sourceId?: string | null;
  adjustedAmount: string;
}

export interface CartShippingOptionSummary {
  id: string;
  name: string;
  currencyCode: string;
  amount: string;
  providerId: string;
  active: boolean;
}

export interface CartDeliveryGroup {
  shippingProfileSlug: string;
  sellerId?: string | null;
  lineItemIds?: string[];
  selectedShippingOptionId?: string | null;
  availableShippingOptions: CartShippingOptionSummary[];
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
  email?: string | null;
  regionId?: string | null;
  countryCode?: string | null;
  localeCode?: string | null;
  selectedShippingOptionId?: string | null;
  lineItems: CartLineItem[];
  adjustments: CartAdjustment[];
  deliveryGroups?: CartDeliveryGroup[];
}

export interface StorefrontShippingSelectionInput {
  shippingProfileSlug: string;
  sellerId?: string | null;
  selectedShippingOptionId?: string | null;
}

export interface CompleteStorefrontCheckoutInput {
  cartId: string;
  shippingOptionId?: string | null;
  shippingSelections?: StorefrontShippingSelectionInput[];
  regionId?: string | null;
  countryCode?: string | null;
  locale?: string | null;
  createFulfillment?: boolean;
  metadata?: string | null;
}

export interface OrderSummary {
  id: string;
  tenantId: string;
  channelId?: string | null;
  channelSlug?: string | null;
  customerId?: string | null;
  status: string;
  currencyCode: string;
  subtotalAmount: string;
  adjustmentTotal: string;
  shippingTotal: string;
  totalAmount: string;
  taxTotal: string;
  taxIncluded: boolean;
  metadata?: string | null;
}

export interface PaymentCollectionSummary {
  id: string;
  status: string;
  currencyCode: string;
  amount: string;
}

export interface CompleteCheckoutResult {
  cart: Cart;
  order: OrderSummary;
  paymentCollection: PaymentCollectionSummary;
}

export interface CustomerShippingAddress {
  fullName: string;
  email: string;
  phone: string;
  countryCode: string;
  city: string;
  streetAddress: string;
  postalCode: string;
  paymentMethod: "card" | "cod" | "transfer";
  notes?: string;
}
