'use client';

import { FormInput, FormTextarea, FormSwitch, FormSelect } from '@/shared/ui/forms';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Form } from '@/components/ui/form';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from 'next/navigation';
import { useMemo, useState } from 'react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import type { PostResponse, GqlOpts } from '../api/posts';
import { createPost, updatePost } from '../api/posts';
import { RtJsonEditor } from './rt-json-editor';
import { markdownToRtDoc, stringifyRtDoc, parseRtDoc, type RtDoc } from './rt-json-format';

const formSchema = z
  .object({
    title: z.string().min(2, 'Title must be at least 2 characters.'),
    slug: z.string().optional(),
    locale: z.string().min(2).default('en'),
    bodyFormat: z.enum(['markdown', 'rt_json_v1']).default('markdown'),
    body: z.string().default(''),
    contentJson: z.string().optional(),
    excerpt: z.string().optional(),
    tags: z.string().optional(),
    featuredImageUrl: z.string().url().optional().or(z.literal('')),
    seoTitle: z.string().optional(),
    seoDescription: z.string().optional(),
    publish: z.boolean().default(false)
  })
  .superRefine((values, ctx) => {
    if (values.bodyFormat === 'markdown' && !values.body.trim()) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['body'],
        message: 'Body is required for markdown format.'
      });
    }
    if (values.bodyFormat === 'rt_json_v1' && !values.contentJson?.trim()) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['contentJson'],
        message: 'content_json is required for rt_json_v1 format.'
      });
    }
    if (values.bodyFormat === 'rt_json_v1' && values.contentJson?.trim()) {
      try {
        JSON.parse(values.contentJson);
      } catch {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ['contentJson'],
          message: 'content_json must be valid JSON.'
        });
      }
    }
  });

type FormValues = z.infer<typeof formSchema>;

function resolveInitialDoc(initialData: PostResponse | null): RtDoc {
  if (initialData?.contentJson) {
    try {
      return parseRtDoc(initialData.contentJson);
    } catch {
      // fallthrough to markdown
    }
  }

  if (initialData?.body?.trim()) {
    return markdownToRtDoc(initialData.body);
  }

  return { type: 'doc', content: [] };
}

