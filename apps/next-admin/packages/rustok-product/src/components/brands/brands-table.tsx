/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table/table';
import { ExternalLink, Trash2 } from 'lucide-react';
import type { BrandSummary } from '../../api/types';

interface BrandsTableProps {
  brands: BrandSummary[];
  onDeleteBrand?: (id: string) => Promise<void>;
}

export function BrandsTable({ brands, onDeleteBrand }: BrandsTableProps) {
  const [deletingId, setDeletingId] = React.useState<string | null>(null);

  const handleDelete = async (id: string) => {
    if (!onDeleteBrand) return;
    if (!window.confirm('Are you sure you want to delete this brand?')) return;
    try {
      setDeletingId(id);
      await onDeleteBrand(id);
    } finally {
      setDeletingId(null);
    }
  };

  if (brands.length === 0) {
    return (
      <div className='text-muted-foreground py-8 text-center text-sm'>
        No brands found. Create your first brand to get started.
      </div>
    );
  }

  return (
    <div className='rounded-md border'>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Brand</TableHead>
            <TableHead>Slug</TableHead>
            <TableHead>Website</TableHead>
            <TableHead className='text-center'>Status</TableHead>
            <TableHead className='text-right'>Products</TableHead>
            {onDeleteBrand && (
              <TableHead className='text-right'>Actions</TableHead>
            )}
          </TableRow>
        </TableHeader>
        <TableBody>
          {brands.map((brand) => (
            <TableRow key={brand.id}>
              <TableCell className='font-medium'>
                <div className='flex items-center gap-2'>
                  {brand.logoUrl ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={brand.logoUrl}
                      alt={brand.name}
                      className='bg-muted h-7 w-7 rounded object-contain p-0.5'
                    />
                  ) : (
                    <div className='bg-primary/10 text-primary flex h-7 w-7 items-center justify-center rounded text-xs font-bold'>
                      {brand.name.charAt(0).toUpperCase()}
                    </div>
                  )}
                  <div>
                    <div>{brand.name}</div>
                    {brand.description && (
                      <div className='text-muted-foreground line-clamp-1 text-xs'>
                        {brand.description}
                      </div>
                    )}
                  </div>
                </div>
              </TableCell>
              <TableCell className='text-muted-foreground font-mono text-xs'>
                {brand.slug}
              </TableCell>
              <TableCell className='text-xs'>
                {brand.websiteUrl ? (
                  <a
                    href={brand.websiteUrl}
                    target='_blank'
                    rel='noreferrer'
                    className='text-primary inline-flex items-center gap-1 hover:underline'
                  >
                    Visit <ExternalLink className='h-3 w-3' />
                  </a>
                ) : (
                  <span className='text-muted-foreground'>—</span>
                )}
              </TableCell>
              <TableCell className='text-center'>
                <Badge variant={brand.isActive ? 'default' : 'secondary'}>
                  {brand.isActive ? 'Active' : 'Inactive'}
                </Badge>
              </TableCell>
              <TableCell className='text-right font-mono text-xs'>
                {brand.productsCount ?? 0}
              </TableCell>
              {onDeleteBrand && (
                <TableCell className='text-right'>
                  <Button
                    variant='ghost'
                    size='sm'
                    className='text-destructive hover:bg-destructive/10 h-8 w-8 p-0'
                    disabled={deletingId === brand.id}
                    onClick={() => handleDelete(brand.id)}
                  >
                    <Trash2 className='h-4 w-4' />
                    <span className='sr-only'>Delete</span>
                  </Button>
                </TableCell>
              )}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
