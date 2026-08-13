'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useLocale } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useMemo, useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import {
  mergeForumTopics,
  type GqlOpts,
  type ForumTopicSummary
} from '../api/forum';
import {
  buildForumTopicMergeCommand,
  forumTopicMergeCandidateLabel,
  forumTopicMergeNeedsSolutionChoice,
  newForumTopicMergeOperationId,
  type ForumTopicMergeReceipt,
  type ForumTopicMergeWinner
} from '../core/topic-merge';
import en from '../locales/en.json';
import ru from '../locales/ru.json';

type ClientGqlOpts = Pick<GqlOpts, 'tenantId' | 'tenantSlug'>;

export function ForumTopicMerge({
  topics,
  gqlOpts = {}
}: {
  topics: ForumTopicSummary[];
  gqlOpts?: ClientGqlOpts;
}) {
  const locale = useLocale();
  const copy = locale.toLowerCase().startsWith('ru') ? ru : en;
  const router = useRouter();
  const [sourceTopicId, setSourceTopicId] = useState('');
  const [targetTopicId, setTargetTopicId] = useState('');
  const [reason, setReason] = useState('');
  const [winner, setWinner] = useState<ForumTopicMergeWinner | undefined>();
  const [operationId, setOperationId] = useState(newForumTopicMergeOperationId);
  const [pending, setPending] = useState(false);
  const [receipt, setReceipt] = useState<ForumTopicMergeReceipt | null>(null);

  const source = useMemo(
    () => topics.find((topic) => topic.id === sourceTopicId),
    [sourceTopicId, topics]
  );
  const target = useMemo(
    () => topics.find((topic) => topic.id === targetTopicId),
    [targetTopicId, topics]
  );
  const needsWinner = Boolean(
    source && target && forumTopicMergeNeedsSolutionChoice(source, target)
  );

  function commandShapeChanged() {
    setOperationId(newForumTopicMergeOperationId());
    setReceipt(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!source || !target) {
      toast.error(!source ? copy.source : copy.target);
      return;
    }
    try {
      const command = buildForumTopicMergeCommand({
        operationId,
        source,
        target,
        reason,
        winner
      });
      setPending(true);
      const result = await mergeForumTopics(command, gqlOpts);
      setReceipt(result);
      toast.success(copy.success);
      setSourceTopicId('');
      setTargetTopicId('');
      setReason('');
      setWinner(undefined);
      setOperationId(newForumTopicMergeOperationId());
      router.refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : copy.failure);
    } finally {
      setPending(false);
    }
  }

  if (topics.length < 2) {
    return (
      <Card>
        <CardHeader>
          <CardTitle>{copy.title}</CardTitle>
        </CardHeader>
        <CardContent className='text-muted-foreground text-sm'>
          {copy.notEnough}
        </CardContent>
      </Card>
    );
  }

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{copy.title}</CardTitle>
          <p className='text-muted-foreground text-sm'>{copy.subtitle}</p>
        </CardHeader>
        <CardContent>
          <form
            className='grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]'
            onSubmit={submit}
          >
            <div className='space-y-5'>
              <div className='grid gap-5 md:grid-cols-2'>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.source}</span>
                  <select
                    className='bg-background w-full rounded-md border px-3 py-2'
                    value={sourceTopicId}
                    onChange={(event) => {
                      setSourceTopicId(event.target.value);
                      setWinner(undefined);
                      commandShapeChanged();
                    }}
                  >
                    <option value=''>{copy.choose}</option>
                    {topics
                      .filter((topic) => topic.id !== targetTopicId)
                      .map((topic) => (
                        <option key={topic.id} value={topic.id}>
                          {forumTopicMergeCandidateLabel(topic)}
                        </option>
                      ))}
                  </select>
                </label>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.target}</span>
                  <select
                    className='bg-background w-full rounded-md border px-3 py-2'
                    value={targetTopicId}
                    onChange={(event) => {
                      setTargetTopicId(event.target.value);
                      setWinner(undefined);
                      commandShapeChanged();
                    }}
                  >
                    <option value=''>{copy.choose}</option>
                    {topics
                      .filter((topic) => topic.id !== sourceTopicId)
                      .map((topic) => (
                        <option key={topic.id} value={topic.id}>
                          {forumTopicMergeCandidateLabel(topic)}
                        </option>
                      ))}
                  </select>
                </label>
              </div>

              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.reason}</span>
                <textarea
                  className='bg-background min-h-28 w-full rounded-md border px-3 py-2'
                  maxLength={500}
                  placeholder={copy.reasonPlaceholder}
                  value={reason}
                  onChange={(event) => {
                    setReason(event.target.value);
                    commandShapeChanged();
                  }}
                />
              </label>

              {needsWinner ? (
                <fieldset className='space-y-3 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4'>
                  <legend className='px-1 text-sm font-semibold'>
                    {copy.winner}
                  </legend>
                  <label className='flex items-center gap-3 text-sm'>
                    <input
                      type='radio'
                      name='solution-winner'
                      checked={winner === 'source'}
                      onChange={() => {
                        setWinner('source');
                        commandShapeChanged();
                      }}
                    />
                    {copy.sourceWinner}
                  </label>
                  <label className='flex items-center gap-3 text-sm'>
                    <input
                      type='radio'
                      name='solution-winner'
                      checked={winner === 'target'}
                      onChange={() => {
                        setWinner('target');
                        commandShapeChanged();
                      }}
                    />
                    {copy.targetWinner}
                  </label>
                </fieldset>
              ) : null}

              <p className='border-destructive/20 bg-destructive/5 text-muted-foreground rounded-lg border px-4 py-3 text-sm'>
                {copy.warning}
              </p>
            </div>

            <aside className='bg-muted/20 rounded-xl border p-5 xl:sticky xl:top-6 xl:self-start'>
              <p className='text-muted-foreground text-xs font-semibold tracking-wider uppercase'>
                {copy.retryIdentity}
              </p>
              <p className='mt-3 font-mono text-xs break-all'>{operationId}</p>
              <p className='text-muted-foreground mt-4 text-xs leading-5'>
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
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {receipt.operationId}
              </p>
            </div>
            <div>
              <p className='font-medium'>Event</p>
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {receipt.eventId}
              </p>
            </div>
            <div>
              <p className='font-medium'>{copy.movedReplies}</p>
              <p>{receipt.movedReplyCount}</p>
            </div>
            <div>
              <p className='font-medium'>{copy.resultingReplies}</p>
              <p>{receipt.resultingPublishedReplyCount}</p>
            </div>
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
