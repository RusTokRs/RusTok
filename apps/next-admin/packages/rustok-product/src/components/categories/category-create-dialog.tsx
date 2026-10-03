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
import type { CatalogCategorySummary } from '../../api/types';

interface CategoryCreateDialogProps {
  categories: CatalogCategorySummary[];
  onCreateCategory: (payload: {
    name: string;
    code: string;
    slug: string;
    parentId?: string | null;
    kind?: string;
    description?: string;
  }) => Promise<void>;
}

export function CategoryCreateDialog({
  categories,
  onCreateCategory
}: CategoryCreateDialogProps) {
  const [open, setOpen] = React.useState(false);
  const [isSubmitting, setIsSubmitting] = React.useState(false);
  const [name, setName] = React.useState('');
  const [code, setCode] = React.useState('');
  const [slug, setSlug] = React.useState('');
  const [parentId, setParentId] = React.useState<string>('none');
  const [kind, setKind] = React.useState('standard');
  const [description, setDescription] = React.useState('');

  const handleNameChange = (val: string) => {
    setName(val);
    const generatedSlug = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');
    const generatedCode = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');
    if (!slug || slug === val.slice(0, -1).toLowerCase().replace(/[^a-z0-9]+/g, '-')) {
      setSlug(generatedSlug);
    }
    if (!code || code === val.slice(0, -1).toLowerCase().replace(/[^a-z0-9]+/g, '_')) {
      setCode(generatedCode);
    }
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
        description: description.trim() || undefined
      });
      toast.success('Category created successfully');
      setOpen(false);
      setName('');
      setCode('');
      setSlug('');
      setParentId('none');
      setDescription('');
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to create category');
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
              Add a new category to the catalog hierarchy. Categories organize products and determine dynamic attribute schemas.
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
                  onChange={(e) => setCode(e.target.value)}
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
                  onChange={(e) => setSlug(e.target.value)}
                  placeholder='e.g. mechanical-keyboards'
                  required
                  disabled={isSubmitting}
                />
              </div>
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-1.5'>
                <Label>Parent Category</Label>
                <Select value={parentId} onValueChange={setParentId} disabled={isSubmitting}>
                  <SelectTrigger>
                    <SelectValue placeholder='Select parent...' />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='none'>None (Root category)</SelectItem>
                    {categories.map((c) => (
                      <SelectItem key={c.id} value={c.id}>
                        {c.path || c.name || c.code}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className='space-y-1.5'>
                <Label>Kind</Label>
                <Select value={kind} onValueChange={setKind} disabled={isSubmitting}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='standard'>Standard</SelectItem>
                    <SelectItem value='virtual'>Virtual (Rule-based)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

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
