import * as z from 'zod';
import type { RichTextDocument } from '@rustok/richtext';
import type {
  PostResponse,
  CreatePostInput,
  UpdatePostInput
} from '../api/posts';
import type { BlogCategory, UpdateBlogCategoryInput } from '../api/categories';

export interface PostEditorValues {
  title: string;
  slug?: string;
  locale: string;
  categoryId?: string;
  channelSlugs?: string;
  excerpt?: string;
  tags?: string;
  featuredImageUrl?: string;
  seoTitle?: string;
  seoDescription?: string;
  publish: boolean;
  scheduledAt?: string;
}
export type EditorError =
  | 'requiredCopy'
  | 'localeChanged'
  | 'invalidSchedule'
  | 'invalidImage'
  | 'slugRequired'
  | 'revisionMissing';
export class EditorValidationError extends Error {
  readonly code: EditorError;
  constructor(code: EditorError) {
    super(code);
    this.code = code;
  }
}
const optional = (value?: string | null) => value?.trim() || undefined;
const nullable = (value?: string | null) => optional(value) ?? null;
const csv = (value?: string) =>
  (value ?? '')
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean);
const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);

export type EditorWriteState = 'idle' | 'saving' | 'acknowledged';
export function canSubmitEditor(state: EditorWriteState): boolean {
  return state === 'idle';
}
export function completeEditorWrite(acknowledged: boolean): EditorWriteState {
  return acknowledged ? 'acknowledged' : 'idle';
}

export function targetPostCopy(post: PostResponse | null) {
  const exact = post?.requestedLocale === post?.effectiveLocale;
  return {
    title: exact ? (post?.title ?? '') : '',
    excerpt: exact ? (post?.excerpt ?? '') : '',
    seoTitle: exact ? (post?.seoTitle ?? '') : '',
    seoDescription: exact ? (post?.seoDescription ?? '') : '',
    document: exact ? post?.content?.document : undefined
  };
}

/** Input uses an offset-bearing RFC 3339 instant; local datetime ambiguity is not accepted. */
function schedule(value?: string): string | undefined {
  if (!optional(value)) return undefined;
  if (!z.iso.datetime({ offset: true }).safeParse(value).success) {
    throw new EditorValidationError('invalidSchedule');
  }
  return new Date(value!).toISOString();
}
export function buildPostCommand(
  values: PostEditorValues,
  document: RichTextDocument,
  post: PostResponse | null,
  emptyDocument: RichTextDocument,
  hasText: (value: RichTextDocument) => boolean
):
  | { kind: 'create'; input: Omit<CreatePostInput, 'commandId'> }
  | { kind: 'update'; input: UpdatePostInput } {
  const title = values.title.trim();
  const locale = values.locale.trim();
  const image = optional(values.featuredImageUrl);
  if (image && !postImageSource(image))
    throw new EditorValidationError('invalidImage');
  if (!locale) throw new EditorValidationError('localeChanged');
  const date = schedule(values.scheduledAt);
  if (!post) {
    if (!title || !hasText(document))
      throw new EditorValidationError('requiredCopy');
    return {
      kind: 'create',
      input: {
        locale,
        title,
        content: document,
        slug: optional(values.slug),
        excerpt: optional(values.excerpt),
        tags: csv(values.tags),
        categoryId:
          values.categoryId === '__none__'
            ? undefined
            : optional(values.categoryId),
        channelSlugs: csv(values.channelSlugs),
        featuredImageUrl: image,
        seoTitle: optional(values.seoTitle),
        seoDescription: optional(values.seoDescription),
        publish: values.publish,
        scheduledAt: values.publish ? undefined : date
      }
    };
  }
  if (!Number.isInteger(post.version) || post.version < 1)
    throw new EditorValidationError('revisionMissing');
  if (locale !== post.requestedLocale)
    throw new EditorValidationError('localeChanged');
  if (!optional(values.slug)) throw new EditorValidationError('slugRequired');
  const baseline = targetPostCopy(post);
  const input: UpdatePostInput = { version: post.version };
  if (title !== baseline.title) input.title = title;
  if (!same(document, baseline.document ?? emptyDocument))
    input.content = document;
  for (const key of ['excerpt', 'seoTitle', 'seoDescription'] as const) {
    if (nullable(values[key]) !== nullable(baseline[key]))
      input[key] = nullable(values[key]);
  }
  const copyChanged = [
    'title',
    'content',
    'excerpt',
    'seoTitle',
    'seoDescription'
  ].some((key) => key in input);
  if (
    (post.requestedLocale !== post.effectiveLocale &&
      copyChanged &&
      (!title || !hasText(document))) ||
    (input.title !== undefined && !input.title) ||
    (input.content !== undefined && !hasText(input.content))
  ) {
    throw new EditorValidationError('requiredCopy');
  }
  if (!same(csv(values.tags), post.tags)) input.tags = csv(values.tags);
  if (copyChanged || input.tags !== undefined) input.locale = locale;
  const selectedCategory =
    values.categoryId === '__none__' ? null : nullable(values.categoryId);
  if (selectedCategory !== (post.categoryId ?? null))
    input.categoryId = selectedCategory;
  if (!same(csv(values.channelSlugs), post.channelSlugs ?? []))
    input.channelSlugs = csv(values.channelSlugs);
  if (nullable(values.featuredImageUrl) !== post.featuredImageUrl)
    input.featuredImageUrl = nullable(values.featuredImageUrl);
  if (optional(values.slug) !== optional(post.slug) && optional(values.slug))
    input.slug = optional(values.slug);
  if (
    post.status === 'DRAFT' &&
    (date ?? null) !==
      (post.scheduledAt ? new Date(post.scheduledAt).toISOString() : null)
  )
    input.scheduledAt = date ?? null;
  return { kind: 'update', input };
}

