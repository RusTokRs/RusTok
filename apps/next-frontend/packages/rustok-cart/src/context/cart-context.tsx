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
  useInsertionEffect,
  useMemo,
  useRef,
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
import type { Cart, CartPricingExpectation } from "../api/types";
import {
  UNSCOPED_CART_STORAGE_KEY,
  cartMatchesPricingExpectation,
  cartStorageKey,
  adoptBuyerCart,
  validCartBuyerSelection,
  cartMutationMarkerKey,
  cartReferenceFromSearch,
} from "../core";

export type CartActionError =
  | "auth"
  | "restore"
  | "context"
  | "invalid"
  | "unconfirmed";
export interface CartContextValue {
  /** Whether the host enabled the Cart module for this tenant. */
  enabled: boolean;
  cart: Cart | null;
  reference: string | null;
  isOpen: boolean;
  isLoading: boolean;
  isUpdating: boolean;
  itemCount: number;
  actionError: CartActionError | null;
  openCart: () => void;
  closeCart: () => void;
  toggleCart: () => void;
  addItem: (
    variantId: string,
    quantity: number,
    context: CartPricingExpectation,
  ) => Promise<boolean>;
  updateQuantity: (lineItemId: string, quantity: number) => Promise<boolean>;
  removeItem: (lineItemId: string) => Promise<boolean>;
  refreshCart: () => Promise<void>;
  selectShippingOption: (shippingOptionId: string) => Promise<boolean>;
  clearCart: () => void;
}

const CartContext = createContext<CartContextValue | undefined>(undefined);
export interface CartProviderProps {
  children: ReactNode;
  locale: string;
  tenantSlug?: string | null;
  authToken?: string | null;
  authReady: boolean;
  authRefused: boolean;
  authScopeIsCurrent: () => boolean;
  enabled?: boolean;
}

// Availability and capability transitions create new scopes even if the same actor returns.
// An old operation must not continue after an auth or capability A → B → A cycle.
type Scope = {
  tenant: string | null;
  token: string | null;
  locale: string;
  enabled: boolean;
  authReady: boolean;
  authRefused: boolean;
};
type Session = {
  scope: Scope;
  cart: Cart | null;
  storageKey: string | null;
  storedId: string | null;
  loading: boolean;
};

// Coordinate known in-flight mutations across CartProvider remounts in this JavaScript realm.
// Storage markers survive reloads, but are not locks or server receipts; they are handled
// separately and never make a cross-tab or post-reload outcome certain.
type MutationBarrier = {
  revision: object;
  pending: Promise<void> | null;
  observers: number;
};
const mutationBarriers = new Map<string, MutationBarrier>();

function mutationBarrier(key: string): MutationBarrier {
  let barrier = mutationBarriers.get(key);
  if (!barrier) {
    barrier = { revision: {}, pending: null, observers: 0 };
    mutationBarriers.set(key, barrier);
  }
  return barrier;
}

function pruneMutationBarrier(key: string, barrier: MutationBarrier) {
  if (!barrier.pending && barrier.observers === 0 && mutationBarriers.get(key) === barrier) {
    mutationBarriers.delete(key);
  }
}

function observeMutations(key: string) {
  const barrier = mutationBarrier(key);
  barrier.observers += 1;
  let released = false;
  return {
    barrier,
    release() {
      if (released) return;
      released = true;
      barrier.observers -= 1;
      pruneMutationBarrier(key, barrier);
    },
  };
}

function beginMutation(key: string): (() => void) | null {
  const barrier = mutationBarrier(key);
  if (barrier.pending) return null;
  let settle: () => void = () => {};
  const pending = new Promise<void>((resolve) => { settle = resolve; });
  barrier.revision = {};
  barrier.pending = pending;
  return () => {
    if (barrier.pending !== pending) return;
    barrier.pending = null;
    settle();
    pruneMutationBarrier(key, barrier);
  };
}

