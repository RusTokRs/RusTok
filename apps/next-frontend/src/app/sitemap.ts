import type { MetadataRoute } from "next";

import { getSiteUrl, locales, localizedPath } from "@/shared/seo/site";

export default function sitemap(): MetadataRoute.Sitemap {
  const siteUrl = getSiteUrl();
  return locales.map((locale: string) => ({
    url: `${siteUrl}${localizedPath(locale, "/")}`,
    changeFrequency: "daily",
    priority: 1,
  }));
}
