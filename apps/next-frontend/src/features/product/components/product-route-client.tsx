/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
"use client";

import {
  ProductDetailView,
  type ProductDetailViewProps,
} from "@rustok/product-frontend";
import { useStorefrontAuth } from "@/shared/lib/auth-session";
import { storefrontGraphql } from "@/shared/lib/graphql";

/** Keep the executor on the client side of the RSC boundary. */
export function ProductRouteClient(
  props: Omit<ProductDetailViewProps, "graphql" | "token">,
) {
  const auth = useStorefrontAuth();
  return (
    <ProductDetailView
      {...props}
      token={auth.token}
      graphql={storefrontGraphql}
    />
  );
}
