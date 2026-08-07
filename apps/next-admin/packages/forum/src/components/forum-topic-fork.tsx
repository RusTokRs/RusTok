'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useLocale } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import {
  forkForumTopicReplyBranch,
  listForumTopicReplies,
  type ForumTopicSummary,
  type GqlOpts
} from '../api/forum';
import {
  buildForumTopicForkCommand,
  forumTopicForkReplyLabel,
  newForumTopicForkIdentity,
  type ForumTopicForkIdentity,
  type ForumTopicForkReceipt,
  type ForumTopicForkReplyPage
} from '../core/topic-fork';
import en from './fork/locales/en.json';
import ru from './fork/locales/ru.json';

type ClientGqlOpts = Pick<GqlOpts, 'tenantId' | 'tenantSlug'>;

export function ForumTopicFork({
  topics,
  gqlOpts = {}
}: {
  topics: ForumTopicSummary[];
  gqlOpts?: ClientGqlOpts;
}) {
  const activeLocale = useLocale();
  const copy = activeLocale.toLowerCase().startsWith('ru') ? ru : en;
  const router = useRouter();
  const [sourceTopicId, setSourceTopicId] = useState('');
  const [replies, setReplies] = useState<ForumTopicForkReplyPage | null>(null);
  const [loadingReplies, setLoadingReplies] = useState(false);
  const [rootReplyId, setRootReplyId] = useState('');
  const [targetLocale, setTargetLocale] = useState(activeLocale || 'en');
  const [targetTitle, setTargetTitle] = useState('');
  const [targetSlug, setTargetSlug] = useState('');
  const [reason, setReason] = useState('');
  const [identity, setIdentity] = useState<ForumTopicForkIdentity>(
    newForumTopicForkIdentity
  );
  const [pending, setPending] = useState(false);
  const [receipt, setReceipt] = useState<ForumTopicForkReceipt | null>(null);

  const source = useMemo(
    () => topics.find((topic) => topic.id === sourceTopicId),
    [sourceTopicId, topics]
  );

  useEffect(() => {
    if (!sourceTopicId) {
      setReplies(null);
      setLoadingReplies(false);
      return;
    }

    let cancelled = false;
    setLoadingReplies(true);
    listForumTopicReplies(sourceTopicId, gqlOpts, {
      locale: source?.locale || activeLocale,
      first: 500
    })
      .then((page) => {
        if (!cancelled) {
          setReplies(page);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setReplies(null);
          toast.error(error instanceof Error ? error.message : copy.failure);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingReplies(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    activeLocale,
    copy.failure,
    gqlOpts.tenantId,
    gqlOpts.tenantSlug,
    source?.locale,
    sourceTopicId
  ]);

  function commandShapeChanged() {
    setIdentity(newForumTopicForkIdentity());
    setReceipt(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!source || !replies) {
      toast.error(copy.noReplies);
      return;
    }

    try {
      const command = buildForumTopicForkCommand({
        identity,
        sourceTopicId: source.id,
        replies,
        rootReplyId,
        locale: targetLocale,
        title: targetTitle,
        slug: targetSlug,
        reason
      });
      setPending(true);
      const result = await forkForumTopicReplyBranch(command, gqlOpts);
      setReceipt(result);
      toast.success(copy.success);
      setSourceTopicId('');
      setReplies(null);
      setRootReplyId('');
      setTargetTitle('');
      setTargetSlug('');
      setReason('');
      setIdentity(newForumTopicForkIdentity());
      router.refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : copy.failure);
    } finally {
      setPending(false);
    }
  }

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{copy.title}</CardTitle>
          <p className='text-sm text-muted-foreground'>{copy.subtitle}</p>
        </CardHeader>
        <CardContent>
          <form className='grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]' onSubmit={submit}>
            <div className='space-y-6'>
              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.source}</span>
                <select
                  className='w-full rounded-md border bg-background px-3 py-2'
                  value={sourceTopicId}
                  onChange={(event) => {
                    const value = event.target.value;
                    setSourceTopicId(value);
                    setRootReplyId('');
                    const selected = topics.find((topic) => topic.id === value);
                    if (selected?.locale) {
                      setTargetLocale(selected.locale);
                    }
                    commandShapeChanged();
                  }}
                >
                  <option value=''>{copy.choose}</option>
                  {topics
                    .filter((topic) => topic.replyCount >= 1)
                    .map((topic) => (
                      <option key={topic.id} value={topic.id}>
                        {topic.title} · {topic.replyCount} replies
                      </option>
                    ))}
                </select>
              </label>

              <section className='space-y-3'>
                <h2 className='text-sm font-semibold'>{copy.root}</h2>
                {loadingReplies ? (
                  <div className='rounded-lg border p-5 text-sm text-muted-foreground'>
                    {copy.loadingReplies}
                  </div>
                ) : replies?.items.length ? (
                  <div className='max-h-96 space-y-2 overflow-y-auto rounded-lg border p-3'>
                    {replies.items.map((reply) => (
                      <label
                        className='flex items-start gap-3 rounded-md px-3 py-2 text-sm hover:bg-muted/50'
                        key={reply.id}
                      >
                        <input
                          className='mt-1'
                          type='radio'
                          name='forum-topic-fork-root'
                          checked={rootReplyId === reply.id}
                          onChange={() => {
                            setRootReplyId(reply.id);
                            commandShapeChanged();
                          }}
                        />
                        <span>
                          <span className='block'>{forumTopicForkReplyLabel(reply)}</span>
                          <span className='mt-1 block break-all font-mono text-[11px] text-muted-foreground'>
                            {reply.id}
                          </span>
                        </span>
                      </label>
                    ))}
                  </div>
                ) : (
                  <div className='rounded-lg border border-dashed p-5 text-sm text-muted-foreground'>
                    {copy.noReplies}
                  </div>
                )}
              </section>

              <div className='grid gap-5 md:grid-cols-2'>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.targetLocale}</span>
                  <input
                    className='w-full rounded-md border bg-background px-3 py-2'
                    maxLength={64}
                    value={targetLocale}
                    onChange={(event) => {
                      setTargetLocale(event.target.value);
                      commandShapeChanged();
                    }}
                  />
                </label>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.targetSlug}</span>
                  <input
                    className='w-full rounded-md border bg-background px-3 py-2'
                    maxLength={255}
                    value={targetSlug}
                    onChange={(event) => {
                      setTargetSlug(event.target.value);
                      commandShapeChanged();
                    }}
                  />
                </label>
              </div>

              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.targetTitle}</span>
                <input
                  className='w-full rounded-md border bg-background px-3 py-2'
                  maxLength={500}
                  value={targetTitle}
                  onChange={(event) => {
                    setTargetTitle(event.target.value);
                    commandShapeChanged();
                  }}
                />
              </label>

              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.reason}</span>
                <textarea
                  className='min-h-28 w-full rounded-md border bg-background px-3 py-2'
                  maxLength={500}
                  value={reason}
                  onChange={(event) => {
                    setReason(event.target.value);
                    commandShapeChanged();
                  }}
                />
              </label>

              <p className='rounded-lg border border-amber-500/20 bg-amber-500/5 px-4 py-3 text-sm text-muted-foreground'>
                {copy.warning}
              </p>
            </div>

            <aside className='rounded-xl border bg-muted/20 p-5 xl:sticky xl:top-6 xl:self-start'>
              <p className='text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                {copy.retryIdentity}
              </p>
              <p className='mt-3 break-all font-mono text-xs'>{identity.operationId}</p>
              <p className='mt-5 text-xs font-semibold uppercase tracking-wider text-muted-foreground'>
                {copy.targetIdentity}
              </p>
              <p className='mt-3 break-all font-mono text-xs'>{identity.targetTopicId}</p>
              <p className='mt-4 text-xs leading-5 text-muted-foreground'>
                {copy.retryHint}
              </p>
              <Button className='mt-6 w-full' type='submit' disabled={pending}>
                {pending ? copy.pending : copy.submit}
              </Button>
            </aside>
          </form>
        </CardContent>
      </Card>

      {receipt ? (
        <Card className='border-emerald-500/30 bg-emerald-500/5'>
          <CardHeader>
            <CardTitle>{copy.success}</CardTitle>
          </CardHeader>
          <CardContent className='grid gap-3 text-sm sm:grid-cols-2'>
            <div>
              <p className='font-medium'>Operation</p>
              <p className='break-all font-mono text-xs text-muted-foreground'>
                {receipt.operationId}
              </p>
            </div>
            <div>
              <p className='font-medium'>Target</p>
              <p className='break-all font-mono text-xs text-muted-foreground'>
                {receipt.targetTopicId}
              </p>
            </div>
            <div>
              <p className='font-medium'>{copy.copiedReplies}</p>
              <p>{receipt.copiedReplyCount}</p>
            </div>
            <div>
              <p className='font-medium'>{copy.copiedPublishedReplies}</p>
              <p>{receipt.copiedPublishedReplyCount}</p>
            </div>
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
