'use client';

import React, { createContext, useContext, useMemo } from 'react';
import type { FluentBundle } from '@fluent/bundle';
import type { Translations } from './types';
import { createFluentBundle, createTranslator } from './bundle';

interface FluentContextValue {
  locale: string;
  bundle: FluentBundle | null;
}

const FluentContext = createContext<FluentContextValue>({
  locale: 'en',
  bundle: null,
});

export interface FluentProviderProps {
  locale: string;
  messages: string | FluentBundle;
  children: React.ReactNode;
}

export function FluentProvider({
  locale,
  messages,
  children,
}: FluentProviderProps) {
  const bundle = useMemo(() => {
    if (!messages) return null;
    if (typeof messages === 'string') {
      return createFluentBundle(locale, messages);
    }
    return messages;
  }, [locale, messages]);

  const value = useMemo(
    () => ({
      locale,
      bundle,
    }),
    [locale, bundle]
  );

  return React.createElement(FluentContext.Provider, { value }, children);
}

export function useLocale(): string {
  const context = useContext(FluentContext);
  return context.locale;
}

export function useTranslations(namespace?: string): Translations {
  const context = useContext(FluentContext);
  return useMemo(
    () => createTranslator(context.bundle, namespace),
    [context.bundle, namespace]
  );
}
