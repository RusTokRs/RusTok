'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useLocale } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useMemo, useState, type FormEvent } from 'react';
import { toast } from 'sonner';
import {
  renameForumTopicSlug,
  type GqlOpts,
  type ForumTopicSummary
} from '../api/forum';
import {
  buildForumTopicSlugRenameCommand,
  forumTopicSlugRenameCandidateLabel,
  type ForumTopicSlugRenameReceipt
} from '../core/topic-slug-rename';
import en from '../locales/en.json';
import ru from '../locales/ru.json';

type ClientGqlOpts = Pick<GqlOpts, 'tenantId' | 'tenantSlug'>;

export function ForumTopicSlugRename({
  topics,
  gqlOpts = {}
}: {
  topics: ForumTopicSummary[];
  gqlOpts?: ClientGqlOpts;
}) {
  const locale = useLocale();
  const copy = locale.toLowerCase().startsWith('ru') ? ru : en;
  const router = useRouter();
  const [topicId, setTopicId] = useState('');
  const [slug, setSlug] = useState('');
  const [pending, setPending] = useState(false);
  const [receipt, setReceipt] = useState<ForumTopicSlugRenameReceipt | null>(
    null
  );

  const candidate = useMemo(
    () => topics.find((topic) => topic.id === topicId),
    [topicId, topics]
  );

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!candidate) {
      toast.error(copy.renameErrorTopic);
      return;
    }

    try {
      const command = buildForumTopicSlugRenameCommand({ candidate, slug });
      setPending(true);
      const result = await renameForumTopicSlug(command, gqlOpts);
      setReceipt(result);
      setSlug(result.slug);
      toast.success(result.changed ? copy.renameSuccess : copy.renameReplay);
      router.refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : copy.renameFailure);
    } finally {
      setPending(false);
    }
  }

  return (
    <div className='space-y-6'>
      <Card>
        <CardHeader>
          <CardTitle>{copy.renameTitle}</CardTitle>
          <p className='text-muted-foreground text-sm'>{copy.renameSubtitle}</p>
        </CardHeader>
        <CardContent>
          <form
            className='grid gap-6 xl:grid-cols-[minmax(0,1fr)_22rem]'
            onSubmit={submit}
          >
            <div className='space-y-5'>
              <label className='space-y-2 text-sm font-medium'>
                <span className='block'>{copy.renameTopic}</span>
                <select
                  className='bg-background w-full rounded-md border px-3 py-2'
                  value={topicId}
                  onChange={(event) => {
                    const selected = topics.find(
                      (topic) => topic.id === event.target.value
                    );
                    setTopicId(event.target.value);
                    setSlug(selected?.slug ?? '');
                    setReceipt(null);
                  }}
                >
                  <option value=''>{copy.renameChoose}</option>
                  {topics.map((topic) => (
                    <option key={topic.id} value={topic.id}>
                      {forumTopicSlugRenameCandidateLabel(topic)}
                    </option>
                  ))}
                </select>
              </label>

              <label className='block space-y-2 text-sm font-medium'>
                <span className='block'>{copy.renameSlug}</span>
                <input
                  className='bg-background w-full rounded-md border px-3 py-2'
                  maxLength={255}
                  value={slug}
                  onChange={(event) => {
                    setSlug(event.target.value);
                    setReceipt(null);
                  }}
                />
                <span className='text-muted-foreground block text-xs leading-5 font-normal'>
                  {copy.renameSlugHint}
                </span>
              </label>
            </div>

            <aside className='bg-muted/20 rounded-xl border p-5 xl:sticky xl:top-6 xl:self-start'>
              <p className='text-muted-foreground text-xs leading-5'>
                {copy.renameWarning}
              </p>
              <Button className='mt-6 w-full' type='submit' disabled={pending}>
                {pending ? copy.renamePending : copy.renameSubmit}
              </Button>
            </aside>
          </form>
        </CardContent>
      </Card>

      {receipt ? (
        <Card className='border-emerald-500/30 bg-emerald-500/5'>
          <CardHeader>
            <CardTitle>
              {receipt.changed ? copy.renameSuccess : copy.renameReplay}
            </CardTitle>
          </CardHeader>
          <CardContent className='grid gap-3 text-sm sm:grid-cols-2'>
            <div>
              <p className='font-medium'>{copy.renamePreviousPath}</p>
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {receipt.previousPath}
              </p>
            </div>
            <div>
              <p className='font-medium'>{copy.renameCanonicalPath}</p>
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {receipt.canonical.path}
              </p>
            </div>
            <div>
              <p className='font-medium'>{copy.renameLocale}</p>
              <p>{receipt.locale}</p>
            </div>
            <div>
              <p className='font-medium'>{copy.renameAlias}</p>
              <p className='text-muted-foreground font-mono text-xs break-all'>
                {receipt.aliasId ?? '—'}
              </p>
            </div>
          </CardContent>
        </Card>
      ) : null}
    </div>
  );
}
