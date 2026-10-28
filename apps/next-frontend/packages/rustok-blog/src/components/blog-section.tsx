import { RichTextHtml } from '@rustok/richtext/view';
import { getTranslations } from '@rustok/next-fluent/server';

import { storefrontGraphql } from '@/shared/lib/graphql';
import { fetchPublishedPost, fetchPublishedPosts } from "../api/posts";
import { BlogCommentsSection } from './blog-comments-section';
import { PostCard } from "./post-card";

export async function BlogSection({ tenantId, tenantSlug, locale, selectedSlug }: {
  tenantId: string | null;
  tenantSlug: string | null;
  locale: string;
  selectedSlug: string | null;
}) {
  if (!tenantId) return null;

  let posts;
  try {
    posts = (
      await fetchPublishedPosts(storefrontGraphql, tenantId, tenantSlug, null, 6, undefined, undefined, locale)
    ).items;
  } catch {
    return null;
  }
  if (posts.length === 0) return null;
  const selectedPost = selectedSlug
    ? await fetchPublishedPost(storefrontGraphql, tenantId, tenantSlug, selectedSlug, locale)
    : null;
  const t = await getTranslations('Blog');
  const degradedCommentsMessage = selectedPost
    ? selectedPost.publicComments.availability === 'UNAVAILABLE'
      ? t(
          selectedPost.publicComments.cachedSnapshot
            ? 'comments.unavailableCached'
            : 'comments.unavailable'
        )
      : selectedPost.publicComments.availability === 'TIMEOUT'
        ? t(
            selectedPost.publicComments.cachedSnapshot
              ? 'comments.timeoutCached'
              : 'comments.timeout'
          )
        : selectedPost.publicComments.availability === 'READ_ONLY'
          ? t('comments.readOnly')
          : null
    : null;

  return (
    <div className="space-y-6">
      {selectedPost && (
        <article className="rounded-2xl border border-border bg-card p-6">
          <h2 className="text-2xl font-semibold text-foreground">{selectedPost.title}</h2>
          {selectedPost.excerpt && <p className="mt-2 text-muted-foreground">{selectedPost.excerpt}</p>}
          <RichTextHtml
            view={selectedPost.content}
            contentLocale={selectedPost.effectiveLocale}
            className="mt-5 text-sm leading-7 text-foreground"
          />
          {tenantSlug && (
            <BlogCommentsSection
              tenantId={tenantId}
              tenantSlug={tenantSlug}
              postId={selectedPost.id}
              contentLocale={selectedPost.effectiveLocale}
              comments={selectedPost.publicComments}
              degradedMessage={degradedCommentsMessage}
              locale={locale}
            />
          )}
        </article>
      )}
      <div>
        <h2 className="text-2xl font-semibold text-foreground">{t('latest.title')}</h2>
        <p className="mt-1 text-sm text-muted-foreground">{t('latest.subtitle')}</p>
      </div>
      <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
        {posts.map((post) => (
          <PostCard key={post.id} post={post} href={post.slug ? `/${locale}?slug=${encodeURIComponent(post.slug)}` : null} />
        ))}
      </div>
    </div>
  );
}
