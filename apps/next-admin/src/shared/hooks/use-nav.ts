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

export function useFilteredNavItems(items: NavItem[]) {
  const { data: session } = useSession();
  const role = session?.user?.role;
  const enabledModules = useEnabledModules();

  return useMemo(() => {
    return items
      .filter((item) => {
        if (item.moduleSlug && !enabledModules.includes(item.moduleSlug)) {
          return false;
        }
        if (!item.access) return true;
        if (item.access.requireOrg) return false;
        if (item.access.role && !hasMinRole(role, item.access.role))
          return false;
        return true;
      })
      .map((item) => ({
        ...item,
        items:
          item.items?.filter((child) => {
            if (
              child.moduleSlug &&
              !enabledModules.includes(child.moduleSlug)
            ) {
              return false;
            }
            if (!child.access) return true;
            if (child.access.requireOrg) return false;
            if (child.access.role && !hasMinRole(role, child.access.role))
              return false;
            return true;
          }) ?? []
      }));
  }, [enabledModules, items, role]);
}
