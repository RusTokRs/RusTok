'use client';

import { AiAdminPage } from '@rustok/ai-admin';
import { graphqlRequest } from '@/shared/api/graphql';

type AiAdminClientProps = {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
  section?: 'overview' | 'diagnostics';
};

export function AiAdminClient(props: AiAdminClientProps) {
  return <AiAdminPage {...props} graphql={graphqlRequest} />;
}
