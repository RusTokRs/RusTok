import { storefrontGraphql } from "@/shared/lib/graphql";

const ENABLED_MODULES_QUERY = `
  query EnabledModules {
    enabledModules
  }
`;

type EnabledModulesResponse = {
  enabledModules?: string[] | null;
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

export async function fetchEnabledModules(
  tenantSlug = getStorefrontTenantSlug(),
): Promise<string[]> {
  if (!tenantSlug) {
    return [];
  }

  const response = await storefrontGraphql<EnabledModulesResponse>({
    query: ENABLED_MODULES_QUERY,
    tenant: tenantSlug,
  });

  if (response.errors?.length) {
    return [];
  }

  return normalizeModules(response.data?.enabledModules ?? []);
}
