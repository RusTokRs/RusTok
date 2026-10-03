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
import type {
  Cart,
  CompleteCheckoutResult,
  CompleteStorefrontCheckoutInput,
  StorefrontShippingSelectionInput,
} from "./types";

export type CartGraphqlExecutor = typeof storefrontGraphql;

const CART_FIELDS = `
  id
  currencyCode
  subtotalAmount
  adjustmentTotal
  shippingTotal
  totalAmount
  taxTotal
  status
  email
  regionId
  countryCode
  localeCode
  selectedShippingOptionId
  lineItems {
    id
    cartId
    productId
    variantId
    sku
    title
    quantity
    unitPrice
    totalPrice
    currencyCode
    shippingProfileSlug
    sellerId
  }
  adjustments {
    id
    cartId
    lineItemId
    sourceType
    sourceId
    adjustedAmount
  }
  deliveryGroups {
    shippingProfileSlug
    sellerId
    selectedShippingOptionId
    availableShippingOptions {
      id
      name
      currencyCode
      amount
      providerId
      active
    }
  }
`;

const STOREFRONT_CART_QUERY = `
  query StorefrontCart($id: UUID!) {
    storefrontCart(id: $id) {
      ${CART_FIELDS}
    }
  }
`;

const CREATE_STOREFRONT_CART_MUTATION = `
  mutation CreateStorefrontCart($input: CreateStorefrontCartInput!) {
    createStorefrontCart(input: $input) {
      cart {
        ${CART_FIELDS}
      }
    }
  }
`;

const ADD_CART_LINE_ITEM_MUTATION = `
  mutation AddStorefrontCartLineItem($cartId: UUID!, $input: AddStorefrontCartLineItemInput!) {
    addStorefrontCartLineItem(cartId: $cartId, input: $input) {
      ${CART_FIELDS}
    }
  }
`;

const UPDATE_CART_LINE_ITEM_MUTATION = `
  mutation UpdateStorefrontCartLineItemQuantity(
    $cartId: UUID!
    $lineItemId: UUID!
    $quantity: Int!
  ) {
    updateStorefrontCartLineItemQuantity(
      cartId: $cartId
      lineItemId: $lineItemId
      quantity: $quantity
    ) {
      ${CART_FIELDS}
    }
  }
`;

const REMOVE_CART_LINE_ITEM_MUTATION = `
  mutation RemoveStorefrontCartLineItem($cartId: UUID!, $lineItemId: UUID!) {
    removeStorefrontCartLineItem(cartId: $cartId, lineItemId: $lineItemId) {
      ${CART_FIELDS}
    }
  }
`;

