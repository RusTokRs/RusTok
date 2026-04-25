'use client';

import React from 'react';

type EnabledModulesContextValue = {
  enabledModules: string[];
  setModuleEnabled: (slug: string, enabled: boolean) => void;
  replaceEnabledModules: (modules: string[]) => void;
};

const EnabledModulesContext =
  React.createContext<EnabledModulesContextValue | null>(null);

function normalizeModules(modules: string[]): string[] {
  return Array.from(new Set(modules)).sort();
}

function modulesEqual(left: string[], right: string[]): boolean {
  return (
    left.length === right.length &&
    left.every((module, index) => module === right[index])
  );
}

export function EnabledModulesClientProvider({
  initialModules,
  children
}: {
  initialModules: string[];
  children: React.ReactNode;
}) {
  const normalizedInitialModules = React.useMemo(
    () => normalizeModules(initialModules),
    [initialModules]
  );
  const [enabledModules, setEnabledModules] = React.useState(
    normalizedInitialModules
  );

  React.useEffect(() => {
    setEnabledModules((prev) =>
      modulesEqual(prev, normalizedInitialModules)
        ? prev
        : normalizedInitialModules
    );
  }, [normalizedInitialModules]);

  const value = React.useMemo<EnabledModulesContextValue>(
    () => ({
      enabledModules,
      setModuleEnabled: (slug, enabled) => {
        setEnabledModules((prev) => {
          const next = new Set(prev);
          if (enabled) {
            next.add(slug);
          } else {
            next.delete(slug);
          }
          return normalizeModules(Array.from(next));
        });
      },
      replaceEnabledModules: (modules) => {
        setEnabledModules(normalizeModules(modules));
      }
    }),
    [enabledModules]
  );

  return (
    <EnabledModulesContext.Provider value={value}>
      {children}
    </EnabledModulesContext.Provider>
  );
}

export function useEnabledModulesContext() {
  const context = React.useContext(EnabledModulesContext);

  if (!context) {
    throw new Error(
      'useEnabledModulesContext must be used within an EnabledModulesClientProvider.'
    );
  }

  return context;
}
