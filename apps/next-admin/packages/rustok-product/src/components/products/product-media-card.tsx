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
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/shared/ui/shadcn/card';
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
import { ImageIcon, Plus, Trash2, ArrowLeft, ArrowRight, ExternalLink } from 'lucide-react';
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
    <Card className='rounded-2xl border-border shadow-sm'>
      <CardHeader className='pb-4 border-b border-border/60'>
        <div className='flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3'>
          <div className='flex items-center gap-2'>
            <ImageIcon className='h-4 w-4 text-primary' />
            <div>
              <CardTitle className='text-sm font-semibold'>Media Gallery</CardTitle>
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
              className='h-8 px-3 rounded-xl text-xs gap-1.5 self-start sm:self-auto'
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
          <div className='flex flex-col items-center justify-center py-8 text-center text-muted-foreground gap-2 bg-muted/20 rounded-xl border border-dashed border-border/80'>
            <ImageIcon className='h-8 w-8 opacity-40' />
            <p className='text-xs font-medium'>No product images added yet</p>
            <p className='text-[11px] max-w-xs opacity-75'>
              Add media assets by ID or URL to showcase your product in the catalog.
            </p>
          </div>
        ) : (
          <div className='grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-4'>
            {sortedImages.map((img, index) => (
              <div
                key={img.id}
                className='group relative rounded-xl border border-border bg-card overflow-hidden shadow-xs flex flex-col justify-between'
              >
                {/* Image Display */}
                <div className='aspect-square bg-muted/40 relative flex items-center justify-center overflow-hidden'>
                  {img.url ? (
                    <img
                      src={img.url}
                      alt={img.altText || 'Product image'}
                      className='object-cover w-full h-full'
                    />
                  ) : (
                    <div className='flex flex-col items-center gap-1 text-muted-foreground/60'>
                      <ImageIcon className='h-6 w-6' />
                      <span className='text-[10px] font-mono'>Media ID: {img.mediaId.slice(0, 8)}</span>
                    </div>
                  )}

                  {/* Position badge */}
                  <span className='absolute top-2 left-2 px-1.5 py-0.5 rounded text-[10px] font-bold bg-background/80 backdrop-blur-xs text-foreground border border-border/60'>
                    #{index + 1}
                  </span>
                </div>

                {/* Footer Controls */}
                <div className='p-2 flex items-center justify-between gap-1 border-t border-border bg-muted/10'>
                  <div className='flex items-center gap-0.5'>
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon'
                      disabled={index === 0 || disabled}
                      onClick={() => handleMove(index, 'prev')}
                      className='h-6 w-6 rounded text-muted-foreground hover:text-foreground'
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
                      className='h-6 w-6 rounded text-muted-foreground hover:text-foreground'
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
                      className='h-6 w-6 rounded text-rose-500 hover:text-rose-700 hover:bg-rose-50 dark:hover:bg-rose-950/40'
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
        <DialogContent className='sm:max-w-md rounded-2xl'>
          <DialogHeader>
            <DialogTitle className='text-sm font-semibold'>Add Product Media</DialogTitle>
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
                className='h-9 text-xs rounded-xl font-mono'
              />
            </div>
            <div className='space-y-1.5'>
              <Label htmlFor='img-alt'>Alt Text (Accessibility / SEO)</Label>
              <Input
                id='img-alt'
                value={altTextInput}
                onChange={(e) => setAltTextInput(e.target.value)}
                placeholder='Front view of keyboard with RGB lighting'
                className='h-9 text-xs rounded-xl'
              />
            </div>
          </div>

          <DialogFooter className='gap-2 sm:gap-0'>
            <Button
              type='button'
              variant='outline'
              onClick={() => setDialogOpen(false)}
              className='h-9 text-xs rounded-xl'
            >
              Cancel
            </Button>
            <Button
              type='button'
              onClick={handleAddImage}
              disabled={isBusy || !mediaIdInput.trim()}
              className='h-9 text-xs rounded-xl font-semibold'
            >
              {isBusy ? 'Adding...' : 'Attach Image'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
