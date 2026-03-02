'use client';

import { FormInput, FormTextarea, FormSwitch } from '@/shared/ui/forms';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Form } from '@/components/ui/form';
import { zodResolver } from '@hookform/resolvers/zod';
import { useRouter } from 'next/navigation';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import type { PostResponse } from '../api/posts';
import { createPost, updatePost } from '../api/posts';

const formSchema = z.object({
  title: z.string().min(2, 'Title must be at least 2 characters.'),
  slug: z.string().optional(),
  locale: z.string().min(2).default('en'),
  body: z.string().min(10, 'Body must be at least 10 characters.'),
  excerpt: z.string().optional(),
  tags: z.string().optional(),
  featured_image_url: z.string().url().optional().or(z.literal('')),
  seo_title: z.string().optional(),
  seo_description: z.string().optional(),
  publish: z.boolean().default(false)
});

type FormValues = z.infer<typeof formSchema>;

export default function PostForm({
  initialData,
  pageTitle
}: {
  initialData: PostResponse | null;
  pageTitle: string;
}) {
  const router = useRouter();

  const defaultValues: FormValues = {
    title: initialData?.title ?? '',
    slug: initialData?.slug ?? '',
    locale: initialData?.locale ?? 'en',
    body: initialData?.body ?? '',
    excerpt: initialData?.excerpt ?? '',
    tags: initialData?.tags?.join(', ') ?? '',
    featured_image_url: initialData?.featured_image_url ?? '',
    seo_title: initialData?.seo_title ?? '',
    seo_description: initialData?.seo_description ?? '',
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
          excerpt: values.excerpt || undefined,
          tags,
          featured_image_url: values.featured_image_url || undefined,
          seo_title: values.seo_title || undefined,
          seo_description: values.seo_description || undefined,
          version: initialData.version
        });
        toast.success('Post updated');
      } else {
        await createPost({
          title: values.title,
          slug: values.slug || undefined,
          locale: values.locale,
          body: values.body,
          excerpt: values.excerpt || undefined,
          publish: values.publish,
          tags,
          featured_image_url: values.featured_image_url || undefined,
          seo_title: values.seo_title || undefined,
          seo_description: values.seo_description || undefined
        });
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

          <FormTextarea
            control={form.control}
            name='body'
            label='Body'
            placeholder='Write your post content...'
            required
            config={{ rows: 12 }}
          />

          <FormTextarea
            control={form.control}
            name='excerpt'
            label='Excerpt'
            placeholder='Short summary'
            config={{ rows: 3, maxLength: 1000, showCharCount: true }}
          />

          <FormInput
            control={form.control}
            name='featured_image_url'
            label='Featured Image URL'
            placeholder='https://...'
          />

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormInput
              control={form.control}
              name='seo_title'
              label='SEO Title'
              placeholder='SEO title override'
            />
            <FormInput
              control={form.control}
              name='seo_description'
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
