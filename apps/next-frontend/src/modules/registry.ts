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
  slot: StorefrontSlot;
  order?: number;
  render: (context: StorefrontRenderContext) => ReactNode;
};

const registry = new Map<string, StorefrontModule>();

export function registerStorefrontModule(module: StorefrontModule) {
  registry.set(module.id, module);
}

export function getModulesForSlot(
  slot: StorefrontSlot,
  enabledModules?: Iterable<string>,
) {
  const enabled = enabledModules === undefined ? null : new Set(enabledModules);

  return Array.from(registry.values())
    .filter((module) => {
      if (module.slot !== slot) return false;
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