export async function fetchStorefrontCart(
  graphql: CartGraphqlExecutor,
  cartId: string,
  tenantSlug?: string | null,
): Promise<Cart | null> {
  const cleanId = cartId.trim();
  if (!cleanId) return null;

  try {
    const response = await graphql<{
      storefrontCart: Cart | null;
    }, { id: string }>({
      query: STOREFRONT_CART_QUERY,
      variables: { id: cleanId },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.storefrontCart ?? null;
  } catch {
    return null;
  }
}

export async function createStorefrontCart(
  graphql: CartGraphqlExecutor,
  input: {
    currencyCode?: string;
    locale?: string;
  } = {},
  tenantSlug?: string | null,
): Promise<Cart | null> {
  try {
    const response = await graphql<{
      createStorefrontCart: { cart: Cart };
    }, {
      input: {
        currencyCode?: string;
        locale?: string;
      };
    }>({
      query: CREATE_STOREFRONT_CART_MUTATION,
      variables: {
        input: {
          currencyCode: input.currencyCode,
          locale: input.locale,
        },
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.createStorefrontCart.cart ?? null;
  } catch {
    return null;
  }
}

export async function addStorefrontCartLineItem(
  graphql: CartGraphqlExecutor,
  cartId: string,
  variantId: string,
  quantity = 1,
  tenantSlug?: string | null,
): Promise<Cart | null> {
  try {
    const response = await graphql<{
      addStorefrontCartLineItem: Cart;
    }, {
      cartId: string;
      input: {
        variantId: string;
        quantity: number;
      };
    }>({
      query: ADD_CART_LINE_ITEM_MUTATION,
      variables: {
        cartId,
        input: {
          variantId,
          quantity,
        },
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.addStorefrontCartLineItem ?? null;
  } catch (error) {
    console.error("Failed to add line item to cart:", error);
    return null;
  }
}

export async function updateStorefrontCartLineItemQuantity(
  graphql: CartGraphqlExecutor,
  cartId: string,
  lineItemId: string,
  quantity: number,
  tenantSlug?: string | null,
): Promise<Cart | null> {
  try {
    const response = await graphql<{
      updateStorefrontCartLineItemQuantity: Cart;
    }, {
      cartId: string;
      lineItemId: string;
      quantity: number;
    }>({
      query: UPDATE_CART_LINE_ITEM_MUTATION,
      variables: {
        cartId,
        lineItemId,
        quantity,
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.updateStorefrontCartLineItemQuantity ?? null;
  } catch (error) {
    console.error("Failed to update cart line item quantity:", error);
    return null;
  }
}

export async function removeStorefrontCartLineItem(
  graphql: CartGraphqlExecutor,
  cartId: string,
  lineItemId: string,
  tenantSlug?: string | null,
): Promise<Cart | null> {
  try {
    const response = await graphql<{
      removeStorefrontCartLineItem: Cart;
    }, {
      cartId: string;
      lineItemId: string;
    }>({
      query: REMOVE_CART_LINE_ITEM_MUTATION,
      variables: {
        cartId,
        lineItemId,
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.removeStorefrontCartLineItem ?? null;
  } catch (error) {
    console.error("Failed to remove cart line item:", error);
    return null;
  }
}

const UPDATE_STOREFRONT_CART_SHIPPING_MUTATION = `
  mutation UpdateStorefrontCartShipping(
    $cartId: UUID!
    $input: UpdateStorefrontCartContextInput!
  ) {
    updateStorefrontCartContext(cartId: $cartId, input: $input) {
      cart {
        ${CART_FIELDS}
      }
    }
  }
`;

export async function updateStorefrontCartShipping(
  graphql: CartGraphqlExecutor,
  cartId: string,
  shippingOptionId: string,
  shippingSelections?: StorefrontShippingSelectionInput[],
  tenantSlug?: string | null,
): Promise<Cart | null> {
  try {
    const response = await graphql<{
      updateStorefrontCartContext: { cart: Cart };
    }, {
      cartId: string;
      input: {
        selectedShippingOptionId: string;
        shippingSelections?: StorefrontShippingSelectionInput[];
      };
    }>({
      query: UPDATE_STOREFRONT_CART_SHIPPING_MUTATION,
      variables: {
        cartId,
        input: {
          selectedShippingOptionId: shippingOptionId,
          ...(shippingSelections && shippingSelections.length > 0 ? { shippingSelections } : {}),
        },
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.updateStorefrontCartContext.cart ?? null;
  } catch (error) {
    console.error("Failed to update storefront cart shipping option:", error);
    return null;
  }
}

const COMPLETE_STOREFRONT_CHECKOUT_MUTATION = `
  mutation CompleteStorefrontCheckout(
    $idempotencyKey: String!
    $input: CompleteStorefrontCheckoutInput!
  ) {
    completeStorefrontCheckout(
      idempotencyKey: $idempotencyKey
      input: $input
    ) {
      cart {
        ${CART_FIELDS}
      }
      order {
        id
        tenantId
        channelId
        channelSlug
        customerId
        status
        currencyCode
        subtotalAmount
        adjustmentTotal
        shippingTotal
        totalAmount
        taxTotal
        taxIncluded
        metadata
      }
      paymentCollection {
        id
        status
        currencyCode
        amount
      }
    }
  }
`;

export async function completeStorefrontCheckout(
  graphql: CartGraphqlExecutor,
  input: CompleteStorefrontCheckoutInput,
  tenantSlug?: string | null,
): Promise<CompleteCheckoutResult | null> {
  const idempotencyKey = typeof crypto !== "undefined" && crypto.randomUUID
    ? crypto.randomUUID()
    : `checkout-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  try {
    const response = await graphql<{
      completeStorefrontCheckout: CompleteCheckoutResult;
    }, {
      idempotencyKey: string;
      input: CompleteStorefrontCheckoutInput;
    }>({
      query: COMPLETE_STOREFRONT_CHECKOUT_MUTATION,
      variables: {
        idempotencyKey,
        input,
      },
      tenant: tenantSlug ?? undefined,
    });

    return response.data?.completeStorefrontCheckout ?? null;
  } catch (error) {
    console.error("Failed to complete storefront checkout:", error);
    throw error;
  }
}

