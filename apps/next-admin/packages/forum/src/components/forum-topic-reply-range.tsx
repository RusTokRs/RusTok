'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useLocale } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useMemo, useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import { moveForumTopicReplyRange } from '../api/topic-reply-range';
import type { ForumTopicSummary, GqlOpts } from '../api/forum';
import {
  buildForumReplyRangeMoveCommand,
  newForumReplyRangeMoveIdentity,
  type ForumReplyRangeMoveIdentity,
  type ForumReplyRangeMoveReceipt
} from '../core/topic-reply-range';
import en from './reply-range/locales/en.json';
import ru from './reply-range/locales/ru.json';

type ClientGqlOpts = Pick<GqlOpts, 'tenantId' | 'tenantSlug'>;

export function ForumTopicReplyRange({
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
  const [targetTopicId, setTargetTopicId] = useState('');
  const [startPosition, setStartPosition] = useState('1');
  const [endPosition, setEndPosition] = useState('1');
  const [reason, setReason] = useState('');
  const [identity, setIdentity] = useState<ForumReplyRangeMoveIdentity>(
    newForumReplyRangeMoveIdentity
  );
  const [pending, setPending] = useState(false);
  const [receipt, setReceipt] = useState<ForumReplyRangeMoveReceipt | null>(
    null
  );

  const orderedTopics = useMemo(
    () =>
      [...topics].sort(
        (left, right) =>
          left.title.localeCompare(right.title) ||
          left.id.localeCompare(right.id)
      ),
    [topics]
  );

  function commandShapeChanged() {
    setIdentity(newForumReplyRangeMoveIdentity());
    setReceipt(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const command = buildForumReplyRangeMoveCommand({
        identity,
        sourceTopicId,
        targetTopicId,
        startPosition: Number(startPosition),
        endPosition: Number(endPosition),
        reason
      });
      setPending(true);
      const result = await moveForumTopicReplyRange(command, gqlOpts);
      setReceipt(result);
      toast.success(copy.success);
      setSourceTopicId('');
      setTargetTopicId('');
      setStartPosition('1');
      setEndPosition('1');
      setReason('');
      setIdentity(newForumReplyRangeMoveIdentity());
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
          <p className='text-muted-foreground text-sm'>{copy.subtitle}</p>
        </CardHeader>
        <CardContent>
          <form
            className='grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]'
            onSubmit={submit}
          >
            <div className='space-y-6'>
              <div className='grid gap-5 md:grid-cols-2'>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.source}</span>
                  <select
                    className='bg-background w-full rounded-md border px-3 py-2'
                    value={sourceTopicId}
                    onChange={(event) => {
                      const value = event.target.value;
                      setSourceTopicId(value);
                      if (targetTopicId === value) {
                        setTargetTopicId('');
                      }
                      commandShapeChanged();
                    }}
                  >
                    <option value=''>{copy.choose}</option>
                    {orderedTopics.map((topic) => (
                      <option key={topic.id} value={topic.id}>
                        {topic.title} · {topic.replyCount} replies
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
                      commandShapeChanged();
                    }}
                  >
                    <option value=''>{copy.choose}</option>
                    {orderedTopics
                      .filter((topic) => topic.id !== sourceTopicId)
                      .map((topic) => (
                        <option key={topic.id} value={topic.id}>
                          {topic.title} · {topic.replyCount} replies
                        </option>
                      ))}
                  </select>
                </label>
              </div>

              <div className='grid gap-5 md:grid-cols-2'>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.startPosition}</span>
                  <input
                    className='bg-background w-full rounded-md border px-3 py-2'
                    type='number'
                    min={1}
                    step={1}
                    value={startPosition}
                    onChange={(event) => {
                      setStartPosition(event.target.value);
                      commandShapeChanged();
                    }}
                  />
                </label>
                <label className='space-y-2 text-sm font-medium'>
                  <span className='block'>{copy.endPosition}</span>
                  <input
                    className='bg-background w-full rounded-md border px-3 py-2'
                    type='number'
                    min={1}
                    step={1}
                    value={endPosition}
                    onChange={(event) => {
                      setEndPosition(event.target.value);
                      commandShapeChanged();
                    }}
                  />
                </label>
              </div>

              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.reason}</span>
                <textarea
                  className='bg-background min-h-28 w-full rounded-md border px-3 py-2'
                  maxLength={500}
                  value={reason}
                  onChange={(event) => {
                    setReason(event.target.value);
                    commandShapeChanged();
                  }}
                />
              </label>

              <p className='text-muted-foreground rounded-lg border border-amber-500/20 bg-amber-500/5 px-4 py-3 text-sm'>
                {copy.warning}
              </p>
            </div>

            <aside className='bg-muted/20 rounded-xl border p-5 xl:sticky xl:top-6 xl:self-start'>
              <p className='text-muted-foreground text-xs font-semibold tracking-wider uppercase'>
                {copy.retryIdentity}
              </p>
              <p className='mt-3 font-mono text-xs break-all'>
                {identity.operationId}
              </p>
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
            <CardTitle>{copy.receipt}</CardTitle>
          </CardHeader>
          <CardContent>
            <dl className='grid gap-4 text-sm md:grid-cols-2 xl:grid-cols-3'>
              <ReceiptValue
                label={copy.operation}
                value={receipt.operationId}
                mono
              />
              <ReceiptValue
                label={copy.sourceRange}
                value={`${receipt.sourceStartPosition}–${receipt.sourceEndPosition}`}
              />
              <ReceiptValue
                label={copy.targetRange}
                value={`${receipt.targetStartPosition}–${receipt.targetEndPosition}`}
              />
              <ReceiptValue
                label={copy.movedReplies}
                value={String(receipt.movedReplyCount)}
              />
              <ReceiptValue
                label={copy.publishedReplies}
                value={String(receipt.movedPublishedReplyCount)}
              />
              <ReceiptValue label='Event' value={receipt.eventId} mono />
            </dl>
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}

function ReceiptValue({
  label,
  value,
  mono = false
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div>
      <dt className='font-medium'>{label}</dt>
      <dd
        className={`text-muted-foreground mt-1 break-all ${
          mono ? 'font-mono text-xs' : ''
        }`}
      >
        {value}
      </dd>
    </div>
  );
}