export function CartProvider({
  children,
  locale,
  tenantSlug = null,
  authToken = null,
  authReady,
  authRefused,
  authScopeIsCurrent,
  enabled = true,
}: CartProviderProps) {
  const scope = useMemo<Scope>(
    () => ({ tenant: tenantSlug, token: authToken, locale, enabled, authReady, authRefused }),
    [tenantSlug, authToken, locale, enabled, authReady, authRefused],
  );
  const [session, setSession] = useState<Session>({
    scope,
    cart: null,
    storageKey: null,
    storedId: null,
    loading: enabled,
  });
  const [error, setErrorState] = useState<{
    scope: Scope;
    code: CartActionError;
  } | null>(null);
  const [isOpen, setIsOpen] = useState(false);
  const [operation, setOperation] = useState<{ scope: Scope } | null>(null);
  const inFlight = useRef<{ scope: Scope } | null>(null);
  const liveScope = useRef<Scope | null>(null);
  const liveAuthCheck = useRef(authScopeIsCurrent);
  // Commit the fence before any descendant layout effect can invoke a captured callback. A
  // render-time ref write would incorrectly invalidate the current scope on an aborted render;
  // a parent layout effect runs too late because descendant layout effects run first.
  useInsertionEffect(() => {
    liveScope.current = scope;
    liveAuthCheck.current = authScopeIsCurrent;
    return () => {
      if (liveScope.current === scope) liveScope.current = null;
    };
  }, [scope, authScopeIsCurrent]);
  const scopeCurrent = useCallback(
    () =>
      scope.enabled &&
      scope.authReady &&
      !scope.authRefused &&
      liveScope.current === scope &&
      liveAuthCheck.current(),
    [scope],
  );
  const blockedScope = useRef<Scope | null>(null);
  const adoptedInitialUrl = useRef(false);
  const setError = useCallback(
    (value: { scope: Scope; code: CartActionError } | null) => {
      if (!scopeCurrent()) return;
      blockedScope.current =
        value?.code === "unconfirmed" || value?.code === "restore"
          ? scope
          : null;
      setErrorState(value);
    },
    [scope, scopeCurrent],
  );
  const current = session.scope === scope ? session : null;
  // A restore must not clear an in-flight mutation's marker using a read made before that
  // mutation settles. The storage key is shared across locales and capability generations.
  const waitingForPreviousOperation = operation !== null && operation.scope !== scope &&
    (operation.scope.tenant ?? "host") === (scope.tenant ?? "host") &&
    operation.scope.token === scope.token;
  const cart = enabled && !waitingForPreviousOperation ? (current?.cart ?? null) : null;
  const actionError = !enabled
    ? null
    : authRefused
      ? "auth"
      : error?.scope === scope
        ? error.code
        : null;
  const isLoading = enabled
    ? (!authReady || authRefused || waitingForPreviousOperation || (current?.loading ?? true))
    : false;
  const isUpdating = enabled && operation !== null;

  useEffect(() => {
    if (!enabled || !authReady || waitingForPreviousOperation || !scopeCurrent()) return;
    let cancelled = false;
    async function restore() {
      let key: string | null = null;
      let storedId: string | null = null;
      let uncertain = false;
      let observation: ReturnType<typeof observeMutations> | null = null;
      try {
        key = await cartStorageKey(scope.tenant, scope.token);
        if (cancelled || !scopeCurrent()) return;
        observation = observeMutations(key);
        while (observation.barrier.pending) {
          await observation.barrier.pending;
          if (cancelled || !scopeCurrent()) return;
        }
        const revision = observation.barrier.revision;
        const explicit = adoptedInitialUrl.current
          ? null
          : cartReferenceFromSearch(window.location.search);
        adoptedInitialUrl.current = true;
        const scoped = window.localStorage.getItem(key);
        // Only the initial explicit URL requests adoption. An auth switch never adopts the previous
        // actor's URL. Scoped empty entries explicitly suppress the old global storage reference.
        storedId =
          explicit ??
          (scoped === null
            ? window.localStorage.getItem(UNSCOPED_CART_STORAGE_KEY)
            : scoped || null);
        const markerKey = cartMutationMarkerKey(key);
        const marker = window.localStorage.getItem(markerKey);
        if (marker !== null) {
          // A remount, reload or another tab can leave a transmitted mutation running. One
          // automatic owner read cannot prove it has settled, even when an ID is available.
          uncertain = true;
          throw new Error("An earlier Cart mutation is unconfirmed");
        }
        const restored = storedId
          ? await fetchStorefrontCart(
              storefrontGraphql,
              storedId,
              scope.tenant,
              scope.token,
            )
          : null;
        if (cancelled || !scopeCurrent()) return;
        const active = storedId ? adoptBuyerCart(restored, storedId) : null;
        if (observation.barrier.revision !== revision || window.localStorage.getItem(markerKey) !== marker) {
          uncertain = true;
          throw new Error("Cart changed during restore");
        }
        // A successful, uncontested owner read validates the reference in this scope.
        window.localStorage.setItem(key, active?.id ?? "");
        setSession({
          scope,
          cart: active,
          storageKey: key,
          storedId: active?.id ?? null,
          loading: false,
        });
        setError(null);
      } catch {
        if (cancelled || !scopeCurrent()) return;
        // Preserve the reference on failure. A failed read does not prove a missing/empty cart.
        setSession({
          scope,
          cart: null,
          storageKey: key,
          storedId,
          loading: false,
        });
        setError({ scope, code: uncertain ? "unconfirmed" : "restore" });
      } finally {
        observation?.release();
      }
    }
    void restore();
    return () => {
      cancelled = true;
    };
  }, [scope, enabled, authReady, waitingForPreviousOperation, scopeCurrent, setError]);

  // A control captured by an old landing must not affect the current Cart UI. Opening during
  // an auth/restore wait remains allowed so the drawer can show its explicit pending state.
  const openCart = useCallback(() => {
    if (scope.enabled && liveScope.current === scope) setIsOpen(true);
  }, [scope]);
  const closeCart = useCallback(() => {
    if (liveScope.current === scope) setIsOpen(false);
  }, [scope]);
  const toggleCart = useCallback(() => {
    if (scope.enabled && liveScope.current === scope) setIsOpen((value) => !value);
  }, [scope]);

  const commit = useCallback(
    (value: Cart) => {
      if (!scopeCurrent()) return;
      setSession((previous) =>
        previous.scope === scope
          ? { ...previous, cart: value, storedId: value.id }
          : previous,
      );
    },
    [scope, scopeCurrent],
  );

  const refreshCart = useCallback(async () => {
    if (!enabled) return;
    const id = current?.cart?.id ?? current?.storedId;
    if (!scopeCurrent() || !id || inFlight.current) return;
    const op = { scope };
    inFlight.current = op;
    setOperation(op);
    let observation: ReturnType<typeof observeMutations> | null = null;
    let uncertain = false;
    try {
      if (!current?.storageKey)
        throw new Error("Cart reference storage is unavailable");
      observation = observeMutations(current.storageKey);
      while (observation.barrier.pending) {
        await observation.barrier.pending;
        if (!scopeCurrent()) return;
      }
      const revision = observation.barrier.revision;
      const markerKey = cartMutationMarkerKey(current.storageKey);
      const marker = window.localStorage.getItem(markerKey);
      const refreshed = await fetchStorefrontCart(
        storefrontGraphql,
        id,
        scope.tenant,
        scope.token,
      );
      if (!scopeCurrent()) return;
      const active = adoptBuyerCart(refreshed, id);
      if (observation.barrier.revision !== revision || window.localStorage.getItem(markerKey) !== marker) {
        uncertain = true;
        throw new Error("Cart changed during inspection");
      }
      window.localStorage.setItem(current.storageKey, active?.id ?? "");
      window.localStorage.removeItem(markerKey);
      setSession({
        ...current,
        cart: active,
        storedId: active?.id ?? null,
        loading: false,
      });
      setError(null);
    } catch {
      if (scopeCurrent()) setError({ scope, code: uncertain ? "unconfirmed" : "restore" });
    } finally {
      observation?.release();
      if (inFlight.current === op) {
        inFlight.current = null;
        setOperation(null);
      }
    }
  }, [current, scope, enabled, scopeCurrent, setError]);

  const addItem = useCallback(
    async (
      variantId: string,
      quantity: number,
      expectation: CartPricingExpectation,
    ): Promise<boolean> => {
      if (
        !enabled ||
        !scopeCurrent() ||
        inFlight.current ||
        blockedScope.current === scope ||
        !current ||
        current.loading
      )
        return false;
      if (!validCartBuyerSelection(variantId, expectation, quantity)) {
        setError({ scope, code: "invalid" });
        return false;
      }
      if (
        current.cart &&
        !cartMatchesPricingExpectation(current.cart, expectation)
      ) {
        setError({ scope, code: "context" });
        return false;
      }
      const op = { scope };
      inFlight.current = op;
      setOperation(op);
      setError(null);
      let transmitted = false;
      let finishMutation: (() => void) | null = null;
      try {
        if (!current.storageKey) {
          setError({ scope, code: "restore" });
          return false;
        }
        const key = cartMutationMarkerKey(current.storageKey);
        if (window.localStorage.getItem(key) !== null) {
          setError({ scope, code: "unconfirmed" });
          return false;
        }
        finishMutation = beginMutation(current.storageKey);
        if (!finishMutation) {
          setError({ scope, code: "unconfirmed" });
          return false;
        }
        const marker = { key, id: globalThis.crypto.randomUUID() };
        window.localStorage.setItem(key, marker.id);
        let active = current.cart;
        if (!active) {
          transmitted = true;
          active = await createStorefrontCart(
            storefrontGraphql,
            {
              currencyCode: expectation.currencyCode,
              regionId: expectation.regionId,
              locale: expectation.locale,
            },
            scope.tenant,
            scope.token,
          );
          if (!active) {
            if (scopeCurrent()) setError({ scope, code: "unconfirmed" });
            return false;
          }
          adoptBuyerCart(active, active.id);
          // Retain a confirmed response under its ORIGINAL actor's key even if auth changed during
          // create. Never publish it or continue the add in the new actor. Backend restore still
          // proves access; this is only a local reference, not a transferable guest capability.
          if (scopeCurrent()) commit(active);
          if (!current.storageKey) {
            if (scopeCurrent()) setError({ scope, code: "unconfirmed" });
            return false;
          }
          try {
            window.localStorage.setItem(current.storageKey, active.id);
          } catch {
            if (scopeCurrent()) setError({ scope, code: "unconfirmed" });
            return false;
          }
          if (!scopeCurrent()) {
            // Creation returned a validated reference and no add was transmitted. This operation
            // is settled; leave a marker only when that fact or the reference is uncertain.
            if (window.localStorage.getItem(marker.key) === marker.id)
              window.localStorage.removeItem(marker.key);
            return false;
          }
        }
        if (!cartMatchesPricingExpectation(active, expectation)) {
          if (window.localStorage.getItem(marker.key) === marker.id)
            window.localStorage.removeItem(marker.key);
          setError({ scope, code: "context" });
          return false;
        }
        transmitted = true;
        const updated = await addStorefrontCartLineItem(
          storefrontGraphql,
          active.id,
          variantId,
          quantity,
          scope.tenant,
          expectation,
          scope.token,
        );
        if (!updated) {
          if (scopeCurrent()) setError({ scope, code: "unconfirmed" });
          return false;
        }
        adoptBuyerCart(updated, active.id);
        if (window.localStorage.getItem(marker.key) === marker.id)
          window.localStorage.removeItem(marker.key);
        if (!scopeCurrent()) return false;
        commit(updated);
        return true;
      } catch {
        if (scopeCurrent())
          setError({ scope, code: transmitted ? "unconfirmed" : "restore" });
        return false;
      } finally {
        finishMutation?.();
        if (inFlight.current === op) {
          inFlight.current = null;
          setOperation(null);
        }
      }
    },
    [current, scope, enabled, commit, scopeCurrent, setError],
  );

  const mutate = useCallback(
    async (run: (active: Cart) => Promise<Cart | null>): Promise<boolean> => {
      if (
        !enabled ||
        !scopeCurrent() ||
        !cart ||
        inFlight.current ||
        blockedScope.current === scope ||
        isLoading
      )
        return false;
      const op = { scope };
      inFlight.current = op;
      setOperation(op);
      setError(null);
      let transmitted = false;
      let finishMutation: (() => void) | null = null;
      try {
        if (!current?.storageKey)
          throw new Error("Cart reference storage is unavailable");
        const key = cartMutationMarkerKey(current.storageKey);
        if (window.localStorage.getItem(key) !== null) {
          setError({ scope, code: "unconfirmed" });
          return false;
        }
        finishMutation = beginMutation(current.storageKey);
        if (!finishMutation) {
          setError({ scope, code: "unconfirmed" });
          return false;
        }
        const marker = { key, id: globalThis.crypto.randomUUID() };
        window.localStorage.setItem(key, marker.id);
        transmitted = true;
        const updated = await run(cart);
        if (!updated) {
          if (scopeCurrent()) setError({ scope, code: "unconfirmed" });
          return false;
        }
        adoptBuyerCart(updated, cart.id);
        if (window.localStorage.getItem(marker.key) === marker.id)
          window.localStorage.removeItem(marker.key);
        if (!scopeCurrent()) return false;
        commit(updated);
        return true;
      } catch {
        if (scopeCurrent())
          setError({ scope, code: transmitted ? "unconfirmed" : "restore" });
        return false;
      } finally {
        finishMutation?.();
        if (inFlight.current === op) {
          inFlight.current = null;
          setOperation(null);
        }
      }
    },
    [cart, current, isLoading, scope, enabled, commit, scopeCurrent, setError],
  );

  const updateQuantity = useCallback(
    (lineItemId: string, quantity: number) =>
      mutate((active) =>
        quantity <= 0
          ? removeStorefrontCartLineItem(
              storefrontGraphql,
              active.id,
              lineItemId,
              scope.tenant,
              scope.token,
            )
          : updateStorefrontCartLineItemQuantity(
              storefrontGraphql,
              active.id,
              lineItemId,
              quantity,
              scope.tenant,
              scope.token,
            ),
      ),
    [mutate, scope],
  );
  const removeItem = useCallback(
    (lineItemId: string) =>
      mutate((active) =>
        removeStorefrontCartLineItem(
          storefrontGraphql,
          active.id,
          lineItemId,
          scope.tenant,
          scope.token,
        ),
      ),
    [mutate, scope],
  );
  const selectShippingOption = useCallback(
    (shippingOptionId: string) =>
      mutate((active) =>
        updateStorefrontCartShipping(
          storefrontGraphql,
          active.id,
          shippingOptionId,
          undefined,
          scope.tenant,
          scope.token,
        ),
      ),
    [mutate, scope],
  );
  const clearCart = useCallback(() => {
    if (!enabled || !scopeCurrent() || inFlight.current || !current?.storageKey) return;
    try {
      // Explicit user action drops only this session's local reference, never persisted cart data.
      window.localStorage.setItem(current.storageKey, "");
      window.localStorage.removeItem(cartMutationMarkerKey(current.storageKey));
      setSession({ ...current, cart: null, storedId: null, loading: false });
      setError(null);
    } catch {
      setError({ scope, code: "restore" });
    }
  }, [current, scope, enabled, scopeCurrent, setError]);

  const itemCount = useMemo(
    () => cart?.lineItems.reduce((sum, line) => sum + line.quantity, 0) ?? 0,
    [cart],
  );
  const reference = current?.cart?.id ?? current?.storedId ?? null;
  const value = useMemo<CartContextValue>(
    () => ({
      enabled,
      cart,
      reference,
      isOpen,
      isLoading,
      isUpdating,
      itemCount,
      actionError,
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
      enabled,
      cart,
      reference,
      isOpen,
      isLoading,
      isUpdating,
      itemCount,
      actionError,
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
  if (!context) throw new Error("useCart must be used within a CartProvider");
  return context;
}
