import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';

interface CreateForumReplyInput {
  locale: string;
  content: string;
  contentFormat: 'markdown' | 'rt_json_v1';
  contentJson?: Record<string, unknown>;
  parentReplyId?: string;
}

export async function createForumReply(
  topicId: string,
  input: CreateForumReplyInput,
  opts: GqlOpts = {}
): Promise<string> {
  const mutation = `
    mutation CreateForumReply($tenantId: UUID!, $topicId: UUID!, $input: CreateForumReplyInput!) {
      createForumReply(tenantId: $tenantId, topicId: $topicId, input: $input)
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; topicId: string; input: CreateForumReplyInput },
    { createForumReply: string }
  >(mutation, { tenantId: opts.tenantId!, topicId, input }, opts.token, opts.tenantSlug);

  return data.createForumReply;
}
