import { McpAdminPage } from '@rustok/mcp-admin';
import { getServerSession } from 'next-auth';

import { authOptions } from '@/shared/auth/options';
import { tenantSlugFromSession } from '@/shared/auth/tenant';

export default async function McpPage() {
  const session = await getServerSession(authOptions);
  const token = session?.accessToken ?? null;
  const tenantSlug = tenantSlugFromSession(session);

  return <McpAdminPage token={token} tenantSlug={tenantSlug} />;
}
