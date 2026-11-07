'use client';

import { useEffect, useRef, useState } from 'react';
import { useTranslations } from '@rustok/next-fluent/client';
import { Alert, AlertDescription } from '@/shared/ui/shadcn/alert';
import {
  buildCategoryUpdate,
  targetCategoryCopy,
  validCategoryName,
  canSubmitEditor,
  completeEditorWrite,
  type EditorWriteState,
  EditorValidationError
} from '../model/editor';
import { useForm, type Resolver } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import * as z from 'zod';
import { toast } from 'sonner';

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter
} from '@/shared/ui/shadcn/dialog';
import { Button } from '@/components/ui/button';
import { Form } from '@/shared/ui/shadcn/form';
import { FormInput, FormTextarea, FormSelect } from '@/shared/ui/forms';

import type { BlogCategory, GqlOpts } from '../api/categories';
import { createBlogCategory, updateBlogCategory } from '../api/categories';

const categorySchema = (invalidPosition: string) =>
  z.object({
    name: z.string(),
    slug: z.string().optional(),
    description: z.string().optional(),
    parentId: z.string().optional(),
    /** Canonical sibling position; the owner clamps it to the sibling count. */
    position: z.coerce
      .number({ error: invalidPosition })
      .int({ error: invalidPosition })
      .min(0, { error: invalidPosition })
      .default(0)
  });

type CategoryFormValues = z.infer<ReturnType<typeof categorySchema>>;

interface CategoryModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  category?: BlogCategory | null;
  categories: BlogCategory[];
  locale: string;
  gqlOpts?: GqlOpts;
  onSuccess: () => void;
}

export function CategoryModal(props: CategoryModalProps) {
  const session = JSON.stringify([
    props.open,
    props.category,
    props.locale,
    props.gqlOpts?.tenantId,
    props.gqlOpts?.tenantSlug,
    props.gqlOpts?.token
  ]);
  return <CategoryEditorSession key={session} {...props} />;
}

function CategoryEditorSession({
  open,
  onOpenChange,
  category,
  categories,
  locale,
  gqlOpts = {},
  onSuccess
}: CategoryModalProps) {
  const isEditing = Boolean(category);
  const t = useTranslations('blog');
  const writeState = useRef<EditorWriteState>('idle');
  const [writeAcknowledged, setWriteAcknowledged] = useState(false);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  const form = useForm<CategoryFormValues>({
    resolver: zodResolver(
      categorySchema(t('categories.error.invalidPosition'))
    ) as Resolver<CategoryFormValues>,
    defaultValues: {
      name: '',
      slug: '',
      description: '',
      parentId: '__root__',
      position: 0
    }
  });

  useEffect(() => {
    if (open) {
      if (category) {
        form.reset({
          name: targetCategoryCopy(category).name,
          slug: targetCategoryCopy(category).slug,
          description: targetCategoryCopy(category).description,
          parentId: category.parentId ?? '__root__',
          position: category.position
        });
      } else {
        form.reset({
          name: '',
          slug: '',
          description: '',
          parentId: '__root__',
          position: 0
        });
      }
    }
  }, [open, category, form]);

  async function onSubmit(values: CategoryFormValues) {
    if (!canSubmitEditor(writeState.current)) return;
    writeState.current = 'saving';
    let acknowledged = false;
    try {
      if (isEditing && category) {
        // The owner exposes the canonical sibling position through a dedicated
        // move command, not through the update command, so it is not sent here.
        await updateBlogCategory(
          category.id,
          buildCategoryUpdate(category, locale, values),
          gqlOpts
        );
      } else {
        if (!validCategoryName(values.name)) {
          form.setError('name', {
            message: t('categories.error.invalidName')
          });
          return;
        }
        await createBlogCategory(
          {
            name: values.name,
            slug: values.slug || undefined,
            description: values.description?.trim() || undefined,
            parentId:
              values.parentId === '__root__' ? undefined : values.parentId,
            position: values.position,
            locale
          },
          gqlOpts
        );
      }

      acknowledged = true;
    } catch (error) {
      if (!active.current) return;
      if (error instanceof EditorValidationError)
        form.setError('root', {
          message:
            error.code === 'slugRequired'
              ? t('editor.slugRequired')
              : t('categories.error.invalidName')
        });
      else toast.error(t('categories.error.save'));
    } finally {
      writeState.current = completeEditorWrite(acknowledged);
    }
    if (acknowledged && active.current) {
      setWriteAcknowledged(true);
      toast.success(
        isEditing ? t('categories.updated') : t('categories.created')
      );
      // Refresh/navigation are independent of the acknowledged owner write.
      onOpenChange(false);
      onSuccess();
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[480px]'>
        <DialogHeader>
          <DialogTitle>
            {isEditing ? t('categories.edit') : t('categories.new')}
          </DialogTitle>
        </DialogHeader>

        <Form
          form={form}
          onSubmit={form.handleSubmit(onSubmit)}
          className='space-y-4'
        >
          {category && category.locale !== category.effectiveLocale && (
            <Alert>
              <AlertDescription>
                <p>{t('editor.categoryFallbackNotice')}</p>
                <p lang={category.effectiveLocale}>{category.name}</p>
                <p>{category.description}</p>
              </AlertDescription>
            </Alert>
          )}
          {form.formState.errors.root?.message && (
            <Alert variant='destructive'>
              <AlertDescription>
                {form.formState.errors.root.message}
              </AlertDescription>
            </Alert>
          )}
          <FormInput
            control={form.control}
            disabled={form.formState.isSubmitting || writeAcknowledged}
            name='name'
            label={t('categories.form.name')}

            required
          />

          <FormInput
            control={form.control}
            disabled={form.formState.isSubmitting || writeAcknowledged}
            name='slug'
            label={t('categories.form.slug')}
          />

          <FormTextarea
            control={form.control}
            disabled={form.formState.isSubmitting || writeAcknowledged}
            name='description'
            label={t('categories.form.description')}

            config={{ rows: 3 }}
          />

          {!isEditing && (
            <FormSelect
              control={form.control}
              disabled={form.formState.isSubmitting || writeAcknowledged}
              name='parentId'
              label={t('categories.form.parent')}
              options={[
                { label: t('categories.form.root'), value: '__root__' },
                ...categories.map((item) => ({
                  label: item.name,
                  value: item.id
                }))
              ]}
            />
          )}

          {!isEditing && (
            <FormInput
              control={form.control}
              disabled={form.formState.isSubmitting || writeAcknowledged}
              name='position'
              label={t('categories.form.position')}
              type='number'
              placeholder='0'
            />
          )}

          <DialogFooter className='pt-4'>
            <Button
              type='button'
              variant='outline'
              onClick={() => onOpenChange(false)}
              disabled={form.formState.isSubmitting || writeAcknowledged}
            >
              {t('categories.cancel')}
            </Button>
            <Button
              type='submit'
              disabled={form.formState.isSubmitting || writeAcknowledged}
            >
              {isEditing ? t('categories.update') : t('categories.create')}
            </Button>
          </DialogFooter>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
