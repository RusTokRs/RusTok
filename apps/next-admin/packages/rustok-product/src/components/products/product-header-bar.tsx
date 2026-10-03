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
import Link from 'next/link';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/shared/ui/shadcn/dropdown-menu';
import {
  ArrowLeft,
  Check,
  ChevronDown,
  Globe,
  Loader2,
  Trash2,
  Archive,
  FileEdit
} from 'lucide-react';

interface ProductHeaderBarProps {
  title: string;
  status: string;
  isNew: boolean;
  isSaving: boolean;
  onSave: () => void;
  onStatusChange?: (status: string) => void;
  onDelete?: () => void;
}

export function ProductHeaderBar({
  title,
  status,
  isNew,
  isSaving,
  onSave,
  onStatusChange,
  onDelete
}: ProductHeaderBarProps) {
  const normStatus = (status || 'draft').toUpperCase();

  const getStatusBadge = () => {
    switch (normStatus) {
      case 'ACTIVE':
      case 'PUBLISHED':
        return (
          <Badge className='bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30 font-semibold'>
            Active
          </Badge>
        );
      case 'ARCHIVED':
        return (
          <Badge variant='secondary' className='text-muted-foreground'>
            Archived
          </Badge>
        );
      default:
        return (
          <Badge variant='outline' className='text-amber-600 dark:text-amber-400 border-amber-500/30 bg-amber-500/10'>
            Draft
          </Badge>
        );
    }
  };

  return (
    <div className='flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between bg-card rounded-2xl border border-border p-4 shadow-sm'>
      <div className='flex items-center gap-3'>
        <Button asChild variant='outline' size='icon' className='h-9 w-9 rounded-xl'>
          <Link href='/dashboard/product' title='Back to Catalog'>
            <ArrowLeft className='h-4 w-4' />
          </Link>
        </Button>
        <div>
          <div className='flex items-center gap-2'>
            <h1 className='text-lg font-bold tracking-tight text-foreground truncate max-w-md'>
              {title || (isNew ? 'New Product' : 'Untitled Product')}
            </h1>
            {!isNew && getStatusBadge()}
          </div>
          <p className='text-xs text-muted-foreground mt-0.5'>
            {isNew
              ? 'Configure catalog entity, initial variant, and effective category attributes.'
              : 'Product catalog specification, variants matrix, localized media, and lifecycle.'}
          </p>
        </div>
      </div>

      <div className='flex items-center gap-2 self-end sm:self-auto'>
        {!isNew && onStatusChange && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant='outline' size='sm' className='h-9 rounded-xl text-xs gap-1.5' disabled={isSaving}>
                <span>Status: {normStatus}</span>
                <ChevronDown className='h-3.5 w-3.5 opacity-60' />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align='end' className='w-48'>
              <DropdownMenuItem
                onClick={() => onStatusChange('ACTIVE')}
                className='gap-2 text-xs font-medium text-emerald-600 dark:text-emerald-400 cursor-pointer'
              >
                <Globe className='h-3.5 w-3.5' />
                <span>Publish (Active)</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={() => onStatusChange('DRAFT')}
                className='gap-2 text-xs font-medium cursor-pointer'
              >
                <FileEdit className='h-3.5 w-3.5' />
                <span>Move to Draft</span>
              </DropdownMenuItem>
              <DropdownMenuItem
                onClick={() => onStatusChange('ARCHIVED')}
                className='gap-2 text-xs font-medium cursor-pointer'
              >
                <Archive className='h-3.5 w-3.5' />
                <span>Archive</span>
              </DropdownMenuItem>
              {onDelete && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    onClick={onDelete}
                    className='gap-2 text-xs font-medium text-destructive cursor-pointer'
                  >
                    <Trash2 className='h-3.5 w-3.5' />
                    <span>Delete Product</span>
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        )}

        <Button
          onClick={onSave}
          disabled={isSaving}
          className='h-9 px-4 rounded-xl text-xs font-semibold gap-1.5 shadow-sm'
        >
          {isSaving ? (
            <>
              <Loader2 className='h-3.5 w-3.5 animate-spin' />
              <span>Saving...</span>
            </>
          ) : (
            <>
              <Check className='h-3.5 w-3.5' />
              <span>{isNew ? 'Create Product' : 'Save Changes'}</span>
            </>
          )}
        </Button>
      </div>
    </div>
  );
}
