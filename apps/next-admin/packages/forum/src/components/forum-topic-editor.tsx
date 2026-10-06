'use client';

import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { FormInput, FormSelect } from '@/shared/ui/forms';
import { RichTextEditor } from '@/shared/ui/rich-text-editor';
import { Form } from '@/shared/ui/shadcn/form';
import {
  emptyRichTextDocument,
  getRichTextProfile,
  richTextDocumentHasText,
  validateRichTextDocument,
  type RichTextDocument
} from '@rustok/richtext';
import { zodResolver } from '@hookform/resolvers/zod';
import { useLocale } from '@rustok/next-fluent';
import { useRouter } from 'next/navigation';
import { useMemo, useState } from 'react';
import { useForm, type Resolver } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import { cn } from '@/shared/lib/utils';
import {
  closeForumTopic,
  createForumTopic,
  deleteForumTopic,
  lockForumTopic,
  pinForumTopic,
  reopenForumTopic,
  restoreForumTopic,
  updateForumTopic,
  type ForumCategoryOption,
  type ForumTopicDetail,
  type GqlOpts
} from '../api/forum';

const formSchema = z.object({
  locale: z.string().min(2, 'Locale is required.'),
  categoryId: z.string().min(1, 'Category is required.'),
  title: z.string().min(2, 'Title must be at least 2 characters.'),
  slug: z.string().optional(),
  tags: z.string().optional()
});

type FormValues = z.infer<typeof formSchema>;

