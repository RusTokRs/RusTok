// i18n is now handled by next-intl.
//
// Server Components: import { getTranslations } from 'next-intl/server';
// Client Components: import { useTranslations } from 'next-intl';
//
// Locale is stored in cookie 'rustok-admin-locale' and read by src/i18n/request.ts.
// See: next.config.ts (createNextIntlPlugin) and src/app/layout.tsx (NextIntlClientProvider).

export { useTranslations, useLocale } from 'next-intl';
