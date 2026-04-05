import { getRequestConfig } from "next-intl/server";

export const locales = ["en", "ru"] as const;
export const defaultLocale = "en";

export default getRequestConfig(async ({ locale }) => {
  const requestedLocale = locale ?? defaultLocale;
  const resolvedLocale = locales.includes(
    requestedLocale as (typeof locales)[number],
  )
    ? requestedLocale
    : defaultLocale;

  return {
    locale: resolvedLocale,
    messages: (await import(`../messages/${resolvedLocale}.json`)).default,
  };
});
