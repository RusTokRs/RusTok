/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
"use client";

import type { ReactNode } from "react";
import { CartDrawer, CartProvider, CartTrigger } from "@rustok/cart-frontend";
import {
  StorefrontAuthProvider,
  useStorefrontAuth,
} from "@/shared/lib/auth-session";
import { useEnabledModulesContext } from "@/shared/lib/enabled-modules-context";

function CartSession({
  children,
  locale,
  tenantSlug,
}: {
  children: ReactNode;
  locale: string;
  tenantSlug: string | null;
}) {
  const auth = useStorefrontAuth();
  const { enabledModules } = useEnabledModulesContext();
  const isCartEnabled = enabledModules.includes("cart");
  return (
    <CartProvider
      locale={locale}
      tenantSlug={tenantSlug}
      authToken={auth.token}
      authReady={auth.ready}
      authRefused={auth.refused}
      authScopeIsCurrent={auth.isCurrent}
      enabled={isCartEnabled}
    >
      {children}
      {isCartEnabled && (
        <>
          <CartTrigger locale={locale} />
          <CartDrawer locale={locale} />
        </>
      )}
    </CartProvider>
  );
}

export function StorefrontProviders(props: {
  children: ReactNode;
  locale: string;
  tenantSlug: string | null;
}) {
  return (
    <StorefrontAuthProvider tenantSlug={props.tenantSlug}>
      <CartSession {...props} />
    </StorefrontAuthProvider>
  );
}
