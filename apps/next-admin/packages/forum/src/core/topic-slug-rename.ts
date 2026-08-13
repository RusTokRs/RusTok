export const MAX_FORUM_TOPIC_ROUTE_LOCALE_LENGTH = 64;
export const MAX_FORUM_TOPIC_ROUTE_SLUG_LENGTH = 255;

export interface ForumTopicSlugRenameCandidate {
  id: string;
  title: string;
  locale: string;
  slug: string;
}

export interface ForumTopicSlugRenameCommand {
  topicId: string;
  locale: string;
  slug: string;
}

export interface ForumTopicRouteDescriptor {
  topicId: string;
  locale: string;
  shortId: string;
  slug: string;
  path: string;
}

export interface ForumTopicSlugRenameReceipt {
  topicId: string;
  locale: string;
  previousSlug: string;
  slug: string;
  previousPath: string;
  canonical: ForumTopicRouteDescriptor;
  aliasId?: string | null;
  changed: boolean;
}

export function forumTopicSlugRenameCandidateLabel(
  candidate: ForumTopicSlugRenameCandidate
): string {
  return `${candidate.title} · ${candidate.locale} · /${candidate.slug}`;
}

export function buildForumTopicSlugRenameCommand(input: {
  candidate: ForumTopicSlugRenameCandidate;
  slug: string;
}): ForumTopicSlugRenameCommand {
  const topicId = input.candidate.id.trim();
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
      topicId
    )
  ) {
    throw new Error('Topic identity is invalid.');
  }

  const locale = input.candidate.locale.trim();
  if (!locale) {
    throw new Error('Topic locale is required.');
  }
  if ([...locale].length > MAX_FORUM_TOPIC_ROUTE_LOCALE_LENGTH) {
    throw new Error(
      `Topic locale must not exceed ${MAX_FORUM_TOPIC_ROUTE_LOCALE_LENGTH} characters.`
    );
  }
  if (/[\u0000-\u001f\u007f]/u.test(locale)) {
    throw new Error('Topic locale must not contain control characters.');
  }

  const slug = input.slug.trim();
  if (!slug) {
    throw new Error('New topic slug is required.');
  }
  if ([...slug].length > MAX_FORUM_TOPIC_ROUTE_SLUG_LENGTH) {
    throw new Error(
      `Topic slug must not exceed ${MAX_FORUM_TOPIC_ROUTE_SLUG_LENGTH} characters.`
    );
  }
  if (/[\u0000-\u001f\u007f]/u.test(slug)) {
    throw new Error('Topic slug must not contain control characters.');
  }

  return {
    topicId,
    locale,
    slug
  };
}
