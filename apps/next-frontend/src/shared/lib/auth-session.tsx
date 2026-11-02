/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
"use client";

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { getClientAuth, type AuthSession } from "@rustok/ui-auth/browser";

const PENDING = "pending";
const UNAVAILABLE = "unavailable";

// This host adapts the canonical browser-session reader; it does not issue credentials,
// persist tokens, decode JWT claims, resolve a tenant or create another auth protocol.
function readSnapshot(): string {
  try {
    return JSON.stringify(getClientAuth());
  } catch {
    return UNAVAILABLE;
  }
}
function subscribe(onChange: () => void) {
  const cookieStore = (window as Window & { cookieStore?: EventTarget })
    .cookieStore;
  cookieStore?.addEventListener("change", onChange);
  window.addEventListener("focus", onChange);
  window.addEventListener("storage", onChange);
  document.addEventListener("visibilitychange", onChange);
  // Cookie Store is not universal and older login consumers do not emit an event.
  // This is observation only. Mutations also perform a synchronous fresh-snapshot fence.
  const timer = window.setInterval(onChange, 1000);
  return () => {
    cookieStore?.removeEventListener("change", onChange);
    window.removeEventListener("focus", onChange);
    window.removeEventListener("storage", onChange);
    document.removeEventListener("visibilitychange", onChange);
    window.clearInterval(timer);
  };
}

type StorefrontAuth = {
  token: string | null;
  ready: boolean;
  refused: boolean;
  isCurrent: () => boolean;
};
const AuthContext = createContext<StorefrontAuth | null>(null);

export function StorefrontAuthProvider({
  tenantSlug,
  children,
}: {
  tenantSlug: string;
  children: ReactNode;
}) {
  const snapshot = useSyncExternalStore(subscribe, readSnapshot, () => PENDING);
  const session = useMemo<AuthSession | null>(
    () =>
      snapshot === PENDING || snapshot === UNAVAILABLE
        ? null
        : JSON.parse(snapshot),
    [snapshot],
  );
  // A browser-selected foreign tenant never replaces the host's resolved storefront tenant.
  const refused =
    snapshot === UNAVAILABLE ||
    !!(session?.tenant && session.tenant !== tenantSlug);
  const ready = session !== null && !refused;
  const isCurrent = useCallback(
    () => ready && readSnapshot() === snapshot,
    [ready, snapshot],
  );
  const value = useMemo(
    () => ({
      token: ready ? session?.token || null : null,
      ready,
      refused,
      isCurrent,
    }),
    [ready, refused, session, isCurrent],
  );
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useStorefrontAuth(): StorefrontAuth {
  const auth = useContext(AuthContext);
  if (!auth) throw new Error("StorefrontAuthProvider is required");
  return auth;
}
