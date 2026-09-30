import { setRequestConfig } from "@rustok/next-fluent/server";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  defaultLocale,
  locales,
  resolveLocale,
  type Locale,
} from "./i18n-config";

export { defaultLocale, locales, resolveLocale, type Locale };

/** Directory candidates are resolved once, without relying on CJS `__dirname`. */
function messageDirectories(): string[] {
  const candidates = [path.join(process.cwd(), "messages")];
  try {
    const here = path.dirname(fileURLToPath(import.meta.url));
    candidates.push(path.resolve(here, "../messages"));
  } catch {
    // Bundled CJS output has no `import.meta.url`; cwd is enough there.
  }
  candidates.push(path.resolve(process.cwd(), "apps/next-frontend/messages"));
  return candidates;
}

const ftlCache = new Map<Locale, string>();

async function readFtlMessages(locale: Locale): Promise<string | undefined> {
  const cached = ftlCache.get(locale);
  if (cached !== undefined && process.env.NODE_ENV === "production") {
    return cached;
  }

  for (const directory of messageDirectories()) {
    try {
      const content = await fs.readFile(
        /*turbopackIgnore: true*/ path.join(directory, `${locale}.ftl`),
        "utf8",
      );
      ftlCache.set(locale, content);
      return content;
    } catch {
      // try next candidate
    }
  }

  return undefined;
}

/**
 * Loads a locale catalog, falling back to the platform default locale.
 *
 * A missing default catalog is fatal: returning an empty catalog used to make
 * the storefront render bare message keys behind a single `console.warn`.
 */
async function loadFtlMessages(locale: Locale): Promise<string> {
  const requested = await readFtlMessages(locale);
  if (requested !== undefined) return requested;

  if (locale !== defaultLocale) {
    console.warn(
      `[next-frontend] No FTL catalog for locale '${locale}'; falling back to '${defaultLocale}'.`,
    );
    const fallback = await readFtlMessages(defaultLocale);
    if (fallback !== undefined) return fallback;
  }

  throw new Error(
    `[next-frontend] Required FTL catalog '${defaultLocale}.ftl' was not found in ` +
      `${messageDirectories().join(", ")}`,
  );
}

export default setRequestConfig(async ({ locale }) => {
  const resolvedLocale = resolveLocale(locale);

  return {
    locale: resolvedLocale,
    messages: await loadFtlMessages(resolvedLocale),
  };
});
