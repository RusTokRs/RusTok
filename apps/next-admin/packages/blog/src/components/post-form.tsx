'use client';

import {
  FormInput,
  FormTextarea,
  FormSwitch,
  FormSelect,
  type FormOption
} from '@/shared/ui/forms';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
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
import { useEffect, useMemo, useState } from 'react';
import { useForm, type Resolver } from 'react-hook-form';
import { toast } from 'sonner';
import * as z from 'zod';
import type { PostResponse, GqlOpts } from '../api/posts';
import { createPost, updatePost } from '../api/posts';
import { listBlogCategories, type BlogCategory } from '../api/categories';

const formSchema = z.object({
  title: z.string().min(2, 'Title must be at least 2 characters.'),
  slug: z.string().optional(),
  locale: z.string().min(2),
  categoryId: z.string().optional(),
  channelSlugs: z.string().optional(),
  excerpt: z.string().optional(),
  tags: z.string().optional(),
  featuredImageUrl: z.string().url().optional().or(z.literal('')),
  seoTitle: z.string().optional(),
  seoDescription: z.string().optional(),
  publish: z.boolean().default(false)
});

type FormValues = z.infer<typeof formSchema>;

function resolveInitialDoc(initialData: PostResponse | null): RichTextDocument {
  return initialData?.content?.document ?? emptyRichTextDocument();
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
  const hostLocale = useLocale();
  const defaultLocale =
    initialData?.requestedLocale ?? initialData?.effectiveLocale ?? hostLocale;
  const initialDoc = useMemo(
    () => resolveInitialDoc(initialData),
    [initialData]
  );
  const [content, setContent] = useState<RichTextDocument>(initialDoc);
  const [categories, setCategories] = useState<BlogCategory[]>([]);

  useEffect(() => {
    let active = true;
    listBlogCategories({ locale: defaultLocale }, gqlOpts)
      .then((res) => {
        if (active) {
          setCategories(res.items);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [defaultLocale, gqlOpts]);

  const categoryOptions: FormOption[] = useMemo(() => {
    const opts: FormOption[] = [{ label: 'None (Uncategorized)', value: '' }];
    categories.forEach((cat) => {
      opts.push({ label: cat.name, value: cat.id });
    });
    return opts;
  }, [categories]);

  const defaultValues: FormValues = {
    title: initialData?.title ?? '',
    slug: initialData?.slug ?? '',
    locale: defaultLocale,
    categoryId: initialData?.categoryId ?? '',
    channelSlugs: initialData?.channelSlugs?.join(', ') ?? '',
    excerpt: initialData?.excerpt ?? '',
    tags: initialData?.tags?.join(', ') ?? '',
    featuredImageUrl: initialData?.featuredImageUrl ?? '',
    seoTitle: initialData?.seoTitle ?? '',
    seoDescription: initialData?.seoDescription ?? '',
    publish: false
  };

  const form = useForm<FormValues>({
    resolver: zodResolver(formSchema) as Resolver<FormValues>,
    defaultValues
  });
  const contentLocale = form.watch('locale');
  const featuredImageUrl = form.watch('featuredImageUrl');

  async function onSubmit(values: FormValues) {
    const tags = values.tags
      ? values.tags
          .split(',')
          .map((t) => t.trim())
          .filter(Boolean)
      : [];

    const channelSlugs = values.channelSlugs
      ? values.channelSlugs
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean)
      : [];

    const validation = validateRichTextDocument(
      content,
      getRichTextProfile('article')
    );
    if (!validation.valid || !richTextDocumentHasText(content)) {
      toast.error(validation.error ?? 'Post content is required.');
      return;
    }

    try {
      if (initialData) {
        await updatePost(
          initialData.id,
          {
            title: values.title,
            slug: values.slug || undefined,
            locale: values.locale,
            content,
            excerpt: values.excerpt || undefined,
            tags,
            categoryId: values.categoryId ? values.categoryId : null,
            channelSlugs,
            featuredImageUrl: values.featuredImageUrl || undefined,
            seoTitle: values.seoTitle || undefined,
            seoDescription: values.seoDescription || undefined,
            version: initialData.version
          },
          gqlOpts
        );
        toast.success('Post updated');
      } else {
        await createPost(
          {
            title: values.title,
            slug: values.slug || undefined,
            locale: values.locale,
            content,
            excerpt: values.excerpt || undefined,
            publish: values.publish,
            tags,
            categoryId: values.categoryId ? values.categoryId : undefined,
            channelSlugs: channelSlugs.length > 0 ? channelSlugs : undefined,
            featuredImageUrl: values.featuredImageUrl || undefined,
            seoTitle: values.seoTitle || undefined,
            seoDescription: values.seoDescription || undefined
          },
          gqlOpts
        );
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
              placeholder='Host locale'
              required
            />
            <FormInput
              control={form.control}
              name='tags'
              label='Tags'
              placeholder='rust, blog, news'
            />
          </div>

          <div className='grid grid-cols-1 gap-6 md:grid-cols-2'>
            <FormSelect
              control={form.control}
              name='categoryId'
              label='Category'
              options={categoryOptions}
              placeholder='Select category'
            />
            <FormInput
              control={form.control}
              name='channelSlugs'
              label='Channels'
              placeholder='e.g. default, mobile'
              description='Comma-separated channel slugs'
            />
          </div>

          <RichTextEditor
            label='Content'
            profile='article'
            value={content}
            contentLocale={contentLocale}
            disabled={form.formState.isSubmitting}
            onChange={setContent}
          />

          <FormTextarea
            control={form.control}
            name='excerpt'
            label='Excerpt'
            placeholder='Short summary'
            config={{ rows: 3, maxLength: 1000, showCharCount: true }}
          />

          <div className='space-y-2'>
            <FormInput
              control={form.control}
              name='featuredImageUrl'
              label='Featured Image URL'
              placeholder='https://...'
            />
            {featuredImageUrl && (
              <div className='bg-muted relative mt-2 h-44 max-w-sm overflow-hidden rounded-md border'>
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img
                  src={featuredImageUrl}
                  alt='Featured image preview'
                  className='h-full w-full object-cover'
                  onError={(e) => {
                    (e.currentTarget as HTMLElement).style.display = 'none';
                  }}
                />
              </div>
            )}
          </div>

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

          <Button type='submit' disabled={form.formState.isSubmitting}>
            {initialData ? 'Update Post' : 'Create Post'}
          </Button>
        </Form>
      </CardContent>
    </Card>
  );
}
