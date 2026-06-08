import type { MetadataRoute } from "next";

import { resolveRobotsMetadata } from "@/shared/seo/runtime";

export default async function robots(): Promise<MetadataRoute.Robots> {
  return resolveRobotsMetadata();
}
