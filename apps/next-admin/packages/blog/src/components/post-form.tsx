'use client';

import {
  FormInput,
  FormTextarea,
  FormSwitch,
  FormSelect,
  type FormOption
} from '@/shared/ui/forms';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
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
import { useLocale, useTranslations } from '@rustok/next-fluent';
import { useRouter } from 'next/navigation';
import Image from 'next/image';
import { useEffect, useRef, useState } from 'react';
import { useForm } from 'react-hook-form';
import { toast } from 'sonner';
import type { PostResponse, GqlOpts } from '../api/posts';
import { createPost, updatePost } from '../api/posts';
import { listAllBlogCategories, type BlogCategory } from '../api/categories';
import {
  buildPostCommand,
  PostCreationAttempt,
  canSubmitEditor,
  completeEditorWrite,
  type EditorWriteState,
  EditorValidationError,
  targetPostCopy,
  postImageSource,
  type PostEditorValues
} from '../model/editor';
import { BlogModerationPanel } from './moderation-panel';

type Props = {
  initialData: PostResponse | null;
  pageTitle: string;
  actorId: string;
  gqlOpts?: GqlOpts;
};
export default function PostForm(props: Props) {
  // Owner snapshots and tenant switches create new form sessions. React Hook Form
  // defaults must never retain values from a previous post or principal.
  const session = JSON.stringify([
    props.initialData,
    props.gqlOpts?.tenantId,
    props.gqlOpts?.tenantSlug,
    props.actorId,
    props.gqlOpts?.locale
  ]);
  return <PostEditorSession key={session} {...props} />;
}
function PostEditorSession({ initialData, pageTitle, gqlOpts = {} }: Props) {
  const router = useRouter();
  const t = useTranslations('blog');
  const hostLocale = useLocale();
  const locale = initialData?.requestedLocale ?? gqlOpts.locale ?? hostLocale;
  const targetCopy = targetPostCopy(initialData);
  const [content, setContent] = useState<RichTextDocument>(
    targetCopy.document ?? emptyRichTextDocument()
  );
  const documentRef = useRef(content);
  const writeState = useRef<EditorWriteState>('idle');
  const creation = useRef(new PostCreationAttempt());
  const [creationPending, setCreationPending] = useState(false);
  const [writeAcknowledged, setWriteAcknowledged] = useState(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const [categories, setCategories] = useState<BlogCategory[]>([]);
  const [categoryState, setCategoryState] = useState<
    'loading' | 'ready' | 'error'
  >('loading');
  const [categoryRetry, setCategoryRetry] = useState(0);
  const { token, tenantSlug, tenantId } = gqlOpts;
  useEffect(() => {
    let active = true;
    setCategoryState('loading');
    listAllBlogCategories({ locale }, { token, tenantSlug, tenantId, locale })
      .then((rows) => {
        if (active) {
          setCategories(rows);
          setCategoryState('ready');
        }
      })
      .catch(() => {
        if (active) setCategoryState('error');
      });
    return () => {
      active = false;
    };
  }, [locale, token, tenantSlug, tenantId, categoryRetry]);
  const options: FormOption[] = [
    { label: t('form.uncategorized'), value: '__none__' },
    ...categories.map((category) => ({
      label: category.name,
      value: category.id
    }))
  ];
  if (
    initialData?.categoryId &&
    !categories.some((row) => row.id === initialData.categoryId)
  ) {
    options.push({
      label: initialData.categoryName ?? t('editor.currentCategory'),
      value: initialData.categoryId
    });
  }
  const form = useForm<PostEditorValues>({
    defaultValues: {
      title: targetCopy.title,
      slug: initialData?.slug ?? '',
      locale,
      categoryId: initialData?.categoryId ?? '__none__',
      channelSlugs: initialData?.channelSlugs?.join(', ') ?? '',
      excerpt: targetCopy.excerpt,
      tags: initialData?.tags.join(', ') ?? '',
      featuredImageUrl: initialData?.featuredImageUrl ?? '',
      seoTitle: targetCopy.seoTitle,
      seoDescription: targetCopy.seoDescription,
      publish: false,
      scheduledAt: initialData?.scheduledAt ?? ''
    }
  });
  const contentLocale = form.watch('locale');
  const publishNow = form.watch('publish');
  const imageSource = postImageSource(form.watch('featuredImageUrl'));
  useEffect(() => {
    if (publishNow) form.setValue('scheduledAt', '');
  }, [publishNow, form]);
  async function onSubmit(values: PostEditorValues) {
    if (!canSubmitEditor(writeState.current)) return;
    let acknowledged = false;
    try {
      const retained = creation.current.request;
      const command = retained ? { kind: 'create' as const, input: retained } : buildPostCommand(
        values,
        documentRef.current,
        initialData,
        emptyRichTextDocument(),
        richTextDocumentHasText
      );
      if (command.input.content) {
        const validation = validateRichTextDocument(
          command.input.content,
          getRichTextProfile('article')
        );
        if (!validation.valid) {
          form.setError('root', { message: t('editor.requiredCopy') });
          return;
        }
      }
      writeState.current = 'saving';
      if (command.kind === 'update' && initialData)
        await updatePost(initialData.id, command.input, gqlOpts);
      else if (command.kind === 'create') {
        const input = creation.current.prepare(command.input, () => crypto.randomUUID());
        setCreationPending(true);
        await createPost(input, gqlOpts);
      }
      acknowledged = true;
    } catch (error) {
      if (!active.current) return;
      creation.current.resolve(error);
      setCreationPending(Boolean(creation.current.request));
      if (error instanceof EditorValidationError) {
        const messages = {
          requiredCopy: t('editor.requiredCopy'),
          localeChanged: t('editor.localeChanged'),
          invalidSchedule: t('editor.invalidSchedule'),
          invalidImage: t('editor.invalidImage'),
          slugRequired: t('editor.slugRequired'),
          revisionMissing: t('editor.revisionMissing')
        };
        form.setError('root', { message: messages[error.code] });
      } else toast.error(t('error.savePost'));
    } finally {
      writeState.current = completeEditorWrite(acknowledged);
    }
    if (acknowledged && active.current) {
      setWriteAcknowledged(true);
      toast.success(t('editor.saved'));
      // Navigation is independent of the acknowledged owner write.
      router.push('/dashboard/blog');
      router.refresh();
    }
  }
  return (
    <>
      <Card className='mx-auto w-full'>
        <CardHeader>
          <CardTitle>{pageTitle}</CardTitle>
        </CardHeader>
        <CardContent>
          {initialData &&
            initialData.requestedLocale !== initialData.effectiveLocale && (
              <Alert>
                <AlertDescription>
                  <p>{t('editor.fallbackNotice')}</p>
                  <p lang={initialData.effectiveLocale}>{initialData.title}</p>
                  <p lang={initialData.effectiveLocale}>
                    {initialData.contentPlainText}
                  </p>
                </AlertDescription>
              </Alert>
            )}
          {creationPending && <Alert><AlertDescription>{t('editor.creationUnknown')}</AlertDescription></Alert>}
          <Form
            form={form}
            onSubmit={form.handleSubmit(onSubmit)}
            className='space-y-6'
          >
            <FormInput
              control={form.control}
              name='title'
              label={t('form.title')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <FormInput
              control={form.control}
              name='slug'
              label={t('form.slug')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <FormInput
              control={form.control}
              name='locale'
              label={t('form.locale')}
              disabled={
                Boolean(initialData) ||
                form.formState.isSubmitting ||
                writeAcknowledged || creationPending
              }
            />
            <FormInput
              control={form.control}
              name='tags'
              label={t('form.tags')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <FormSelect
              control={form.control}
              name='categoryId'
              label={t('form.category')}
              options={options}
              disabled={
                categoryState !== 'ready' ||
                form.formState.isSubmitting ||
                writeAcknowledged || creationPending
              }
            />
            {categoryState === 'error' && (
              <Alert variant='destructive'>
                <AlertDescription>
                  {t('editor.categoriesUnavailable')}{' '}
                  <Button
                    type='button'
                    onClick={() => setCategoryRetry((value) => value + 1)}
                  >
                    {t('editor.retry')}
                  </Button>
                </AlertDescription>
              </Alert>
            )}
            <FormInput
              control={form.control}
              name='channelSlugs'
              label={t('form.channels')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <RichTextEditor
              label={t('form.content')}
              profile='article'
              value={content}
              contentLocale={contentLocale}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
              onChange={(document) => {
                documentRef.current = document;
                setContent(document);
              }}
            />
            <FormTextarea
              control={form.control}
              name='excerpt'
              label={t('form.excerpt')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <FormInput
              control={form.control}
              name='featuredImageUrl'
              label={t('form.featuredImage')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            {imageSource && (
              <Image
                src={imageSource}
                alt={t('editor.imagePreview')}
                width={560}
                height={320}
                unoptimized
                className='max-w-sm rounded-md object-cover'
              />
            )}
            <FormInput
              control={form.control}
              name='seoTitle'
              label={t('form.seoTitle')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            <FormTextarea
              control={form.control}
              name='seoDescription'
              label={t('form.seoDescription')}
              disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
            />
            {!initialData && (
              <FormSwitch
                control={form.control}
                name='publish'
                label={t('form.publishNow')}
                disabled={form.formState.isSubmitting || writeAcknowledged || creationPending}
              />
            )}
            {(!initialData || initialData.status === 'DRAFT') && (
              <FormInput
                control={form.control}
                name='scheduledAt'
                label={t('form.scheduledAt')}
                description={t('editor.scheduleHelp')}
                disabled={
                  publishNow || form.formState.isSubmitting || writeAcknowledged || creationPending
                }
              />
            )}
            {form.formState.errors.root?.message && (
              <Alert variant='destructive'>
                <AlertDescription>
                  {form.formState.errors.root.message}
                </AlertDescription>
              </Alert>
            )}
            <Button
              type='submit'
              disabled={form.formState.isSubmitting || writeAcknowledged}
            >
              {initialData ? t('form.update') : t('form.create')}
            </Button>
          </Form>
        </CardContent>
      </Card>
      {initialData && (
        <BlogModerationPanel
          postId={initialData.id}
          locale={initialData.requestedLocale}
          gqlOpts={gqlOpts}
        />
      )}
    </>
  );
}
