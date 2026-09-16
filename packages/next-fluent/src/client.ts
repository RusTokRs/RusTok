'use client';

import React, { createContext, useContext, useMemo } from 'react';
import type { FluentBundle } from '@fluent/bundle';
import type { Translations } from './types';
import { createFluentBundle, createTranslator } from './bundle';

interface FluentContextValue {
  locale: string;
  bundle: FluentBundle | null;
  fallbackLocale?: string;
  fallbackBundle: FluentBundle | null;
  fallbackBundles?: readonly FluentBundle[];
  debug?: boolean;
}

const FluentContext = createContext<FluentContextValue>({
  locale: 'en',
  bundle: null,
  fallbackBundle: null,
  debug: false,
});

export interface FluentProviderProps {
  locale: string;
  messages: string | readonly string[] | FluentBundle;
  fallbackLocale?: string;
  fallbackMessages?: string | readonly string[] | FluentBundle;
  fallbackBundles?: FluentBundle | readonly FluentBundle[];
  debug?: boolean;
  children: React.ReactNode;
}

export function FluentProvider({
  locale,
  messages,
  fallbackLocale,
  fallbackMessages,
  fallbackBundles,
  debug,
  children,
}: FluentProviderProps) {
  const messagesKey =
    typeof messages === 'string'
      ? messages
      : Array.isArray(messages)
        ? messages.join('\u0000')
        : null;

  const fallbackMessagesKey =
    typeof fallbackMessages === 'string'
      ? fallbackMessages
      : Array.isArray(fallbackMessages)
        ? fallbackMessages.join('\u0000')
        : null;

  const bundle = useMemo<FluentBundle | null>(() => {
    if (!messages) return null;
    if (typeof messages === 'string' || Array.isArray(messages)) {
      return createFluentBundle(locale, messages);
    }
    return messages as FluentBundle;
  }, [locale, messagesKey ?? messages]);

  const fallbackBundle = useMemo<FluentBundle | null>(() => {
    if (!fallbackMessages) return null;
    const fLocale = fallbackLocale || 'en';
    if (typeof fallbackMessages === 'string' || Array.isArray(fallbackMessages)) {
      return createFluentBundle(fLocale, fallbackMessages);
    }
    return fallbackMessages as FluentBundle;
  }, [fallbackLocale, fallbackMessagesKey ?? fallbackMessages]);

  const resolvedFallbackBundles = useMemo<readonly FluentBundle[] | undefined>(() => {
    if (fallbackBundles) {
      return Array.isArray(fallbackBundles) ? fallbackBundles : [fallbackBundles];
    }
    if (fallbackBundle) {
      return [fallbackBundle];
    }
    return undefined;
  }, [fallbackBundles, fallbackBundle]);

  const value = useMemo(
    () => ({
      locale,
      bundle,
      fallbackLocale,
      fallbackBundle,
      fallbackBundles: resolvedFallbackBundles,
      debug,
    }),
    [locale, bundle, fallbackLocale, fallbackBundle, resolvedFallbackBundles, debug]
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
    () =>
      createTranslator(context.bundle, {
        fallbackBundles:
          context.fallbackBundles ??
          (context.fallbackBundle ? [context.fallbackBundle] : null),
        namespace,
        debug: context.debug,
      }),
    [context.bundle, context.fallbackBundle, context.fallbackBundles, namespace, context.debug]
  );
}

