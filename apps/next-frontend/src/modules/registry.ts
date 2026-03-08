import type { ReactNode } from "react";

export type StorefrontSlot = "home:afterHero";

export type StorefrontModule = {
  id: string;
  moduleSlug?: string;
  slot: StorefrontSlot;
  order?: number;
  render: () => ReactNode;
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
      if (module.slot !== slot) {
        return false;
      }

      if (!module.moduleSlug || enabled === null) {
        return true;
      }

      return enabled.has(module.moduleSlug);
    })
    .sort((left, right) => {
      const leftOrder = left.order ?? 0;
      const rightOrder = right.order ?? 0;
      if (leftOrder !== rightOrder) {
        return leftOrder - rightOrder;
      }
      return left.id.localeCompare(right.id);
    });
}
