import { Suspense } from 'react';

import { auth } from '@/auth';
import { McpAdminClient } from '@/modules/mcp-admin-client';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'Dashboard: MCP'
};

export default async function McpPage() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='MCP'
      pageDescription='Manage MCP clients, token policies, audit events, and Alloy scaffold drafts'
    >
      <Suspense fallback={<div>Loading MCP control plane...</div>}>
        <McpAdminClient token={token} tenantSlug={tenantSlug} />
      </Suspense>
    </PageContainer>
  );
}
