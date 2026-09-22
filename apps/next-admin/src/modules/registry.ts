import type { AdminModule } from './types';
import type { NavItem } from '@/types';

const registry = new Map<string, AdminModule>();

function attachModuleSlug(items: NavItem[], moduleSlug: string): NavItem[] {
  return items.map((item) => ({
    ...item,
    moduleSlug: item.moduleSlug ?? moduleSlug,
    items: item.items ? attachModuleSlug(item.items, moduleSlug) : item.items
  }));
}

export function registerAdminModule(module: AdminModule) {
  registry.set(module.id, {
    ...module,
    navItems: attachModuleSlug(module.navItems, module.id)
  });
}

export function getAdminModules(): AdminModule[] {
  return Array.from(registry.values());
}

export function getAdminNavItems(): NavItem[] {
  return Array.from(registry.values()).flatMap((m) => m.navItems);
}
