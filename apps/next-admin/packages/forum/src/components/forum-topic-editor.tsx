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
import { useLocale } from 'next-intl';
import { useRouter } from 'next/navigation';
import { useMemo, useState } from 'react';
import { useForm, type Resolver } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import {
  createForumTopic,
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
      <CardHeader>
        <CardTitle>
          {isEditing ? 'Edit forum topic' : 'Create forum topic'}
        </CardTitle>
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
