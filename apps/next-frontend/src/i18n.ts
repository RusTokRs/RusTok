import { getRequestConfig } from "next-intl/server";

const messageLoaders = {
  en: () => import("../messages/en.json").then((module) => module.default),
  ru: () => import("../messages/ru.json").then((module) => module.default),
} as const;

export type Locale = keyof typeof messageLoaders;
export const locales = Object.keys(messageLoaders) as Locale[];
export const defaultLocale = "en";

function matchSupportedLocale(value?: string | null): Locale | undefined {
  const normalized = value?.trim().replaceAll("_", "-").toLowerCase();
  if (!normalized) return undefined;

  return (
    locales.find((locale) => locale.toLowerCase() === normalized) ??
    locales.find((locale) => locale.toLowerCase() === normalized.split("-")[0])
  );
}

export function resolveLocale(value?: string | null): Locale {
  return matchSupportedLocale(value) ?? defaultLocale;
}

export default getRequestConfig(async ({ locale }) => {
  const resolvedLocale = resolveLocale(locale);

  return {
    locale: resolvedLocale,
    messages: await messageLoaders[resolvedLocale](),
  };
});
