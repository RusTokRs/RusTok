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
import { FormInput, FormTextarea, FormSwitch } from '@/shared/ui/forms';

import type { BlogCategory, GqlOpts } from '../api/categories';
import { createBlogCategory, updateBlogCategory } from '../api/categories';

const categorySchema = z.object({
  name: z.string().min(2, 'Name must be at least 2 characters.'),
  slug: z.string().optional(),
  description: z.string().optional(),
  displayOrder: z.coerce.number().default(0),
  isActive: z.boolean().default(true)
});

type CategoryFormValues = z.infer<typeof categorySchema>;

interface CategoryModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  category?: BlogCategory | null;
  locale: string;
  gqlOpts?: GqlOpts;
  onSuccess: () => void;
}

export function CategoryModal({
  open,
  onOpenChange,
  category,
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
      displayOrder: 0,
      isActive: true
    }
  });

  useEffect(() => {
    if (open) {
      if (category) {
        form.reset({
          name: category.name,
          slug: category.slug,
          description: category.description ?? '',
          displayOrder: category.displayOrder,
          isActive: category.isActive
        });
      } else {
        form.reset({
          name: '',
          slug: '',
          description: '',
          displayOrder: 0,
          isActive: true
        });
      }
    }
  }, [open, category, form]);

  async function onSubmit(values: CategoryFormValues) {
    try {
      if (isEditing && category) {
        await updateBlogCategory(
          category.id,
          {
            name: values.name,
            slug: values.slug || undefined,
            description: values.description || undefined,
            displayOrder: values.displayOrder,
            isActive: values.isActive,
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
            description: values.description || undefined,
            displayOrder: values.displayOrder,
            isActive: values.isActive,
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

          <FormInput
            control={form.control}
            name='displayOrder'
            label='Display Order'
            type='number'
            placeholder='0'
          />

          <FormSwitch control={form.control} name='isActive' label='Active' />

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
