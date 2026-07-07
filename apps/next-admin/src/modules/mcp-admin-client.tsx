'use client';

import { McpAdminPage, type McpAdminPageProps } from '@rustok/mcp-admin';
import { graphqlRequest } from '@/shared/api/graphql';

type McpAdminClientProps = Omit<McpAdminPageProps, 'graphql'>;

export function McpAdminClient(props: McpAdminClientProps) {
  return <McpAdminPage {...props} graphql={graphqlRequest} />;
}
