import type { Metadata } from 'next';
import { notFound, redirect } from 'next/navigation';
import { cookies } from 'next/headers';
import { ADMIN_TENANT_KEY, ADMIN_TOKEN_KEY } from '@/shared/lib/auth';

import {
  TopicDetailView,
  fetchStorefrontTopic,
  fetchStorefrontReplies,
  resolveStorefrontTopicRoute,
} from '@rustok/forum-frontend';
import { getStorefrontTenantId, getStorefrontTenantSlug } from '@/shared/api/modules';

// This route reads per-request audience data. Never put personalized topic
// bodies into the public prerender cache or pass credentials to client props.
export const dynamic = 'force-dynamic';

interface TopicPageProps {
  params: Promise<{ locale: string; slug: string[] }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

async function resolveTopicId(
  locale: string,
  slugParts: string[],
  tenantId?: string | null,
  tenantSlug?: string | null,
  token?: string
): Promise<{ topicId: string | null; shouldRedirectTo?: string }> {
  // Case 1: [shortId, slug] e.g. /t/123456789abc/welcome-to-the-community
  if (slugParts.length >= 2) {
    const [shortId, slug] = slugParts;
    const decision = await resolveStorefrontTopicRoute({
      tenantId: tenantId ?? undefined,
      tenantSlug: tenantSlug ?? undefined,
      token,
      locale,
      shortId,
      slug,
    });

    if (decision?.disposition === 'REDIRECT' && decision.canonical?.path) {
      return { topicId: decision.canonical.topicId, shouldRedirectTo: decision.canonical.path };
    }

    if (decision?.canonical?.topicId) {
      return { topicId: decision.canonical.topicId };
    }
  }

  // Case 2: [topicIdOrSlug]
  const singlePart = slugParts[0];
  if (singlePart) {
    // If it looks like a full UUID (36 chars with dashes)
    if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(singlePart)) {
      return { topicId: singlePart };
    }

    // Attempt shortId / slug resolution for canonical redirect
    const decision = await resolveStorefrontTopicRoute({
      tenantId: tenantId ?? undefined,
      tenantSlug: tenantSlug ?? undefined,
      token,
      locale,
      shortId: singlePart,
      slug: '',
    });

    if (decision?.disposition === 'REDIRECT' && decision.canonical?.path) {
      return { topicId: decision.canonical.topicId, shouldRedirectTo: decision.canonical.path };
    }

    if (decision?.canonical?.topicId) {
      return { topicId: decision.canonical.topicId };
    }
  }

  return { topicId: null };
}

export async function generateMetadata({ params }: TopicPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  const { topicId } = await resolveTopicId(locale, slug, tenantId, tenantSlug);

  if (!topicId) {
    return {
      title: 'Topic | Forum | RusToK',
    };
  }

  const topic = await fetchStorefrontTopic({
    tenantId: tenantId ?? undefined,
    tenantSlug: tenantSlug ?? undefined,
    topicId,
    locale,
  });

  if (!topic) {
    return {
      title: 'Topic Not Found | Forum | RusToK',
    };
  }

  const description =
    topic.bodyPlainText?.trim().slice(0, 160) || 'Community forum discussion thread';

  return {
    title: `${topic.title} | Forum | RusToK`,
    description,
    openGraph: {
      title: topic.title,
      description,
      type: 'article',
    },
  };
}

export default async function ForumTopicPage({ params, searchParams }: TopicPageProps) {
  const { locale, slug } = await params;
  // `?reply=<uuid>` is appended to this route by the forum search projection so a result points at
  // one post; the view validates the id and resolves the anchor.
  const query = await searchParams;
  const replyParam = typeof query.reply === 'string' ? query.reply : null;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();
  const cookieStore = await cookies();
  const cookieTenant = cookieStore.get(ADMIN_TENANT_KEY)?.value ?? 'demo';
  const token = !tenantSlug || cookieTenant === tenantSlug
    ? cookieStore.get(ADMIN_TOKEN_KEY)?.value
    : undefined;

  const { topicId, shouldRedirectTo } = await resolveTopicId(
    locale,
    slug,
    tenantId,
    tenantSlug,
    token
  );

  if (shouldRedirectTo) {
    // Keep the addressed post: a canonical redirect that drops the query silently loses the
    // anchor, and the reader lands at the top of a long thread.
    redirect(
      replyParam
        ? `${shouldRedirectTo}?reply=${encodeURIComponent(replyParam)}`
        : shouldRedirectTo
    );
  }

  if (!topicId) {
    notFound();
  }

  const [topic, repliesData] = await Promise.all([
    fetchStorefrontTopic({
      tenantId: tenantId ?? undefined,
      tenantSlug: tenantSlug ?? undefined,
      token,
      topicId,
      locale,
    }),
    fetchStorefrontReplies({
      tenantId: tenantId ?? undefined,
      tenantSlug: tenantSlug ?? undefined,
      token,
      topicId,
      locale,
    }),
  ]);

  if (!topic) {
    notFound();
  }

  return (
    <main className="min-h-screen bg-background py-8">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">
        <TopicDetailView
          tenantId={tenantId}
          tenantSlug={tenantSlug}
          locale={locale}
          topicId={topic.id}
          initialTopic={topic}
          initialReplies={repliesData.items}
          initialRepliesTotal={topic.replyCount}
          initialRepliesCursor={repliesData.nextCursor}
          focusReplyId={replyParam}
        />
      </div>
    </main>
  );
}
