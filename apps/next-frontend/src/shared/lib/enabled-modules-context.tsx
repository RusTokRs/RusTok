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
  // A new server prop remains authoritative even if its values happen to match the old
  // prop: local overrides must not survive a refresh. Compare the prop, not the memoized
  // normalization (whose cache React may discard). Reset during render because an effect
  // would commit the old capability once after new props arrive.
  const [snapshot, setSnapshot] = React.useState(() => ({
    initialModules,
    enabledModules: normalizedInitialModules
  }));
  const enabledModules = snapshot.initialModules === initialModules
    ? snapshot.enabledModules
    : normalizedInitialModules;
  if (snapshot.initialModules !== initialModules) {
    setSnapshot({ initialModules, enabledModules });
  }

  const value = React.useMemo<EnabledModulesContextValue>(
    () => ({
      enabledModules,
      setModuleEnabled: (slug, enabled) => {
        setSnapshot((prev) => {
          const next = new Set(
            prev.initialModules === initialModules
              ? prev.enabledModules
              : normalizedInitialModules
          );
          if (enabled) {
            next.add(slug);
          } else {
            next.delete(slug);
          }
          return {
            initialModules,
            enabledModules: normalizeModules(Array.from(next))
          };
        });
      },
      replaceEnabledModules: (modules) => {
        setSnapshot({
          initialModules,
          enabledModules: normalizeModules(modules)
        });
      }
    }),
    [enabledModules, initialModules, normalizedInitialModules]
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
