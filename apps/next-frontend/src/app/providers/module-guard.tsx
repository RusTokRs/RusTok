import { notFound } from "next/navigation";
import type { ReactNode } from "react";
import { fetchEnabledModules, getStorefrontTenantSlug } from "@/shared/api/modules";

export async function ModuleGuard({
  slug,
  children,
  fallback,
}: {
  slug: string;
  children: ReactNode;
  fallback?: ReactNode;
}) {
  const tenantSlug = getStorefrontTenantSlug();
  let enabledModules: string[] = [];

  if (tenantSlug) {
    try {
      enabledModules = await fetchEnabledModules(tenantSlug);
    } catch (error) {
      console.warn(`Failed to verify module '${slug}' storefront availability:`, error);
    }
  }

  if (!enabledModules.includes(slug)) {
    if (fallback) {
      return <>{fallback}</>;
    }
    notFound();
  }

  return <>{children}</>;
}

