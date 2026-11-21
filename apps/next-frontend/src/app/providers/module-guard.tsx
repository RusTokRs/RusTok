import { notFound } from "next/navigation";
import type { ReactNode } from "react";
import {
  fetchEnabledModules,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { ModulePolicyError } from "./module-policy-error";

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
  let enabledModules: string[];

  try {
    enabledModules = await fetchEnabledModules(tenantSlug);
  } catch {
    return <ModulePolicyError />;
  }

  if (!enabledModules.includes(slug)) {
    if (fallback) {
      return <>{fallback}</>;
    }
    notFound();
  }

  return <>{children}</>;
}
