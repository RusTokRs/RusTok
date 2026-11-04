import type {
  AdminComponentRegistration,
  AdminModule,
  AdminSlot
} from './types';
import type { NavItem } from '@/types';

const moduleRegistry = new Map<string, AdminModule>();
const componentRegistry = new Map<string, AdminComponentRegistration>();

function attachModuleSlug(items: NavItem[], moduleSlug: string): NavItem[] {
  return items.map((item) => ({
    ...item,
    moduleSlug: item.moduleSlug ?? moduleSlug,
    items: item.items ? attachModuleSlug(item.items, moduleSlug) : item.items
  }));
}

export function registerAdminComponent(component: AdminComponentRegistration) {
  componentRegistry.set(component.id, component);
}

export function registerAdminModule(module: AdminModule) {
  moduleRegistry.set(module.id, {
    ...module,
    navItems: attachModuleSlug(module.navItems, module.id)
  });

  if (module.components) {
    for (const comp of module.components) {
      registerAdminComponent({
        ...comp,
        moduleSlug: comp.moduleSlug ?? module.id
      });
    }
  }
}

export function getAdminModules(): AdminModule[] {
  return Array.from(moduleRegistry.values());
}

export function getAdminNavItems(): NavItem[] {
  return Array.from(moduleRegistry.values()).flatMap((m) => m.navItems);
}

export function getAdminComponentsForSlot(
  slot: AdminSlot,
  enabledModules?: Iterable<string>
): AdminComponentRegistration[] {
  const enabled = enabledModules === undefined ? null : new Set(enabledModules);

  return Array.from(componentRegistry.values())
    .filter((comp) => {
      if (comp.slot !== slot) return false;
      if (!comp.moduleSlug) return true;
      if (enabled === null) return false; // fail-closed if module availability unknown
      return enabled.has(comp.moduleSlug);
    })
    .sort((a, b) => (a.order ?? 50) - (b.order ?? 50));
}
