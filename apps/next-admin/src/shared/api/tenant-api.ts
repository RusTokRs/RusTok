import 'server-only';

import type { Tenant } from '@/entities/tenant';
import { graphqlRequest } from './graphql';

const CURRENT_TENANT_QUERY =
  'query CurrentTenant { currentTenant { id name slug } }';

interface CurrentTenantResponse {
  currentTenant: Tenant;
}

export async function fetchCurrentTenant(
  token: string,
  tenantSlug: string
): Promise<Tenant | null> {
  try {
    const data = await graphqlRequest<
      undefined,
      CurrentTenantResponse
    >(CURRENT_TENANT_QUERY, undefined, token, tenantSlug);
    return data.currentTenant;
  } catch {
    return null;
  }
}
