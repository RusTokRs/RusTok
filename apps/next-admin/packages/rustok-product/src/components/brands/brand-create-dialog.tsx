/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '@/shared/ui/shadcn/dialog';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Textarea } from '@/shared/ui/shadcn/textarea';
import { Plus } from 'lucide-react';
import type { CreateBrandPayload } from '../../api/types';

interface BrandCreateDialogProps {
  onCreateBrand: (payload: CreateBrandPayload) => Promise<void>;
}

export function BrandCreateDialog({ onCreateBrand }: BrandCreateDialogProps) {
  const [open, setOpen] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const [name, setName] = React.useState('');
  const [slug, setSlug] = React.useState('');
  const [description, setDescription] = React.useState('');
  const [websiteUrl, setWebsiteUrl] = React.useState('');

  const handleNameChange = (val: string) => {
    setName(val);
    if (!slug || slug === autoSlug(name)) {
      setSlug(autoSlug(val));
    }
  };

  const autoSlug = (text: string) =>
    text
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9_-]+/g, '-')
      .replace(/^-|-$/g, '');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Brand name is required');
      return;
    }
    if (!slug.trim()) {
      setError('Slug is required');
      return;
    }

    try {
      setBusy(true);
      setError(null);
      await onCreateBrand({
        name: name.trim(),
        slug: slug.trim(),
        description: description.trim() || null,
        websiteUrl: websiteUrl.trim() || null,
        isActive: true
      });
      setName('');
      setSlug('');
      setDescription('');
      setWebsiteUrl('');
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create brand');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size='sm' className='gap-1.5'>
          <Plus className='h-4 w-4' />
          Add Brand
        </Button>
      </DialogTrigger>
      <DialogContent className='sm:max-w-[480px]'>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Create Brand</DialogTitle>
            <DialogDescription>
              Register a new manufacturer or brand partner for your catalog.
            </DialogDescription>
          </DialogHeader>

          <div className='grid gap-4 py-4'>
            {error && (
              <div className='bg-destructive/15 text-destructive rounded-md p-3 text-sm'>
                {error}
              </div>
            )}

            <div className='grid gap-2'>
              <Label htmlFor='brand-name'>Brand Name</Label>
              <Input
                id='brand-name'
                value={name}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder='e.g. Acme Corp'
                required
              />
            </div>

            <div className='grid gap-2'>
              <Label htmlFor='brand-slug'>Slug</Label>
              <Input
                id='brand-slug'
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder='e.g. acme-corp'
                required
              />
            </div>

            <div className='grid gap-2'>
              <Label htmlFor='brand-desc'>Description</Label>
              <Textarea
                id='brand-desc'
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder='Brief description of the brand'
                rows={3}
              />
            </div>

            <div className='grid gap-2'>
              <Label htmlFor='brand-url'>Website URL</Label>
              <Input
                id='brand-url'
                type='url'
                value={websiteUrl}
                onChange={(e) => setWebsiteUrl(e.target.value)}
                placeholder='https://www.example.com'
              />
            </div>
          </div>

          <DialogFooter>
            <Button
              type='button'
              variant='outline'
              onClick={() => setOpen(false)}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button type='submit' disabled={busy}>
              {busy ? 'Creating...' : 'Create Brand'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
