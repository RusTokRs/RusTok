import type { MetadataRoute } from "next";

import { resolveSitemapMetadata } from "@/shared/seo/runtime";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  return resolveSitemapMetadata();
}
