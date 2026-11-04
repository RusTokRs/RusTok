import type { ReactNode } from "react";
import type { storefrontGraphql } from "@/shared/lib/graphql";

export type StorefrontSlot =
  | "header:navigation"
  | "header:actions"
  | "home:afterHero"
  | "home:afterCatalog"
  | "home:beforeFooter"
  | "footer:navigation"
  | "checkout:paymentHandoff"
  | "checkout:resultHandoff"
  | "checkout:shippingHandoff";

export function normalizeStorefrontSlot(slot: string): StorefrontSlot | string {
  switch (slot) {
    case "home_after_hero":
      return "home:afterHero";
    case "home_after_catalog":
      return "home:afterCatalog";
    case "home_before_footer":
      return "home:beforeFooter";
    case "header_navigation":
      return "header:navigation";
    case "header_actions":
      return "header:actions";
    case "footer_navigation":
      return "footer:navigation";
    case "checkout_payment_handoff":
      return "checkout:paymentHandoff";
    case "checkout_result_handoff":
      return "checkout:resultHandoff";
    case "checkout_shipping_handoff":
      return "checkout:shippingHandoff";
    default:
      return slot;
  }
}

export type StorefrontRenderContext = {
  locale: string;
  searchParams: Record<string, string | string[] | undefined>;
  enabledModules: string[];
  tenantSlug: string | null;
  tenantId: string | null;
  graphql: typeof storefrontGraphql;
};

export type StorefrontModule = {
  id: string;
  moduleSlug?: string;
  slot: StorefrontSlot | string;
  order?: number;
  render: (context: StorefrontRenderContext) => ReactNode;
};

const registry = new Map<string, StorefrontModule>();

export function registerStorefrontModule(module: StorefrontModule) {
  registry.set(module.id, {
    ...module,
    slot: normalizeStorefrontSlot(module.slot) as StorefrontSlot,
  });
}

export function getModulesForSlot(
  slot: StorefrontSlot | string,
  enabledModules?: Iterable<string>,
) {
  const normalizedTarget = normalizeStorefrontSlot(slot);
  const enabled = enabledModules === undefined ? null : new Set(enabledModules);

  return Array.from(registry.values())
    .filter((module) => {
      if (normalizeStorefrontSlot(module.slot) !== normalizedTarget) return false;
      if (!module.moduleSlug) return true;
      if (enabled === null) return false;
      return enabled.has(module.moduleSlug);
    })
    .sort((left, right) => {
      const leftOrder = left.order ?? 0;
      const rightOrder = right.order ?? 0;
      if (leftOrder !== rightOrder) return leftOrder - rightOrder;
      return left.id.localeCompare(right.id);
    });
}
