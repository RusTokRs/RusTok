/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import Link from 'next/link';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Input } from '@/shared/ui/shadcn/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import {
  Boxes,
  Search,
  Trash2,
  ExternalLink,
  Plus,
  Layers
} from 'lucide-react';
import type { ProductBundle } from '../../api/types';

interface BundlesTableProps {
  bundles: ProductBundle[];
  total: number;
  page: number;
  perPage: number;
  onPageChange?: (page: number) => void;
  onSearchChange?: (search: string) => void;
  onStatusFilterChange?: (status: string) => void;
  onTypeFilterChange?: (type: string) => void;
  onDeleteBundle?: (id: string) => Promise<void>;
  onCreateClick?: () => void;
  disabled?: boolean;
}

export function BundlesTable({
  bundles,
  total,
  page,
  perPage,
  onPageChange,
  onSearchChange,
  onStatusFilterChange,
  onTypeFilterChange,
  onDeleteBundle,
  onCreateClick,
  disabled
}: BundlesTableProps) {
  const [search, setSearch] = React.useState('');
  const [selectedStatus, setSelectedStatus] = React.useState('all');
  const [selectedType, setSelectedType] = React.useState('all');

  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setSearch(val);
    onSearchChange?.(val);
  };

  const handleStatusChange = (val: string) => {
    setSelectedStatus(val);
    onStatusFilterChange?.(val === 'all' ? '' : val);
  };

  const handleTypeChange = (val: string) => {
    setSelectedType(val);
    onTypeFilterChange?.(val === 'all' ? '' : val);
  };

  const totalPages = Math.ceil(total / perPage) || 1;

  return (
    <div className='space-y-4'>
      {/* Search & Filters Bar */}
      <div className='flex flex-wrap items-center justify-between gap-3'>
        <div className='flex max-w-md min-w-[280px] flex-1 items-center gap-2'>
          <div className='relative w-full'>
            <Search className='text-muted-foreground absolute top-2.5 left-3 h-4 w-4' />
            <Input
              placeholder='Search bundles by name or slug...'
              value={search}
              onChange={handleSearchChange}
              className='h-9 pl-9 text-xs'
              disabled={disabled}
            />
          </div>
        </div>

        <div className='flex items-center gap-2'>
          <Select
            value={selectedType}
            onValueChange={handleTypeChange}
            disabled={disabled}
          >
            <SelectTrigger className='h-9 w-[150px] text-xs'>
              <SelectValue placeholder='All Types' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='all'>All Types</SelectItem>
              <SelectItem value='fixed'>Fixed Composition</SelectItem>
              <SelectItem value='flexible'>Flexible Kit</SelectItem>
            </SelectContent>
          </Select>

          <Select
            value={selectedStatus}
            onValueChange={handleStatusChange}
            disabled={disabled}
          >
            <SelectTrigger className='h-9 w-[130px] text-xs'>
              <SelectValue placeholder='All Statuses' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='all'>All Statuses</SelectItem>
              <SelectItem value='active'>Active</SelectItem>
              <SelectItem value='draft'>Draft</SelectItem>
              <SelectItem value='archived'>Archived</SelectItem>
            </SelectContent>
          </Select>

          {onCreateClick && (
            <Button
              size='sm'
              onClick={onCreateClick}
              className='h-9 gap-1.5 text-xs'
              disabled={disabled}
            >
              <Plus className='h-3.5 w-3.5' />
              New Bundle
            </Button>
          )}
        </div>
      </div>

      {/* Bundles Table */}
      <div className='bg-card rounded-md border'>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className='w-[35%]'>Bundle Name & Slug</TableHead>
              <TableHead>Type</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className='text-center'>Items</TableHead>
              <TableHead>Discount</TableHead>
              <TableHead className='text-right'>Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {bundles.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={6}
                  className='text-muted-foreground h-36 text-center text-sm'
                >
                  <div className='flex flex-col items-center justify-center gap-2'>
                    <Boxes className='text-muted-foreground/60 h-8 w-8' />
                    <p className='font-medium'>No bundles found</p>
                    <p className='text-muted-foreground text-xs'>
                      Create the first product bundle or kit for your store
                      catalog.
                    </p>
                    {onCreateClick && (
                      <Button
                        size='sm'
                        variant='outline'
                        onClick={onCreateClick}
                        className='mt-2 gap-1 text-xs'
                      >
                        <Plus className='h-3.5 w-3.5' />
                        Create First Bundle
                      </Button>
                    )}
                  </div>
                </TableCell>
              </TableRow>
            ) : (
              bundles.map((bundle) => {
                const itemsCount = bundle.items?.length ?? 0;

                return (
                  <TableRow key={bundle.id}>
                    <TableCell>
                      <div className='flex items-center gap-3'>
                        <div className='bg-primary/10 text-primary flex h-9 w-9 shrink-0 items-center justify-center rounded'>
                          <Boxes className='h-4 w-4' />
                        </div>
                        <div className='min-w-0'>
                          <p className='truncate text-sm font-medium'>
                            {bundle.name}
                          </p>
                          <p className='text-muted-foreground truncate font-mono text-xs'>
                            /{bundle.slug}
                          </p>
                        </div>
                      </div>
                    </TableCell>

                    <TableCell>
                      <Badge variant='outline' className='text-xs font-normal'>
                        {bundle.bundleType === 'fixed'
                          ? 'Fixed Kit'
                          : 'Flexible Set'}
                      </Badge>
                    </TableCell>

                    <TableCell>
                      <Badge
                        variant={
                          bundle.status === 'active'
                            ? 'default'
                            : bundle.status === 'draft'
                              ? 'secondary'
                              : 'outline'
                        }
                        className='text-xs font-normal capitalize'
                      >
                        {bundle.status}
                      </Badge>
                    </TableCell>

                    <TableCell className='text-center font-mono text-xs'>
                      <span className='bg-muted inline-flex items-center gap-1 rounded px-2 py-0.5 font-semibold'>
                        <Layers className='text-muted-foreground h-3 w-3' />
                        {itemsCount}
                      </span>
                    </TableCell>

                    <TableCell>
                      {bundle.discountType === 'percentage' &&
                      parseFloat(bundle.discountValue) > 0 ? (
                        <span className='inline-flex items-center rounded-sm bg-emerald-500/10 px-2 py-0.5 font-mono text-xs font-medium text-emerald-700 dark:text-emerald-400'>
                          -{bundle.discountValue}%
                        </span>
                      ) : bundle.discountType === 'fixed_amount' &&
                        parseFloat(bundle.discountValue) > 0 ? (
                        <span className='inline-flex items-center rounded-sm bg-blue-500/10 px-2 py-0.5 font-mono text-xs font-medium text-blue-700 dark:text-blue-400'>
                          -{bundle.discountValue}
                        </span>
                      ) : (
                        <span className='text-muted-foreground text-xs'>
                          None
                        </span>
                      )}
                    </TableCell>

                    <TableCell className='text-right'>
                      <div className='flex items-center justify-end gap-1'>
                        {bundle.bundleProductId && (
                          <Button
                            asChild
                            variant='ghost'
                            size='icon'
                            className='text-muted-foreground hover:text-foreground h-8 w-8'
                          >
                            <Link
                              href={`/dashboard/product/${bundle.bundleProductId}`}
                            >
                              <ExternalLink className='h-4 w-4' />
                            </Link>
                          </Button>
                        )}
                        {onDeleteBundle && (
                          <Button
                            variant='ghost'
                            size='icon'
                            className='text-destructive hover:bg-destructive/10 h-8 w-8'
                            onClick={() => onDeleteBundle(bundle.id)}
                            disabled={disabled}
                          >
                            <Trash2 className='h-4 w-4' />
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* Pagination Bar */}
      {totalPages > 1 && onPageChange && (
        <div className='text-muted-foreground flex items-center justify-between text-xs'>
          <span>
            Showing {(page - 1) * perPage + 1} to{' '}
            {Math.min(page * perPage, total)} of {total} bundles
          </span>
          <div className='flex items-center gap-1'>
            <Button
              variant='outline'
              size='sm'
              onClick={() => onPageChange(page - 1)}
              disabled={page <= 1 || disabled}
              className='h-8 text-xs'
            >
              Previous
            </Button>
            <span className='px-2 font-mono'>
              {page} / {totalPages}
            </span>
            <Button
              variant='outline'
              size='sm'
              onClick={() => onPageChange(page + 1)}
              disabled={page >= totalPages || disabled}
              className='h-8 text-xs'
            >
              Next
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
