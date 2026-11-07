import * as z from 'zod';
import type { RichTextDocument } from '@rustok/richtext';
import type { CreatePostInput } from '../api/posts';

const id = z
  .uuid()
  .refine((value) => value !== '00000000-0000-0000-0000-000000000000');
const scopeSchema = z.strictObject({ actor_id: id, tenant_id: id });
const inputSchema = z.strictObject({
  commandId: id,
  locale: z.string(),
  title: z.string(),
  content: z.custom<RichTextDocument>(
    (value) =>
      typeof value === 'object' &&
      value !== null &&
      'type' in value &&
      value.type === 'doc' &&
      'content' in value &&
      Array.isArray(value.content)
  ),
  publish: z.boolean(),
  tags: z.array(z.string()),
  excerpt: z.string().optional(),
  slug: z.string().optional(),
  categoryId: z.string().optional(),
  featuredImageUrl: z.string().optional(),
  seoTitle: z.string().optional(),
  seoDescription: z.string().optional(),
  channelSlugs: z.array(z.string()).optional(),
  scheduledAt: z.string().optional()
});
const recordSchema = z.strictObject({
  version: z.literal(1),
  scope: scopeSchema,
  input: inputSchema
});
type JournalStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

export class CreationJournalError extends Error {}

/** Only owner acknowledgement can remove the exact stored predecessor. */
export class PostCreationJournal {
  private readonly storage: JournalStorage;
  private readonly key: string;
  private readonly scope: z.infer<typeof scopeSchema>;
  private raw: string | null;
  private pending: CreatePostInput | undefined;

  constructor(
    storage: JournalStorage,
    actorId: string,
    tenantId: string | null | undefined
  ) {
    this.storage = storage;
    this.scope = scopeSchema.parse({
      actor_id: actorId.toLowerCase(),
      tenant_id: tenantId?.toLowerCase()
    });
    this.key = `rustok:blog:create:next:${this.scope.tenant_id}:${this.scope.actor_id}`;
    this.raw = storage.getItem(this.key);
    if (this.raw !== null) {
      const { scope, input } = recordSchema.parse(JSON.parse(this.raw));
      if (
        scope.tenant_id !== this.scope.tenant_id ||
        scope.actor_id !== this.scope.actor_id
      ) {
        throw new CreationJournalError('Creation journal scope changed');
      }
      this.pending = input;
    }
  }

  get request(): CreatePostInput | undefined {
    return this.pending && structuredClone(this.pending);
  }

  prepare(
    input: Omit<CreatePostInput, 'commandId'>,
    allocate: () => string
  ): CreatePostInput {
    this.verify();
    if (this.pending) return structuredClone(this.pending);
    const pending = inputSchema.parse({
      ...structuredClone(input),
      commandId: allocate()
    });
    const raw = JSON.stringify({
      version: 1,
      scope: this.scope,
      input: pending
    });
    try {
      this.storage.setItem(this.key, raw);
      if (this.storage.getItem(this.key) !== raw) {
        throw new CreationJournalError(
          'Creation journal write was not retained'
        );
      }
    } catch {
      throw new CreationJournalError('Creation journal write failed');
    }
    this.raw = raw;
    this.pending = pending;
    return structuredClone(pending);
  }

  clear(): void {
    this.verify();
    try {
      this.storage.removeItem(this.key);
    } catch {
      throw new CreationJournalError('Creation journal cleanup failed');
    }
    this.raw = null;
    this.pending = undefined;
  }

  private verify(): void {
    try {
      if (this.storage.getItem(this.key) === this.raw) return;
    } catch {
      throw new CreationJournalError('Creation journal read failed');
    }
    throw new CreationJournalError('Creation journal predecessor changed');
  }
}

export function isCreationAcknowledgement(value: unknown): value is string {
  return id.safeParse(value).success;
}

export function isDurableCreationRejection(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'outcome' in error &&
    error.outcome === 'rejected'
  );
}