export function ForumTopicEditor({
  initialData,
  categories,
  gqlOpts = {}
}: {
  initialData: ForumTopicDetail | null;
  categories: ForumCategoryOption[];
  gqlOpts?: GqlOpts;
}) {
  const router = useRouter();
  const hostLocale = useLocale();
  const defaultLocale =
    initialData?.requestedLocale ?? initialData?.effectiveLocale ?? hostLocale;
  const initialDocument = useMemo(
    () => initialData?.body.document ?? emptyRichTextDocument(),
    [initialData]
  );
  const [body, setBody] = useState<RichTextDocument>(initialDocument);
  const [isPinned, setIsPinned] = useState(initialData?.isPinned ?? false);
  const [isLocked, setIsLocked] = useState(initialData?.isLocked ?? false);
  const [topicStatus, setTopicStatus] = useState(initialData?.status ?? 'open');
  const [isDeleted, setIsDeleted] = useState(initialData?.isDeleted ?? false);
  const [moderationBusy, setModerationBusy] = useState(false);

  const form = useForm<FormValues>({
    resolver: zodResolver(formSchema) as Resolver<FormValues>,
    defaultValues: {
      locale: defaultLocale,
      categoryId: initialData?.categoryId ?? categories[0]?.id ?? '',
      title: initialData?.title ?? '',
      slug: initialData?.slug ?? '',
      tags: initialData?.tags.join(', ') ?? ''
    }
  });
  const contentLocale = form.watch('locale');
  const isEditing = initialData !== null;

  async function handleTogglePin() {
    if (!initialData) return;
    setModerationBusy(true);
    try {
      const nextPinned = !isPinned;
      await pinForumTopic(initialData.id, nextPinned, gqlOpts);
      setIsPinned(nextPinned);
      toast.success(nextPinned ? 'Topic pinned' : 'Topic unpinned');
      router.refresh();
    } catch {
      toast.error('Failed to update pin status');
    } finally {
      setModerationBusy(false);
    }
  }

  async function handleToggleLock() {
    if (!initialData) return;
    setModerationBusy(true);
    try {
      const nextLocked = !isLocked;
      await lockForumTopic(initialData.id, nextLocked, gqlOpts);
      setIsLocked(nextLocked);
      toast.success(nextLocked ? 'Topic locked' : 'Topic unlocked');
      router.refresh();
    } catch {
      toast.error('Failed to update lock status');
    } finally {
      setModerationBusy(false);
    }
  }

  async function handleToggleClose() {
    if (!initialData) return;
    setModerationBusy(true);
    try {
      if (topicStatus === 'closed') {
        await reopenForumTopic(initialData.id, gqlOpts);
        setTopicStatus('open');
        toast.success('Topic reopened');
      } else {
        await closeForumTopic(initialData.id, gqlOpts);
        setTopicStatus('closed');
        toast.success('Topic closed');
      }
      router.refresh();
    } catch {
      toast.error('Failed to update topic status');
    } finally {
      setModerationBusy(false);
    }
  }

  async function handleToggleDelete() {
    if (!initialData) return;
    setModerationBusy(true);
    try {
      if (isDeleted) {
        await restoreForumTopic(initialData.id, gqlOpts);
        setIsDeleted(false);
        setTopicStatus('open');
        toast.success('Topic restored');
      } else {
        await deleteForumTopic(initialData.id, gqlOpts);
        setIsDeleted(true);
        toast.success('Topic deleted');
      }
      router.refresh();
    } catch {
      toast.error('Failed to update topic lifecycle');
    } finally {
      setModerationBusy(false);
    }
  }

  async function submit(values: FormValues) {
    const validation = validateRichTextDocument(
      body,
      getRichTextProfile('discussion')
    );
    if (!validation.valid || !richTextDocumentHasText(body)) {
      toast.error(validation.error ?? 'Topic body is required.');
      return;
    }

    const tags = values.tags
      ? values.tags
          .split(',')
          .map((tag) => tag.trim())
          .filter(Boolean)
      : [];

    try {
      const topic = initialData
        ? await updateForumTopic(
            initialData.id,
            {
              locale: values.locale,
              title: values.title,
              body,
              tags
            },
            gqlOpts
          )
        : await createForumTopic(
            {
              locale: values.locale,
              categoryId: values.categoryId,
              title: values.title,
              slug: values.slug || undefined,
              body,
              tags
            },
            gqlOpts
          );

      toast.success(initialData ? 'Topic updated' : 'Topic created');
      router.push(`/dashboard/forum/topic?topic_id=${topic.id}`);
      router.refresh();
    } catch {
      toast.error('Failed to save topic');
    }
  }

  return (
    <Card className='mx-auto w-full'>
      <CardHeader className='space-y-4'>
        <div className='flex flex-wrap items-center justify-between gap-3'>
          <CardTitle>
            {isEditing ? 'Edit forum topic' : 'Create forum topic'}
          </CardTitle>
          {isEditing && (
            <div className='flex flex-wrap items-center gap-2'>
              <span
                className={cn(
                  'rounded-full px-2.5 py-0.5 text-xs font-medium',
                  isPinned
                    ? 'bg-amber-500/15 text-amber-700 dark:text-amber-300'
                    : 'border text-muted-foreground'
                )}
              >
                {isPinned ? 'Pinned' : 'Normal'}
              </span>
              <span
                className={cn(
                  'rounded-full px-2.5 py-0.5 text-xs font-medium',
                  isLocked
                    ? 'bg-destructive/15 text-destructive'
                    : 'border text-muted-foreground'
                )}
              >
                {isLocked ? 'Locked' : 'Unlocked'}
              </span>
              <span
                className={cn(
                  'rounded-full px-2.5 py-0.5 text-xs font-medium',
                  isDeleted
                    ? 'bg-destructive/15 text-destructive'
                    : topicStatus === 'open'
                      ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300'
                      : topicStatus === 'closed'
                        ? 'bg-muted text-muted-foreground'
                        : 'bg-destructive/15 text-destructive'
                )}
              >
                {isDeleted ? 'DELETED' : topicStatus.toUpperCase()}
              </span>
            </div>
          )}
        </div>

        {isEditing && (
          <div className='flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-muted/40 p-3'>
            <span className='text-xs font-medium text-muted-foreground uppercase tracking-wider'>
              Moderator Actions
            </span>
            <div className='flex flex-wrap items-center gap-2'>
              <Button
                type='button'
                variant='outline'
                size='sm'
                disabled={moderationBusy}
                onClick={handleTogglePin}
              >
                {isPinned ? 'Unpin topic' : 'Pin topic'}
              </Button>
              <Button
                type='button'
                variant='outline'
                size='sm'
                disabled={moderationBusy}
                onClick={handleToggleLock}
              >
                {isLocked ? 'Unlock topic' : 'Lock topic'}
              </Button>
              <Button
                type='button'
                variant='outline'
                size='sm'
                disabled={moderationBusy}
                onClick={handleToggleClose}
              >
                {topicStatus === 'closed' ? 'Reopen topic' : 'Close topic'}
              </Button>
              <Button
                type='button'
                variant={isDeleted ? 'outline' : 'destructive'}
                size='sm'
                disabled={moderationBusy}
                onClick={handleToggleDelete}
              >
                {isDeleted ? 'Restore topic' : 'Delete topic'}
              </Button>
            </div>
          </div>
        )}
      </CardHeader>
      <CardContent>
        <Form
          form={form}
          onSubmit={form.handleSubmit(submit)}
          className='space-y-6'
        >
          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput
              control={form.control}
              name='locale'
              label='Content locale'
              dir='ltr'
              required
            />
            <FormSelect
              control={form.control}
              name='categoryId'
              label='Category'
              required
              disabled={isEditing || form.formState.isSubmitting}
              options={categories.map((category) => ({
                value: category.id,
                label: category.name,
                lang: category.effectiveLocale,
                dir: 'auto' as const
              }))}
              placeholder='Select a category'
              description={
                isEditing
                  ? 'Move operations are separate from translation editing.'
                  : undefined
              }
            />
          </div>

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput
              control={form.control}
              name='title'
              label='Title'
              lang={contentLocale}
              dir='auto'
              required
            />
            <FormInput
              control={form.control}
              name='slug'
              label='Slug'
              dir='ltr'
              placeholder='Generated from the title when empty'
              disabled={isEditing || form.formState.isSubmitting}
              description={
                isEditing
                  ? 'Use the dedicated topic route rename operation.'
                  : undefined
              }
            />
          </div>

          <FormInput
            control={form.control}
            name='tags'
            label='Tags'
            lang={contentLocale}
            dir='auto'
            placeholder='rust, help, discussion'
          />

          <RichTextEditor
            label='Topic body'
            profile='discussion'
            value={body}
            contentLocale={contentLocale}
            disabled={form.formState.isSubmitting}
            onChange={setBody}
          />

          <Button
            type='submit'
            disabled={
              form.formState.isSubmitting ||
              (!isEditing && categories.length === 0)
            }
          >
            {isEditing ? 'Update topic' : 'Create topic'}
          </Button>
        </Form>
      </CardContent>
    </Card>
  );
}
