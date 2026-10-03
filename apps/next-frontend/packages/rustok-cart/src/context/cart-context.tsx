/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  addStorefrontCartLineItem,
  createStorefrontCart,
  fetchStorefrontCart,
  removeStorefrontCartLineItem,
  updateStorefrontCartLineItemQuantity,
  updateStorefrontCartShipping,
} from "../api/cart";
import type { Cart } from "../api/types";

const CART_STORAGE_KEY = "rustok_storefront_cart_id";

export interface CartContextValue {
  cart: Cart | null;
  isOpen: boolean;
  isLoading: boolean;
  isUpdating: boolean;
  itemCount: number;
  openCart: () => void;
  closeCart: () => void;
  toggleCart: () => void;
  addItem: (variantId: string, quantity?: number) => Promise<boolean>;
  updateQuantity: (lineItemId: string, quantity: number) => Promise<boolean>;
  removeItem: (lineItemId: string) => Promise<boolean>;
  refreshCart: () => Promise<void>;
  selectShippingOption: (shippingOptionId: string) => Promise<boolean>;
  clearCart: () => void;
}

const CartContext = createContext<CartContextValue | undefined>(undefined);

export interface CartProviderProps {
  children: ReactNode;
  locale?: string;
  currencyCode?: string;
  tenantSlug?: string | null;
}

export function CartProvider({
  children,
  locale = "ru",
  currencyCode = "RUB",
  tenantSlug = null,
}: CartProviderProps) {
  const [cart, setCart] = useState<Cart | null>(null);
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isUpdating, setIsUpdating] = useState<boolean>(false);

  // Load existing cart from localStorage on client mount
  useEffect(() => {
    let isCancelled = false;

    async function initializeCart() {
      try {
        const storedCartId = window.localStorage.getItem(CART_STORAGE_KEY);
        if (!storedCartId) return;

        setIsLoading(true);
        const fetchedCart = await fetchStorefrontCart(
          storefrontGraphql,
          storedCartId,
          tenantSlug,
        );

        if (isCancelled) return;

        if (fetchedCart && fetchedCart.status === "ACTIVE") {
          setCart(fetchedCart);
        } else {
          // Cart not found or expired; clean up storage
          window.localStorage.removeItem(CART_STORAGE_KEY);
          setCart(null);
        }
      } catch (err) {
        console.error("Failed to restore storefront cart:", err);
      } finally {
        if (!isCancelled) {
          setIsLoading(false);
        }
      }
    }

    void initializeCart();

    return () => {
      isCancelled = true;
    };
  }, [tenantSlug]);

  const openCart = useCallback(() => {
    setIsOpen(true);
  }, []);

  const closeCart = useCallback(() => {
    setIsOpen(false);
  }, []);

  const toggleCart = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  const refreshCart = useCallback(async () => {
    if (!cart?.id) return;
    setIsUpdating(true);
    try {
      const refreshed = await fetchStorefrontCart(
        storefrontGraphql,
        cart.id,
        tenantSlug,
      );
      if (refreshed) {
        setCart(refreshed);
      }
    } finally {
      setIsUpdating(false);
    }
  }, [cart?.id, tenantSlug]);

  const addItem = useCallback(
    async (variantId: string, quantity = 1): Promise<boolean> => {
      setIsUpdating(true);
      try {
        let activeCartId = cart?.id;

        // Create new cart if none exists
        if (!activeCartId) {
          const newCart = await createStorefrontCart(
            storefrontGraphql,
            { currencyCode, locale },
            tenantSlug,
          );
          if (!newCart) {
            console.error("Could not create storefront cart");
            return false;
          }
          activeCartId = newCart.id;
          window.localStorage.setItem(CART_STORAGE_KEY, activeCartId);
          setCart(newCart);
        }

        const updated = await addStorefrontCartLineItem(
          storefrontGraphql,
          activeCartId,
          variantId,
          quantity,
          tenantSlug,
        );

        if (updated) {
          setCart(updated);
          return true;
        }
        return false;
      } catch (error) {
        console.error("Failed to add item to cart:", error);
        return false;
      } finally {
        setIsUpdating(false);
      }
    },
    [cart?.id, currencyCode, locale, tenantSlug],
  );

  const updateQuantity = useCallback(
    async (lineItemId: string, quantity: number): Promise<boolean> => {
      if (!cart?.id) return false;
      setIsUpdating(true);
      try {
        if (quantity <= 0) {
          const updated = await removeStorefrontCartLineItem(
            storefrontGraphql,
            cart.id,
            lineItemId,
            tenantSlug,
          );
          if (updated) {
            setCart(updated);
            return true;
          }
          return false;
        }

        const updated = await updateStorefrontCartLineItemQuantity(
          storefrontGraphql,
          cart.id,
          lineItemId,
          quantity,
          tenantSlug,
        );

        if (updated) {
          setCart(updated);
          return true;
        }
        return false;
      } catch (error) {
        console.error("Failed to update cart line item:", error);
        return false;
      } finally {
        setIsUpdating(false);
      }
    },
    [cart?.id, tenantSlug],
  );

  const removeItem = useCallback(
    async (lineItemId: string): Promise<boolean> => {
      if (!cart?.id) return false;
      setIsUpdating(true);
      try {
        const updated = await removeStorefrontCartLineItem(
          storefrontGraphql,
          cart.id,
          lineItemId,
          tenantSlug,
        );

        if (updated) {
          setCart(updated);
          return true;
        }
        return false;
      } catch (error) {
        console.error("Failed to remove cart line item:", error);
        return false;
      } finally {
        setIsUpdating(false);
      }
    },
    [cart?.id, tenantSlug],
  );

  const selectShippingOption = useCallback(
    async (shippingOptionId: string): Promise<boolean> => {
      if (!cart?.id) return false;
      setIsUpdating(true);
      try {
        const updated = await updateStorefrontCartShipping(
          storefrontGraphql,
          cart.id,
          shippingOptionId,
          undefined,
          tenantSlug,
        );
        if (updated) {
          setCart(updated);
          return true;
        }
        return false;
      } catch (err) {
        console.error("Failed to update shipping option:", err);
        return false;
      } finally {
        setIsUpdating(false);
      }
    },
    [cart?.id, tenantSlug],
  );

  const clearCart = useCallback(() => {
    try {
      window.localStorage.removeItem(CART_STORAGE_KEY);
    } catch {
      // ignore
    }
    setCart(null);
  }, []);

  const itemCount = useMemo(() => {
    if (!cart?.lineItems) return 0;
    return cart.lineItems.reduce((acc, item) => acc + item.quantity, 0);
  }, [cart?.lineItems]);

  const value = useMemo<CartContextValue>(
    () => ({
      cart,
      isOpen,
      isLoading,
      isUpdating,
      itemCount,
      openCart,
      closeCart,
      toggleCart,
      addItem,
      updateQuantity,
      removeItem,
      refreshCart,
      selectShippingOption,
      clearCart,
    }),
    [
      cart,
      isOpen,
      isLoading,
      isUpdating,
      itemCount,
      openCart,
      closeCart,
      toggleCart,
      addItem,
      updateQuantity,
      removeItem,
      refreshCart,
      selectShippingOption,
      clearCart,
    ],
  );

  return <CartContext.Provider value={value}>{children}</CartContext.Provider>;
}

export function useCart(): CartContextValue {
  const context = useContext(CartContext);
  if (!context) {
    throw new Error("useCart must be used within a CartProvider");
  }
  return context;
}
