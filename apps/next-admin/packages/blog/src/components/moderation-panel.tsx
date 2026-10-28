'use client';

import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  listBlogModerationComments,
  moderateBlogComment,
  type BlogModerationPage,
  type BlogModerationStatus
} from '../api/moderation';
import type { GqlOpts } from '../api/posts';

const PER_PAGE = 20;
const STATUSES: ReadonlyArray<{ value: BlogModerationStatus; label: string }> = [
  { value: 'APPROVED', label: 'Approve' },
  { value: 'SPAM', label: 'Mark spam' },
  { value: 'TRASH', label: 'Move to trash' }
];

/** The same Blog-owned moderation projection and lifecycle as the Leptos admin. */
export function BlogModerationPanel({
  postId,
  locale,
  gqlOpts
}: {
  postId: string;
  locale: string;
  gqlOpts: GqlOpts;
}) {
  const { token, tenantSlug, tenantId } = gqlOpts;
  const [page, setPage] = useState(1);
  const [revision, setRevision] = useState(0);
  const [data, setData] = useState<BlogModerationPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setLoadError(null);
    listBlogModerationComments(postId, locale, page, PER_PAGE, {
      token,
      tenantSlug,
      tenantId
    })
      .then((result) => {
        if (!active) return;
        if (page > 1 && (page - 1) * PER_PAGE >= result.total) {
          setPage(Math.max(1, Math.ceil(result.total / PER_PAGE)));
          return;
        }
        setData(result);
      })
      .catch((error: unknown) => {
        if (!active) return;
        setData(null);
        setLoadError(error instanceof Error ? error.message : 'Moderation queue is unavailable');
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => { active = false; };
  }, [postId, locale, page, revision, token, tenantSlug, tenantId]);

  const moderate = async (commentId: string, status: BlogModerationStatus) => {
    if (pendingId) return;
    setPendingId(commentId);
    try {
      await moderateBlogComment(commentId, status, locale, { token, tenantSlug, tenantId });
      toast.success('Comment status updated');
      setRevision((value) => value + 1);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to moderate comment');
    } finally {
      setPendingId(null);
    }
  };

  const total = data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  return (
    <Card className='mt-8'>
      <CardHeader>
        <CardTitle>Comment moderation</CardTitle>
        <CardDescription>
          Review non-deleted comments on this post. Changes use the Blog/Comments owner permissions.
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-4'>
        {loading && <p className='text-muted-foreground text-sm'>Loading moderation queue...</p>}
        {loadError && (
          <p role='alert' className='text-destructive text-sm'>
            Moderation is unavailable: {loadError}
          </p>
        )}
        {!loading && !loadError && data?.items.length === 0 && (
          <p className='text-muted-foreground text-sm'>No comments to moderate.</p>
        )}
        {!loading && !loadError && data?.items.map((comment) => (
          <article key={comment.id} className='space-y-3 rounded-lg border p-4'>
            <div className='text-muted-foreground flex flex-wrap gap-3 text-xs'>
              <span className='font-semibold'>{comment.status}</span>
              <span>{comment.effectiveLocale}</span>
              <time dateTime={comment.createdAt}>
                {new Date(comment.createdAt).toLocaleString(locale)}
              </time>
            </div>
            <p className='whitespace-pre-wrap break-words text-sm'>{comment.contentPreview}</p>
            <div className='flex flex-wrap gap-2'>
              {STATUSES.map((action) => (
                <Button
                  key={action.value}
                  type='button'
                  variant='outline'
                  size='sm'
                  disabled={pendingId !== null}
                  onClick={() => void moderate(comment.id, action.value)}
                >
                  {pendingId === comment.id ? 'Saving...' : action.label}
                </Button>
              ))}
            </div>
          </article>
        ))}
        {!loading && !loadError && (
          <nav aria-label='Comment moderation pages' className='flex items-center justify-end gap-3 text-sm'>
            <span>{total} comments · Page {page} of {totalPages}</span>
            <Button type='button' size='sm' variant='outline' disabled={page <= 1 || pendingId !== null}
              onClick={() => setPage((value) => Math.max(1, value - 1))}>
              Previous
            </Button>
            <Button type='button' size='sm' variant='outline' disabled={page >= totalPages || pendingId !== null}
              onClick={() => setPage((value) => Math.min(totalPages, value + 1))}>
              Next
            </Button>
          </nav>
        )}
      </CardContent>
    </Card>
  );
}
