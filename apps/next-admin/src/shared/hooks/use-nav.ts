'use client';
import { useMemo } from 'react';
import { useSession } from 'next-auth/react';
import type { NavItem } from '@/types';
import { useEnabledModules } from './use-enabled-modules';

const ROLE_HIERARCHY: Record<string, number> = {
  customer: 0,
  manager: 1,
  admin: 2,
  super_admin: 3
};

function hasMinRole(userRole: string | undefined, minRole: string): boolean {
  if (!userRole) return false;
  const userLevel = ROLE_HIERARCHY[userRole.toLowerCase()] ?? -1;
  const minLevel = ROLE_HIERARCHY[minRole.toLowerCase()] ?? 999;
  return userLevel >= minLevel;
}

function canAccessItem(
  item: NavItem,
  role: string | undefined,
  enabledModules: string[]
): boolean {
  if (item.moduleSlug && !enabledModules.includes(item.moduleSlug)) {
    return false;
  }

  if (!item.access) return true;
  if (item.access.requireOrg) return false;
  if (item.access.role && !hasMinRole(role, item.access.role)) return false;

  return true;
}

function filterNavItem(
  item: NavItem,
  role: string | undefined,
  enabledModules: string[]
): NavItem | null {
  if (!canAccessItem(item, role, enabledModules)) {
    return null;
  }

  const filteredChildren =
    item.items
      ?.map((child) => filterNavItem(child, role, enabledModules))
      .filter((child): child is NavItem => Boolean(child)) ?? [];

  if (item.items?.length && filteredChildren.length === 0 && item.url === '#') {
    return null;
  }

  return {
    ...item,
    items: filteredChildren
  };
}

export function useFilteredNavItems(items: NavItem[]) {
  const { data: session } = useSession();
  const role = session?.user?.role;
  const enabledModules = useEnabledModules();

  return useMemo(() => {
    return items
      .map((item) => filterNavItem(item, role, enabledModules))
      .filter((item): item is NavItem => Boolean(item));
  }, [enabledModules, items, role]);
}
