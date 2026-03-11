'use client';

import { FormInput, FormTextarea, FormSwitch, FormSelect } from '@/shared/ui/forms';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Form } from '@/components/ui/form';
import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from 'next/navigation';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import type { PostResponse, GqlOpts } from '../api/posts';
import { createPost, updatePost } from '../api/posts';

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

  const defaultValues: FormValues = {
    title: initialData?.title ?? '',
    slug: initialData?.slug ?? '',
    locale: 'en',
    bodyFormat: 'markdown',
    body: initialData?.body ?? '',
    contentJson: '',
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

  async function onSubmit(values: FormValues) {
    const tags = values.tags
      ? values.tags.split(',').map((t) => t.trim()).filter(Boolean)
      : [];

    try {
      if (initialData) {
        await updatePost(initialData.id, {
          title: values.title,
          slug: values.slug || undefined,
          locale: values.locale,
          body: values.body,
          bodyFormat: values.bodyFormat,
          contentJson: values.bodyFormat === 'rt_json_v1' && values.contentJson
            ? JSON.parse(values.contentJson)
            : undefined,
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
          contentJson: values.bodyFormat === 'rt_json_v1' && values.contentJson
            ? JSON.parse(values.contentJson)
            : undefined,
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
            <FormInput
              control={form.control}
              name='title'
              label='Title'
              placeholder='Enter post title'
              required
            />
            <FormInput
              control={form.control}
              name='slug'
              label='Slug'
              placeholder='auto-generated-if-empty'
            />
          </div>

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput
              control={form.control}
              name='locale'
              label='Locale'
              placeholder='en'
              required
            />
            <FormInput
              control={form.control}
              name='tags'
              label='Tags'
              placeholder='rust, blog, news'
            />
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

          <FormTextarea
            control={form.control}
            name='body'
            label='Body'
            placeholder='Write your post content...'
            required
            config={{ rows: 12 }}
          />

          {form.watch('bodyFormat') === 'rt_json_v1' && (
            <FormTextarea
              control={form.control}
              name='contentJson'
              label='Content JSON (rt_json_v1)'
              placeholder='{"type":"doc","content":[]}'
              required
              config={{ rows: 8 }}
            />
          )}

          <FormTextarea
            control={form.control}
            name='excerpt'
            label='Excerpt'
            placeholder='Short summary'
            config={{ rows: 3, maxLength: 1000, showCharCount: true }}
          />

          <FormInput
            control={form.control}
            name='featuredImageUrl'
            label='Featured Image URL'
            placeholder='https://...'
          />

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput
              control={form.control}
              name='seoTitle'
              label='SEO Title'
              placeholder='SEO title override'
            />
            <FormInput
              control={form.control}
              name='seoDescription'
              label='SEO Description'
              placeholder='SEO meta description'
            />
          </div>

          {!initialData && (
            <FormSwitch
              control={form.control}
              name='publish'
              label='Publish immediately'
            />
          )}

          <Button type='submit'>
            {initialData ? 'Update Post' : 'Create Post'}
          </Button>
        </Form>
      </CardContent>
    </Card>
  );
}
