'use client';

import { useEffect } from 'react';
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

const categorySchema = z.object({
  name: z.string().min(2, 'Name must be at least 2 characters.'),
  slug: z.string().optional(),
  description: z.string().optional(),
  parentId: z.string().optional(),
  /** Canonical sibling position; the owner clamps it to the sibling count. */
  position: z.coerce.number().int().min(0).default(0)
});

type CategoryFormValues = z.infer<typeof categorySchema>;

interface CategoryModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  category?: BlogCategory | null;
  categories: BlogCategory[];
  locale: string;
  gqlOpts?: GqlOpts;
  onSuccess: () => void;
}

export function CategoryModal({
  open,
  onOpenChange,
  category,
  categories,
  locale,
  gqlOpts = {},
  onSuccess
}: CategoryModalProps) {
  const isEditing = Boolean(category);

  const form = useForm<CategoryFormValues>({
    resolver: zodResolver(categorySchema) as Resolver<CategoryFormValues>,
    defaultValues: {
      name: '',
      slug: '',
      description: '',
      parentId: '',
      position: 0
    }
  });

  useEffect(() => {
    if (open) {
      if (category) {
        form.reset({
          name: category.name,
          slug: category.slug,
          description: category.description ?? '',
          parentId: category.parentId ?? '',
          position: category.position
        });
      } else {
        form.reset({
          name: '',
          slug: '',
          description: '',
          parentId: '',
          position: 0
        });
      }
    }
  }, [open, category, form]);

  async function onSubmit(values: CategoryFormValues) {
    try {
      if (isEditing && category) {
        // The owner exposes the canonical sibling position through a dedicated
        // move command, not through the update command, so it is not sent here.
        await updateBlogCategory(
          category.id,
          {
            name: values.name,
            slug: values.slug || undefined,
            description: values.description?.trim() || null,
            locale
          },
          gqlOpts
        );
        toast.success('Category updated successfully');
      } else {
        await createBlogCategory(
          {
            name: values.name,
            slug: values.slug || undefined,
            description: values.description?.trim() || undefined,
            parentId: values.parentId || undefined,
            position: values.position,
            locale
          },
          gqlOpts
        );
        toast.success('Category created successfully');
      }

      onOpenChange(false);
      onSuccess();
    } catch {
      toast.error(
        isEditing ? 'Failed to update category' : 'Failed to create category'
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='sm:max-w-[480px]'>
        <DialogHeader>
          <DialogTitle>
            {isEditing ? 'Edit Category' : 'Create New Category'}
          </DialogTitle>
        </DialogHeader>

        <Form
          form={form}
          onSubmit={form.handleSubmit(onSubmit)}
          className='space-y-4'
        >
          <FormInput
            control={form.control}
            name='name'
            label='Name'
            placeholder='e.g. Technology, Tutorials'
            required
          />

          <FormInput
            control={form.control}
            name='slug'
            label='Slug'
            placeholder='auto-generated-if-empty'
          />

          <FormTextarea
            control={form.control}
            name='description'
            label='Description'
            placeholder='Short description of this category'
            config={{ rows: 3 }}
          />

          {!isEditing && (
            <FormSelect
              control={form.control}
              name='parentId'
              label='Parent category'
              options={[
                { label: 'Root category', value: '' },
                ...categories.map((item) => ({ label: item.name, value: item.id }))
              ]}
            />
          )}

          {!isEditing && (
            <FormInput
              control={form.control}
              name='position'
              label='Position'
              type='number'
              placeholder='0'
            />
          )}

          <DialogFooter className='pt-4'>
            <Button
              type='button'
              variant='outline'
              onClick={() => onOpenChange(false)}
              disabled={form.formState.isSubmitting}
            >
              Cancel
            </Button>
            <Button type='submit' disabled={form.formState.isSubmitting}>
              {isEditing ? 'Save Changes' : 'Create Category'}
            </Button>
          </DialogFooter>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
