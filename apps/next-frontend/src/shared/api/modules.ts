import { storefrontGraphql } from "@/shared/lib/graphql";

const ENABLED_MODULES_QUERY = `
  query EnabledModules {
    enabledModules(limit: 100)
  }
`;

type EnabledModulesResponse = {
  enabledModules?: unknown;
};

function normalizeModules(modules: string[]): string[] {
  return Array.from(new Set(modules)).sort();
}

export function getStorefrontTenantSlug(): string | null {
  return (
    process.env.NEXT_PUBLIC_TENANT_SLUG ??
    process.env.NEXT_PUBLIC_DEFAULT_TENANT_SLUG ??
    null
  );
}

export function getStorefrontTenantId(): string | null {
  return process.env.NEXT_PUBLIC_TENANT_ID ?? null;
}

export async function fetchEnabledModules(
  tenantSlug = getStorefrontTenantSlug(),
): Promise<string[]> {
  if (!tenantSlug?.trim()) {
    throw new Error("Storefront module availability scope is missing");
  }

  const response = await storefrontGraphql<EnabledModulesResponse>({
    query: ENABLED_MODULES_QUERY,
    tenant: tenantSlug,
  });

  if (response.errors?.length) {
    throw new Error("Storefront module availability is unavailable");
  }

  const modules = response.data?.enabledModules;
  if (
    !Array.isArray(modules) ||
    modules.some((slug) => typeof slug !== "string" || !slug.trim())
  ) {
    throw new Error("Storefront module availability response is invalid");
  }
  // The server clamps the unpaginated query at 100. At the cap we cannot
  // distinguish a complete policy from a truncated one.
  if (modules.length >= 100) {
    throw new Error("Storefront module availability may be incomplete");
  }
  return normalizeModules(modules);
}
