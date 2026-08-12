import { RichTextHtml } from '@rustok/richtext/view';
import { getTranslations } from 'next-intl/server';

import { fetchPublishedPost, fetchPublishedPosts, type BlogGraphqlExecutor } from "../api/posts";
import { BlogCommentComposer } from './blog-comment-composer';
import { PostCard } from "./post-card";

export async function BlogSection({ graphql, tenantId, tenantSlug, locale, selectedSlug }: {
  graphql: BlogGraphqlExecutor;
  tenantId: string | null;
  tenantSlug: string | null;
  locale: string;
  selectedSlug: string | null;
}) {
  if (!tenantId) return null;

  let posts;
  try {
    posts = (await fetchPublishedPosts(graphql, tenantId, tenantSlug)).items;
  } catch {
    return null;
  }
  if (posts.length === 0) return null;
  const selectedPost = selectedSlug
    ? await fetchPublishedPost(graphql, tenantId, tenantSlug, selectedSlug, locale)
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
            <BlogCommentComposer
              tenantId={tenantId}
              tenantSlug={tenantSlug}
              postId={selectedPost.id}
              contentLocale={selectedPost.effectiveLocale}
            />
          )}
          <section className="mt-8 border-t border-border pt-6">
            <h3 className="text-lg font-semibold text-foreground">{t('comments.title')}</h3>
            {degradedCommentsMessage && (
              <p className="mt-3 rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground" role="status">
                {degradedCommentsMessage}
              </p>
            )}
            {selectedPost.publicComments.availability !== 'AVAILABLE' &&
            !selectedPost.publicComments.cachedSnapshot ? null :
            selectedPost.publicComments.items.length === 0 ? (
              <p className="mt-3 text-sm text-muted-foreground">{t('comments.empty')}</p>
            ) : (
              <div className="mt-4 space-y-3">
                {selectedPost.publicComments.items.map((comment) => (
                  <article key={comment.id} className="rounded-xl border border-border p-4">
                    <p className="whitespace-pre-line text-sm leading-6 text-foreground">{comment.contentPreview}</p>
                  </article>
                ))}
              </div>
            )}
          </section>
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
