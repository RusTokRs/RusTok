'use client';

import * as React from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '@/shared/ui/shadcn/dialog';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Plus, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { useTranslations } from '@rustok/next-fluent/client';
import type {
  CatalogCategorySummary,
  CreateCatalogCategoryPayload
} from '../../api/types';
import { SearchableCategorySelect } from './searchable-category-select';
import { buildCategoryHierarchyOptions } from '../../category-hierarchy';
import {
  categoryCodeFromName,
  slugifyCategoryName
} from '../../category-identifiers';

const STRUCTURAL_CATEGORY_KIND = 'structural';

interface CategoryCreateDialogProps {
  categories: CatalogCategorySummary[];
  onCreateCategory: (payload: CreateCatalogCategoryPayload) => Promise<void>;
}

export function CategoryCreateDialog({
  categories,
  onCreateCategory
}: CategoryCreateDialogProps) {
  const t = useTranslations('product');
  const [open, setOpen] = React.useState(false);
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [name, setName] = React.useState('');
  const [code, setCode] = React.useState('');
  const [slug, setSlug] = React.useState('');
  const [parentId, setParentId] = React.useState<string>('none');
  const [kind, setKind] = React.useState<string>(STRUCTURAL_CATEGORY_KIND);
  const [ruleJson, setRuleJson] = React.useState('');
  const [description, setDescription] = React.useState('');
  const slugWasEdited = React.useRef(false);
  const codeWasEdited = React.useRef(false);
  const categoryOptions = React.useMemo(
    () =>
      buildCategoryHierarchyOptions(categories).map(({ id, label }) => ({
        id,
        label
      })),
    [categories]
  );

  const handleNameChange = (val: string) => {
    setName(val);
    if (!slugWasEdited.current) setSlug(slugifyCategoryName(val));
    if (!codeWasEdited.current) setCode(categoryCodeFromName(val));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.error('Category name is required');
      return;
    }
    if (!code.trim()) {
      toast.error('Category code is required');
      return;
    }
    if (!slug.trim()) {
      toast.error('Category slug is required');
      return;
    }

    setIsSubmitting(true);
    try {
      await onCreateCategory({
        name: name.trim(),
        code: code.trim(),
        slug: slug.trim(),
        parentId: parentId === 'none' ? null : parentId,
        kind,
        ruleJson: kind === 'virtual' ? ruleJson : undefined,
        description: description.trim() || undefined
      });
      toast.success('Category created successfully');
      setOpen(false);
      setName('');
      setCode('');
      setSlug('');
      setParentId('none');
      setKind(STRUCTURAL_CATEGORY_KIND);
      setRuleJson('');
      setDescription('');
      slugWasEdited.current = false;
      codeWasEdited.current = false;
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to create category'
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus className='mr-1.5 h-4 w-4' />
          Add Category
        </Button>
      </DialogTrigger>
      <DialogContent className='sm:max-w-[500px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Create Catalog Category</DialogTitle>
            <DialogDescription>
              Add a new category to the catalog hierarchy. Categories organize
              products and determine dynamic attribute schemas.
            </DialogDescription>
          </DialogHeader>
          <div className='grid gap-4 py-4'>
            <div className='space-y-1.5'>
              <Label htmlFor='cat-name'>Name *</Label>
              <Input
                id='cat-name'
                value={name}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder='e.g. Mechanical Keyboards'
                required
                disabled={isSubmitting}
              />
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='cat-code'>Code *</Label>
                <Input
                  id='cat-code'
                  value={code}
                  onChange={(e) => {
                    const value = e.target.value;
                    codeWasEdited.current = value.trim().length > 0;
                    setCode(value);
                  }}
                  placeholder='e.g. keyboards_mechanical'
                  required
                  disabled={isSubmitting}
                />
              </div>

              <div className='space-y-1.5'>
                <Label htmlFor='cat-slug'>Slug *</Label>
                <Input
                  id='cat-slug'
                  value={slug}
                  onChange={(e) => {
                    const value = e.target.value;
                    slugWasEdited.current = value.trim().length > 0;
                    setSlug(value);
                  }}
                  placeholder='e.g. mechanical-keyboards'
                  required
                  disabled={isSubmitting}
                />
              </div>
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label htmlFor='cat-parent'>Parent Category</Label>
                <SearchableCategorySelect
                  id='cat-parent'
                  options={categoryOptions}
                  value={parentId}
                  emptyValue='none'
                  emptyLabel='None (Root category)'
                  searchPlaceholder='Search parent category...'
                  disabled={isSubmitting}
                  onChange={setParentId}
                />
              </div>

              <div className='space-y-1.5'>
                <Label>Kind</Label>
                <Select
                  value={kind}
                  onValueChange={setKind}
                  disabled={isSubmitting}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={STRUCTURAL_CATEGORY_KIND}>
                      Structural
                    </SelectItem>
                    <SelectItem value='virtual'>
                      Virtual (Rule-based)
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            {kind === 'virtual' && (
              <div className='space-y-1.5'>
                <Label htmlFor='cat-rule'>{t('categoryRuleLabel')}</Label>
                <Textarea
                  id='cat-rule'
                  aria-describedby='cat-rule-help'
                  className='font-mono'
                  value={ruleJson}
                  onChange={(event) => setRuleJson(event.target.value)}
                  rows={5}
                  disabled={isSubmitting}
                  required
                />
                <p id='cat-rule-help' className='text-muted-foreground text-xs'>
                  {t('categoryRuleHint')}{' '}
                  <code>
                    {'{"version":1,"statuses":["active"],"in_stock":true}'}
                  </code>
                </p>
              </div>
            )}

            <div className='space-y-1.5'>
              <Label htmlFor='cat-desc'>Description</Label>
              <Textarea
                id='cat-desc'
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder='Optional description for operators or storefront navigation...'
                rows={3}
                disabled={isSubmitting}
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              type='button'
              variant='outline'
              onClick={() => setOpen(false)}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
            <Button type='submit' disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className='mr-1.5 h-4 w-4 animate-spin' />
                  Creating...
                </>
              ) : (
                'Create Category'
              )}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