export default function PostForm({
  initialData,
  pageTitle,
  gqlOpts = {}
}: {
  initialData: PostResponse | null;
  pageTitle: string;
  gqlOpts?: GqlOpts;
}) {
  const router = useRouter();
  const initialDoc = useMemo(() => resolveInitialDoc(initialData), [initialData]);
  const [rtDoc, setRtDoc] = useState<RtDoc>(initialDoc);
  const [migrationWarnings, setMigrationWarnings] = useState<string[]>(
    initialData?.body?.trim() && !initialData?.contentJson
      ? ['Legacy markdown detected. Convert it to rt_json_v1 for rich editor features.']
      : []
  );

  const defaultValues: FormValues = {
    title: initialData?.title ?? '',
    slug: initialData?.slug ?? '',
    locale: 'en',
    bodyFormat: initialData?.contentJson ? 'rt_json_v1' : 'markdown',
    body: initialData?.body ?? '',
    contentJson: initialData?.contentJson ? JSON.stringify(initialData.contentJson, null, 2) : '',
    excerpt: initialData?.excerpt ?? '',
    tags: initialData?.tags?.join(', ') ?? '',
    featuredImageUrl: initialData?.featuredImageUrl ?? '',
    seoTitle: initialData?.seoTitle ?? '',
    seoDescription: initialData?.seoDescription ?? '',
    publish: false
  };

  const form = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues
  });

  function convertMarkdownToRtJson() {
    const markdown = form.getValues('body');
    if (!markdown.trim()) {
      toast.error('Markdown body is empty.');
      return;
    }
    const converted = markdownToRtDoc(markdown);
    setRtDoc(converted);
    form.setValue('contentJson', stringifyRtDoc(converted), { shouldValidate: true });
    form.setValue('bodyFormat', 'rt_json_v1', { shouldValidate: true });
    const warnings = markdown.includes('```')
      ? ['Code blocks were migrated as plain text paragraphs.']
      : [];
    setMigrationWarnings(warnings);
    toast.success('Markdown converted to rt_json_v1 editor document.');
  }

  async function onSubmit(values: FormValues) {
    const tags = values.tags
      ? values.tags.split(',').map((t) => t.trim()).filter(Boolean)
      : [];

    const contentJson = values.bodyFormat === 'rt_json_v1' && values.contentJson
      ? JSON.parse(values.contentJson)
      : undefined;

    try {
      if (initialData) {
        await updatePost(initialData.id, {
          title: values.title,
          slug: values.slug || undefined,
          locale: values.locale,
          body: values.body,
          bodyFormat: values.bodyFormat,
          contentJson,
          excerpt: values.excerpt || undefined,
          tags,
          featuredImageUrl: values.featuredImageUrl || undefined,
          seoTitle: values.seoTitle || undefined,
          seoDescription: values.seoDescription || undefined
        }, gqlOpts);
        toast.success('Post updated');
      } else {
        await createPost({
          title: values.title,
          slug: values.slug || undefined,
          locale: values.locale,
          body: values.body,
          bodyFormat: values.bodyFormat,
          contentJson,
          excerpt: values.excerpt || undefined,
          publish: values.publish,
          tags,
          featuredImageUrl: values.featuredImageUrl || undefined,
          seoTitle: values.seoTitle || undefined,
          seoDescription: values.seoDescription || undefined
        }, gqlOpts);
        toast.success('Post created');
      }
      router.push('/dashboard/blog');
      router.refresh();
    } catch {
      toast.error('Failed to save post');
    }
  }

  return (
    <Card className='mx-auto w-full'>
      <CardHeader>
        <CardTitle className='text-left text-2xl font-bold'>
          {pageTitle}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <Form
          form={form}
          onSubmit={form.handleSubmit(onSubmit)}
          className='space-y-8'
        >
          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput control={form.control} name='title' label='Title' placeholder='Enter post title' required />
            <FormInput control={form.control} name='slug' label='Slug' placeholder='auto-generated-if-empty' />
          </div>

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput control={form.control} name='locale' label='Locale' placeholder='en' required />
            <FormInput control={form.control} name='tags' label='Tags' placeholder='rust, blog, news' />
          </div>

          <FormSelect
            control={form.control}
            name='bodyFormat'
            label='Body format'
            options={[
              { label: 'Markdown (legacy)', value: 'markdown' },
              { label: 'RT JSON v1 (rich editor)', value: 'rt_json_v1' }
            ]}
          />

          {form.watch('bodyFormat') === 'markdown' ? (
            <>
              <FormTextarea control={form.control} name='body' label='Body' placeholder='Write your post content...' required config={{ rows: 12 }} />
              <Button type='button' variant='outline' onClick={convertMarkdownToRtJson}>
                Convert markdown to rt_json_v1
              </Button>
            </>
          ) : (
            <RtJsonEditor
              label='Body (rt_json_v1)'
              value={rtDoc}
              onChange={(doc) => {
                setRtDoc(doc);
                form.setValue('contentJson', stringifyRtDoc(doc), { shouldValidate: true });
              }}
            />
          )}

          {migrationWarnings.length > 0 && (
            <Alert>
              <AlertTitle>Legacy content warning</AlertTitle>
              <AlertDescription>
                <ul className='list-disc pl-4'>
                  {migrationWarnings.map((warning) => (
                    <li key={warning}>{warning}</li>
                  ))}
                </ul>
              </AlertDescription>
            </Alert>
          )}

          {form.watch('bodyFormat') === 'rt_json_v1' && (
            <pre className='max-h-52 overflow-auto rounded-md border bg-muted p-3 text-xs'>
              {form.watch('contentJson')}
            </pre>
          )}

          <FormTextarea
            control={form.control}
            name='excerpt'
            label='Excerpt'
            placeholder='Short summary'
            config={{ rows: 3, maxLength: 1000, showCharCount: true }}
          />

          <FormInput control={form.control} name='featuredImageUrl' label='Featured Image URL' placeholder='https://...' />

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput control={form.control} name='seoTitle' label='SEO Title' placeholder='SEO title override' />
            <FormInput control={form.control} name='seoDescription' label='SEO Description' placeholder='SEO meta description' />
          </div>

          {!initialData && <FormSwitch control={form.control} name='publish' label='Publish immediately' />}

          <Button type='submit'>
            {initialData ? 'Update Post' : 'Create Post'}
          </Button>
        </Form>
      </CardContent>
    </Card>
  );
}
