/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { normalizeLocaleTag } from "@rustok/next-fluent/utils";
import {
  normalizePricingQuery,
  parsePricingUuid,
} from "@rustok/pricing-frontend";
import type { Cart, CartPricingExpectation } from "./api/types";

/** Existing persisted key is adopted only after an authenticated, tenant-scoped owner read. */
export const UNSCOPED_CART_STORAGE_KEY = "rustok_storefront_cart_id";

export async function cartStorageKey(
  tenant: string | null,
  token: string | null,
): Promise<string> {
  const fingerprint =
    token === null
      ? "guest"
      : Array.from(
          new Uint8Array(
            await globalThis.crypto.subtle.digest(
              "SHA-256",
              new TextEncoder().encode(token),
            ),
          ),
          (byte) => byte.toString(16).padStart(2, "0"),
        ).join("");
  return `rustok_storefront_cart_id:${encodeURIComponent(tenant ?? "host")}:${fingerprint}`;
}

/** Separate warning only: the ID remains the sole local reference; neither key grants access. */
export function cartMutationMarkerKey(storageKey: string): string {
  return `${storageKey}:mutation`;
}
export function cartReferenceFromSearch(search: string): string | null {
  const values = new URLSearchParams(search).getAll("cart_id");
  if (values.length > 1)
    throw new Error("Cart reference must be single-valued");
  return values.length ? parsePricingUuid(values[0]) : null;
}

export function isActiveCart(cart: Cart): boolean {
  return cart.status === "active";
}

export function validCartQuantity(quantity: number): boolean {
  return Number.isInteger(quantity) && quantity > 0 && quantity <= 2147483647;
}

export function validCartPricingExpectation(
  context: CartPricingExpectation,
  quantity: number,
): boolean {
  try {
    normalizePricingQuery(context);
    return (
      validCartQuantity(quantity) &&
      context.quantity === quantity &&
      !!normalizeLocaleTag(context.locale) &&
      context.locale.length <= 64 &&
      /^[A-Z]{3}$/.test(context.currencyCode) &&
      context.priceListId === null
    );
  } catch {
    return false;
  }
}
export function validCartBuyerSelection(
  variantId: string,
  context: CartPricingExpectation,
  quantity: number,
): boolean {
  try {
    parsePricingUuid(variantId);
    return validCartPricingExpectation(context, quantity);
  } catch {
    return false;
  }
}

/** This is an early UX refusal only. Commerce compares the normalized owner quote and Cart locks
 * its original persisted context before insertion. Missing stored channel halves are not invented. */
export function cartMatchesPricingExpectation(
  cart: Cart,
  expected: CartPricingExpectation,
): boolean {
  return (
    isActiveCart(cart) &&
    expected.priceListId === null &&
    !!normalizeLocaleTag(cart.localeCode) &&
    normalizeLocaleTag(cart.localeCode) ===
      normalizeLocaleTag(expected.locale) &&
    cart.currencyCode === expected.currencyCode &&
    (cart.regionId ?? null) === expected.regionId &&
    (cart.channelId == null || cart.channelId === expected.channelId) &&
    (cart.channelSlug == null ||
      cart.channelSlug.toLowerCase() === expected.channelSlug?.toLowerCase()) &&
    (cart.channelId != null ||
      cart.channelSlug != null ||
      (expected.channelId === null && expected.channelSlug === null))
  );
}

export function pricingExpectationFromCart(
  cart: Cart,
  quantity: number,
): CartPricingExpectation {
  return {
    locale: cart.localeCode ?? "",
    currencyCode: cart.currencyCode,
    regionId: cart.regionId ?? null,
    priceListId: null,
    channelId: cart.channelId ?? null,
    channelSlug: cart.channelSlug ?? null,
    quantity,
  };
}

/** Null is a successful missing-cart read, not a transport failure. Inactive/unknown references are retained for explicit inspection or reset. */
export function adoptBuyerCart(
  cart: Cart | null,
  reference: string,
): Cart | null {
  // Validate the requested reference even for a successful missing response.
  const requestedId = parsePricingUuid(reference);
  if (!cart) return null;
  if (parsePricingUuid(cart.id) !== requestedId)
    throw new Error("Cart reference does not match the owner response");
  if (!isActiveCart(cart)) throw new Error("Cart is not active");
  return cart;
}
