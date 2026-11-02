/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

"use client";

import { PricingView, type PricingViewProps } from "@rustok/pricing-frontend";
import { storefrontGraphql } from "@/shared/lib/graphql";

/** Wire the executor on the client: ordinary functions cannot cross the RSC props boundary. */
export function PricingRouteClient(props: Omit<PricingViewProps, "graphql">) {
  return <PricingView {...props} graphql={storefrontGraphql} />;
}
