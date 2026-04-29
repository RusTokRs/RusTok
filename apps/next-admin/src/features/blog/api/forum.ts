import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';

export interface ForumTopicSummary {
  id: string;
  title: string;
  slug: string;
  categoryId: string;
  replyCount: number;
}

interface CreateForumReplyInput {
  locale: string;
  content: string;
  contentFormat: 'markdown' | 'rt_json_v1';
  contentJson?: Record<string, unknown>;
  parentReplyId?: string;
}

export async function listForumTopics(
  opts: GqlOpts = {},
  input: { locale?: string; first?: number } = {}
): Promise<ForumTopicSummary[]> {
  const query = `
    query ForumTopics($tenantId: UUID!, $locale: String, $pagination: PaginationInput!) {
      forumTopics(tenantId: $tenantId, locale: $locale, pagination: $pagination) {
        items {
          id
          title
          slug
          categoryId
          replyCount
        }
      }
    }
  `;

  const data = await graphqlRequest<
    {
      tenantId: string;
      locale?: string;
      pagination: { first: number };
    },
    {
      forumTopics: { items: ForumTopicSummary[] };
    }
  >(
    query,
    {
      tenantId: opts.tenantId!,
      locale: input.locale,
      pagination: { first: input.first ?? 50 }
    },
    opts.token,
    opts.tenantSlug
  );

  return data.forumTopics.items;
}

export async function createForumReply(
  topicId: string,
  input: CreateForumReplyInput,
  opts: GqlOpts = {}
): Promise<string> {
  const mutation = `
    mutation CreateForumReply($tenantId: UUID!, $topicId: UUID!, $input: CreateForumReplyInput!) {
      createForumReply(tenantId: $tenantId, topicId: $topicId, input: $input) {
        id
      }
    }
  `;

  const data = await graphqlRequest<
    { tenantId: string; topicId: string; input: CreateForumReplyInput },
    { createForumReply: { id: string } }
  >(
    mutation,
    { tenantId: opts.tenantId!, topicId, input },
    opts.token,
    opts.tenantSlug
  );

  return data.createForumReply.id;
}
