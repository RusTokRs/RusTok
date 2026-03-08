'use client';

import { useEnabledModulesContext } from '@/shared/lib/enabled-modules-context';

export function useEnabledModules(): string[] {
  return useEnabledModulesContext().enabledModules;
}

export function useIsModuleEnabled(slug: string): boolean {
  return useEnabledModules().includes(slug);
}

export function useEnabledModulesActions() {
  const { replaceEnabledModules, setModuleEnabled } = useEnabledModulesContext();

  return {
    replaceEnabledModules,
    setModuleEnabled
  };
}
