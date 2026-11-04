/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
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
import { Input } from '@/shared/ui/shadcn/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { listBrands, createBrand, deleteBrand } from '../api/brands';
import type {
  GqlOpts,
  BrandSummary,
  CreateBrandPayload,
  CatalogCategorySummary
} from '../api/types';
import { BrandsTable } from '../components/brands/brands-table';
import { BrandCreateDialog } from '../components/brands/brand-create-dialog';

export interface BrandsPageProps {
  token: string | null;
  tenantSlug: string | null;
  tenantId: string | null;
  locale?: string;
  categories?: CatalogCategorySummary[];
}

export function BrandsPage({
  token,
  tenantSlug,
  tenantId,
  locale = 'en',
  categories = []
}: BrandsPageProps) {
  const opts: GqlOpts = React.useMemo(
    () => ({ token, tenantSlug, tenantId, locale }),
    [token, tenantSlug, tenantId, locale]
  );

  const [brands, setBrands] = React.useState<BrandSummary[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [error, setError] = React.useState<string | null>(null);

  const [search, setSearch] = React.useState('');
  const [selectedCategoryId, setSelectedCategoryId] = React.useState<string>('all');
  const [statusFilter, setStatusFilter] = React.useState<string>('all');

  const fetchBrands = React.useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await listBrands(
        opts,
        {
          search: search.trim() || null,
          categoryId: selectedCategoryId !== 'all' ? selectedCategoryId : null,
          isActive:
            statusFilter === 'active'
              ? true
              : statusFilter === 'inactive'
              ? false
              : null,
          page: 1,
          perPage: 100
        },
        locale
      );
      setBrands(res.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load brands');
    } finally {
      setLoading(false);
    }
  }, [opts, search, selectedCategoryId, statusFilter, locale]);

  React.useEffect(() => {
    fetchBrands();
  }, [fetchBrands]);

  const handleCreateBrand = async (payload: CreateBrandPayload) => {
    await createBrand(opts, payload);
    await fetchBrands();
  };

  const handleDeleteBrand = async (id: string) => {
    await deleteBrand(opts, id);
    await fetchBrands();
  };

  return (
    <div className='space-y-6'>
      <div className='flex items-center justify-between'>
        <div>
          <h2 className='text-lg font-semibold tracking-tight'>Brand Directory</h2>
          <p className='text-muted-foreground text-sm'>
            Manage manufacturers, co-branding partners, and category-scoped brand catalogs.
          </p>
        </div>
        <BrandCreateDialog onCreateBrand={handleCreateBrand} />
      </div>

      {/* Filter and Category Scoping Bar */}
      <div className='flex flex-wrap items-center gap-3'>
        <div className='w-full sm:w-64'>
          <Input
            placeholder='Search brand or slug...'
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>

        {categories.length > 0 && (
          <div className='w-full sm:w-56'>
            <Select
              value={selectedCategoryId}
              onValueChange={setSelectedCategoryId}
            >
              <SelectTrigger>
                <SelectValue placeholder='All Categories' />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='all'>All Categories</SelectItem>
                {categories.map((c) => (
                  <SelectItem key={c.id} value={c.id}>
                    {c.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}

        <div className='w-full sm:w-36'>
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger>
              <SelectValue placeholder='Status' />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='all'>All Status</SelectItem>
              <SelectItem value='active'>Active</SelectItem>
              <SelectItem value='inactive'>Inactive</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      {error ? (
        <Card className='border-destructive/50'>
          <CardContent className='text-destructive py-6 text-sm'>
            {error}
          </CardContent>
        </Card>
      ) : loading ? (
        <div className='py-12 text-center text-sm text-muted-foreground'>
          Loading brands...
        </div>
      ) : (
        <Card>
          <CardHeader className='pb-3'>
            <CardTitle className='text-base'>Brands ({brands.length})</CardTitle>
            <CardDescription>
              {selectedCategoryId !== 'all'
                ? 'Showing brands scoped to the selected category.'
                : 'All registered catalog brands across this tenant.'}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <BrandsTable
              brands={brands}
              onDeleteBrand={handleDeleteBrand}
            />
          </CardContent>
        </Card>
      )}
    </div>
  );
}
