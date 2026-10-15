import type { MetadataRoute } from "next";

import { resolveSitemapMetadata } from "@/shared/seo/runtime";

// Owner visibility and anonymous-reading settings can change after deployment.
// Never serve an older static sitemap containing a now-private Forum route.
export const dynamic = "force-dynamic";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  return resolveSitemapMetadata();
}
