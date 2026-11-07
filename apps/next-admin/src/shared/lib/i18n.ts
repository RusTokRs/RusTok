// i18n is handled by @rustok/next-fluent (Project Fluent).
//
// Server Components: import { getTranslations, getLocale } from '@rustok/next-fluent/server';
// Client Components: import { useTranslations, useLocale } from '@rustok/next-fluent/client';
//
// Locale is resolved by the host runtime and exposed through next-fluent.

export { useTranslations, useLocale } from '@rustok/next-fluent/client';
export { getTranslations, getLocale } from '@rustok/next-fluent/server';
