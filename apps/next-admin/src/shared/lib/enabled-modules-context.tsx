'use client';

import React from 'react';

type EnabledModulesContextValue = {
  enabledModules: string[];
};

const EnabledModulesContext =
  React.createContext<EnabledModulesContextValue | null>(null);

export function EnabledModulesClientProvider({
  initialModules,
  children
}: {
  initialModules: string[];
  children: React.ReactNode;
}) {
  // Only the owner-issued policy projection may determine availability.
  // A lifecycle mutation result is not an effective-policy decision.
  const value = React.useMemo<EnabledModulesContextValue>(
    () => ({ enabledModules: Array.from(new Set(initialModules)).sort() }),
    [initialModules]
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
