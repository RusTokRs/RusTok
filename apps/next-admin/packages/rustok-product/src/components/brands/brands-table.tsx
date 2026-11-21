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
import { toast } from 'sonner';
import { DataTableStatic } from '@/widgets/data-table/data-table-static';
import type { DataTableStaticColumn } from '@/widgets/data-table/data-table-static';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { ExternalLink, Search, Trash2 } from 'lucide-react';
import type { BrandSummary } from '../../api/types';

interface BrandsTableProps {
  brands: BrandSummary[];
  onDeleteBrand?: (id: string) => Promise<void>;
}

export function BrandsTable({ brands, onDeleteBrand }: BrandsTableProps) {
  const [deletingId, setDeletingId] = React.useState<string | null>(null);
  const [search, setSearch] = React.useState('');

  const filteredBrands = React.useMemo(() => {
    if (!search.trim()) return brands;
    const query = search.toLowerCase().trim();
    return brands.filter(
      (brand) =>
        brand.name.toLowerCase().includes(query) ||
        brand.slug.toLowerCase().includes(query) ||
        (brand.description?.toLowerCase().includes(query) ?? false)
    );
  }, [brands, search]);

  const handleDelete = async (id: string) => {
    if (!onDeleteBrand) return;
    if (!window.confirm('Are you sure you want to delete this brand?')) return;
    try {
      setDeletingId(id);
      await onDeleteBrand(id);
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : 'Failed to delete brand.'
      );
    } finally {
      setDeletingId(null);
    }
  };

  const columns: DataTableStaticColumn<BrandSummary>[] = [
    {
      id: 'brand',
      header: 'Brand',
      cell: (brand) => (
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
            <div className='font-medium'>{brand.name}</div>
            {brand.description && (
              <div className='text-muted-foreground line-clamp-1 text-xs'>
                {brand.description}
              </div>
            )}
          </div>
        </div>
      )
    },
    {
      id: 'slug',
      header: 'Slug',
      cellClassName: 'text-muted-foreground font-mono text-xs',
      cell: (brand) => brand.slug
    },
    {
      id: 'website',
      header: 'Website',
      cellClassName: 'text-xs',
      cell: (brand) =>
        brand.websiteUrl ? (
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
        )
    },
    {
      id: 'status',
      header: 'Status',
      headerClassName: 'text-center',
      cellClassName: 'text-center',
      cell: (brand) => (
        <Badge variant={brand.isActive ? 'default' : 'secondary'}>
          {brand.isActive ? 'Active' : 'Inactive'}
        </Badge>
      )
    },
    {
      id: 'products',
      header: 'Products',
      headerClassName: 'text-right',
      cellClassName: 'text-right font-mono text-xs',
      cell: (brand) => brand.productsCount ?? 0
    }
  ];

  if (onDeleteBrand) {
    columns.push({
      id: 'actions',
      header: 'Actions',
      headerClassName: 'text-right',
      cellClassName: 'text-right',
      cell: (brand) => (
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
      )
    });
  }

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-3'>
        <div className='relative max-w-sm flex-1'>
          <Search className='text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2' />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder='Filter brands by name, slug or description...'
            className='pl-9'
          />
        </div>
        <p className='text-muted-foreground ml-auto text-xs'>
          Total brands:{' '}
          <span className='text-foreground font-semibold'>{brands.length}</span>
        </p>
      </div>

      <DataTableStatic
        rows={filteredBrands}
        columns={columns}
        getRowKey={(brand) => brand.id}
        emptyState={
          <div className='text-muted-foreground py-8 text-center text-sm'>
            No brands found. Create your first brand to get started.
          </div>
        }
      />
    </div>
  );
}