export function targetCategoryCopy(category?: BlogCategory | null) {
  const exact = category?.locale === category?.effectiveLocale;
  return {
    name: exact ? (category?.name ?? '') : '',
    slug: exact ? (category?.slug ?? '') : '',
    description: exact ? (category?.description ?? '') : ''
  };
}
export function buildCategoryUpdate(
  category: BlogCategory,
  locale: string,
  values: { name: string; slug?: string; description?: string }
): UpdateBlogCategoryInput {
  if (locale !== category.locale)
    throw new EditorValidationError('localeChanged');
  const baseline = targetCategoryCopy(category);
  if (baseline.slug && values.slug !== undefined && !optional(values.slug))
    throw new EditorValidationError('slugRequired');
  const input: UpdateBlogCategoryInput = { locale };
  const name = values.name.trim();
  if (name !== baseline.name) input.name = name;
  if (nullable(values.description) !== nullable(baseline.description))
    input.description = nullable(values.description);
  if (
    optional(values.slug) !== optional(baseline.slug) &&
    optional(values.slug)
  )
    input.slug = optional(values.slug);
  if (
    (category.locale !== category.effectiveLocale &&
      !validCategoryName(name)) ||
    (input.name !== undefined && !validCategoryName(input.name))
  )
    throw new EditorValidationError('requiredCopy');
  return input;
}

export function validCategoryName(value: string): boolean {
  const size = Array.from(value.trim()).length;
  return size >= 1 && size <= 120;
}

export function postImageSource(value?: string | null): string | undefined {
  const source = optional(value);
  return source &&
    ((source.startsWith('/') && !source.startsWith('//')) ||
      /^https?:\/\//.test(source))
    ? source
    : undefined;
}

export function creationFormValues(input: CreatePostInput): PostEditorValues {
  return {
    title: input.title,
    slug: input.slug ?? '',
    locale: input.locale,
    categoryId: input.categoryId ?? '__none__',
    channelSlugs: input.channelSlugs?.join(', ') ?? '',
    excerpt: input.excerpt ?? '',
    tags: input.tags.join(', '),
    featuredImageUrl: input.featuredImageUrl ?? '',
    seoTitle: input.seoTitle ?? '',
    seoDescription: input.seoDescription ?? '',
    publish: input.publish,
    scheduledAt: input.scheduledAt ?? ''
  };
}
