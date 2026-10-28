import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';

export type BlogModerationStatus = 'APPROVED' | 'SPAM' | 'TRASH';

export interface BlogModerationComment {
  id: string;
  effectiveLocale: string;
  authorId: string | null;
  contentPreview: string;
  status: string;
  parentCommentId: string | null;
  createdAt: string;
}

export interface BlogModerationPage {
  items: BlogModerationComment[];
  total: number;
}

const COMMENTS_QUERY = `
query BlogModerationComments($postId: UUID!, $locale: String, $page: Int!, $perPage: Int!) {
  post(id: $postId, locale: $locale) {
    moderationComments(locale: $locale, page: $page, perPage: $perPage) {
      total
      items { id effectiveLocale authorId contentPreview status parentCommentId createdAt }
    }
  }
}
`;

const MODERATE_COMMENT_MUTATION = `
mutation ModerateBlogComment($id: UUID!, $commandId: UUID!, $status: BlogCommentModerationStatus!, $locale: String) {
  moderateComment(id: $id, commandId: $commandId, status: $status, locale: $locale)
}
`;

export async function listBlogModerationComments(
  postId: string,
  locale: string,
  page: number,
  perPage: number,
  opts: GqlOpts
): Promise<BlogModerationPage> {
  const response = await graphqlRequest<
    { postId: string; locale: string; page: number; perPage: number },
    { post: { moderationComments: BlogModerationPage } | null }
  >(
    COMMENTS_QUERY,
    { postId, locale, page: Math.max(1, page), perPage: Math.max(1, Math.min(perPage, 100)) },
    opts.token,
    opts.tenantSlug
  );
  if (!response.post) throw new Error('Blog post is unavailable for moderation');
  return response.post.moderationComments;
}

export async function moderateBlogComment(
  id: string,
  status: BlogModerationStatus,
  locale: string,
  opts: GqlOpts
): Promise<void> {
  const commandId = crypto.randomUUID();
  const response = await graphqlRequest<
    { id: string; commandId: string; status: BlogModerationStatus; locale: string },
    { moderateComment: boolean }
  >(
    MODERATE_COMMENT_MUTATION,
    { id, commandId, status, locale },
    opts.token,
    opts.tenantSlug
  );
  if (!response.moderateComment) {
    throw new Error('Blog comment moderation was rejected by the owner');
  }
}
