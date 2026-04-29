// i18n is now handled by next-intl.
//
// Server Components: import { getTranslations } from 'next-intl/server';
// Client Components: import { useTranslations } from 'next-intl';
//
// Locale is resolved by the host runtime and exposed through next-intl.
// See: next.config.ts (createNextIntlPlugin) and src/app/layout.tsx (NextIntlClientProvider).

export { useTranslations, useLocale } from 'next-intl';
