import { createI18nMiddleware } from "@rustok/next-fluent/middleware";

import { defaultLocale, locales } from "./src/i18n";

export default createI18nMiddleware({
  locales,
  defaultLocale,
});

export const config = {
  matcher: ["/", "/((?!api|_next|_vercel|.*\\..*).*)"],
};
