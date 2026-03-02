import type { AdminModule } from './types';
import type { NavItem } from '@/types';

const registry = new Map<string, AdminModule>();

export function registerAdminModule(module: AdminModule) {
  registry.set(module.id, module);
}

export function getAdminModules(): AdminModule[] {
  return Array.from(registry.values());
}

export function getAdminNavItems(): NavItem[] {
  return Array.from(registry.values()).flatMap((m) => m.navItems);
}
