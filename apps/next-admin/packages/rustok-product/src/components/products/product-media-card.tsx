/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use client';

import * as React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/shared/ui/shadcn/dialog';
import {
  ImageIcon,
  Plus,
  Trash2,
  ArrowLeft,
  ArrowRight,
  ExternalLink
} from 'lucide-react';
import type { ProductImage } from '../../api/types';

interface ProductMediaCardProps {
  images: ProductImage[];
  onAddImage?: (input: { mediaId: string; altText?: string }) => Promise<void>;
  onDeleteImage?: (id: string) => Promise<void>;
  onReorderImages?: (imageIds: string[]) => Promise<void>;
  disabled?: boolean;
}

export function ProductMediaCard({
  images,
  onAddImage,
  onDeleteImage,
  onReorderImages,
  disabled = false
}: ProductMediaCardProps) {
  const [dialogOpen, setDialogOpen] = React.useState(false);
  const [mediaIdInput, setMediaIdInput] = React.useState('');
  const [altTextInput, setAltTextInput] = React.useState('');
  const [isBusy, setIsBusy] = React.useState(false);

  const sortedImages = React.useMemo(() => {
    return [...images].sort((a, b) => a.position - b.position);
  }, [images]);

  const handleAddImage = async () => {
    if (!mediaIdInput.trim() || !onAddImage) return;
    setIsBusy(true);
    try {
      await onAddImage({
        mediaId: mediaIdInput.trim(),
        altText: altTextInput.trim() || undefined
      });
      setMediaIdInput('');
      setAltTextInput('');
      setDialogOpen(false);
    } finally {
      setIsBusy(false);
    }
  };

  const handleMove = async (index: number, direction: 'prev' | 'next') => {
    if (!onReorderImages) return;
    const targetIndex = direction === 'prev' ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= sortedImages.length) return;

    const reordered = [...sortedImages];
    const [moved] = reordered.splice(index, 1);
    reordered.splice(targetIndex, 0, moved);

    await onReorderImages(reordered.map((img) => img.id));
  };

  return (
    <Card className='border-border rounded-2xl shadow-sm'>
      <CardHeader className='border-border/60 border-b pb-4'>
        <div className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between'>
          <div className='flex items-center gap-2'>
            <ImageIcon className='text-primary h-4 w-4' />
            <div>
              <CardTitle className='text-sm font-semibold'>
                Media Gallery
              </CardTitle>
              <CardDescription className='text-xs'>
                Product photos, asset attachments, and localized alt texts.
              </CardDescription>
            </div>
          </div>
          {onAddImage && (
            <Button
              type='button'
              size='sm'
              onClick={() => setDialogOpen(true)}
              className='h-8 gap-1.5 self-start rounded-xl px-3 text-xs sm:self-auto'
              disabled={disabled}
            >
              <Plus className='h-3.5 w-3.5' />
              <span>Add Image</span>
            </Button>
          )}
        </div>
      </CardHeader>
      <CardContent className='pt-5'>
        {sortedImages.length === 0 ? (
          <div className='text-muted-foreground bg-muted/20 border-border/80 flex flex-col items-center justify-center gap-2 rounded-xl border border-dashed py-8 text-center'>
            <ImageIcon className='h-8 w-8 opacity-40' />
            <p className='text-xs font-medium'>No product images added yet</p>
            <p className='max-w-xs text-[11px] opacity-75'>
              Add media assets by ID or URL to showcase your product in the
              catalog.
            </p>
          </div>
        ) : (
          <div className='grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4'>
            {sortedImages.map((img, index) => (
              <div
                key={img.id}
                className='group border-border bg-card relative flex flex-col justify-between overflow-hidden rounded-xl border shadow-xs'
              >
                {/* Image Display */}
                <div className='bg-muted/40 relative flex aspect-square items-center justify-center overflow-hidden'>
                  {img.url ? (
                    <img
                      src={img.url}
                      alt={img.altText || 'Product image'}
                      className='h-full w-full object-cover'
                    />
                  ) : (
                    <div className='text-muted-foreground/60 flex flex-col items-center gap-1'>
                      <ImageIcon className='h-6 w-6' />
                      <span className='font-mono text-[10px]'>
                        Media ID: {img.mediaId.slice(0, 8)}
                      </span>
                    </div>
                  )}

                  {/* Position badge */}
                  <span className='bg-background/80 text-foreground border-border/60 absolute top-2 left-2 rounded border px-1.5 py-0.5 text-[10px] font-bold backdrop-blur-xs'>
                    #{index + 1}
                  </span>
                </div>

                {/* Footer Controls */}
                <div className='border-border bg-muted/10 flex items-center justify-between gap-1 border-t p-2'>
                  <div className='flex items-center gap-0.5'>
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon'
                      disabled={index === 0 || disabled}
                      onClick={() => handleMove(index, 'prev')}
                      className='text-muted-foreground hover:text-foreground h-6 w-6 rounded'
                      title='Move left'
                    >
                      <ArrowLeft className='h-3 w-3' />
                    </Button>
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon'
                      disabled={index === sortedImages.length - 1 || disabled}
                      onClick={() => handleMove(index, 'next')}
                      className='text-muted-foreground hover:text-foreground h-6 w-6 rounded'
                      title='Move right'
                    >
                      <ArrowRight className='h-3 w-3' />
                    </Button>
                  </div>

                  {onDeleteImage && (
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon'
                      disabled={disabled}
                      onClick={() => onDeleteImage(img.id)}
                      className='h-6 w-6 rounded text-rose-500 hover:bg-rose-50 hover:text-rose-700 dark:hover:bg-rose-950/40'
                      title='Remove image'
                    >
                      <Trash2 className='h-3 w-3' />
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>

      {/* Add Image Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className='rounded-2xl sm:max-w-md'>
          <DialogHeader>
            <DialogTitle className='text-sm font-semibold'>
              Add Product Media
            </DialogTitle>
            <DialogDescription className='text-xs'>
              Enter a Media module asset ID or direct image URL.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-2 text-xs'>
            <div className='space-y-1.5'>
              <Label htmlFor='img-media-id'>Media ID or Image URL *</Label>
              <Input
                id='img-media-id'
                value={mediaIdInput}
                onChange={(e) => setMediaIdInput(e.target.value)}
                placeholder='e.g. 550e8400-e29b-41d4-a716-446655440000 or https://...'
                className='h-9 rounded-xl font-mono text-xs'
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='img-alt'>Alt Text (Accessibility / SEO)</Label>
              <Input
                id='img-alt'
                value={altTextInput}
                onChange={(e) => setAltTextInput(e.target.value)}
                placeholder='Front view of keyboard with RGB lighting'
                className='h-9 rounded-xl text-xs'
              />
            </div>
          </div>

          <DialogFooter className='gap-2 sm:gap-0'>
            <Button
              type='button'
              variant='outline'
              onClick={() => setDialogOpen(false)}
              className='h-9 rounded-xl text-xs'
            >
              Cancel
            </Button>
            <Button
              type='button'
              onClick={handleAddImage}
              disabled={isBusy || !mediaIdInput.trim()}
              className='h-9 rounded-xl text-xs font-semibold'
            >
              {isBusy ? 'Adding...' : 'Attach Image'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
