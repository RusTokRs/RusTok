/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import { useRouter } from 'next/navigation';
import { Boxes, Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { BundlesTable } from '../components/bundles/bundles-table';
import { BundleCreateDialog } from '../components/bundles/bundle-create-dialog';
import type {
  ProductBundle,
  CreateBundleInput
} from '../api/types';

interface BundlesPageProps {
  initialBundles: ProductBundle[];
  total: number;
  page: number;
  perPage: number;
  onCreateBundle?: (input: CreateBundleInput) => Promise<void>;
  onDeleteBundle?: (id: string) => Promise<void>;
  onFilterChange?: (filters: {
    search?: string;
    status?: string;
    bundleType?: string;
    page?: number;
  }) => void;
}

export function BundlesPage({
  initialBundles,
  total,
  page,
  perPage,
  onCreateBundle,
  onDeleteBundle,
  onFilterChange
}: BundlesPageProps) {
  const router = useRouter();
  const [bundles, setBundles] = React.useState<ProductBundle[]>(initialBundles);
  const [isCreateOpen, setIsCreateOpen] = React.useState(false);

  React.useEffect(() => {
    setBundles(initialBundles);
  }, [initialBundles]);

  const handleCreate = async (input: CreateBundleInput) => {
    if (!onCreateBundle) return;
    await onCreateBundle(input);
    router.refresh();
  };

  const handleDelete = async (id: string) => {
    if (!onDeleteBundle) return;
    if (!confirm('Are you sure you want to delete this bundle?')) return;
    await onDeleteBundle(id);
    setBundles((prev) => prev.filter((b) => b.id !== id));
    router.refresh();
  };

  return (
    <div className='flex flex-1 flex-col gap-6 p-6'>
      {/* Header Bar */}
      <div className='flex flex-wrap items-center justify-between gap-4 border-b pb-4'>
        <div className='space-y-1'>
          <div className='flex items-center gap-2'>
            <Boxes className='h-6 w-6 text-primary' />
            <h1 className='text-xl font-bold tracking-tight'>Product Bundles & Kits</h1>
          </div>
          <p className='text-sm text-muted-foreground'>
            Manage curated product packages, kits, configurable sets, and package discounts.
          </p>
        </div>

        <Button onClick={() => setIsCreateOpen(true)} className='gap-1.5'>
          <Plus className='h-4 w-4' />
          New Bundle
        </Button>
      </div>

      {/* Main Table */}
      <BundlesTable
        bundles={bundles}
        total={total}
        page={page}
        perPage={perPage}
        onPageChange={(p) => onFilterChange?.({ page: p })}
        onSearchChange={(search) => onFilterChange?.({ search, page: 1 })}
        onStatusFilterChange={(status) => onFilterChange?.({ status, page: 1 })}
        onTypeFilterChange={(bundleType) => onFilterChange?.({ bundleType, page: 1 })}
        onDeleteBundle={handleDelete}
        onCreateClick={() => setIsCreateOpen(true)}
      />

      {/* Create Dialog */}
      <BundleCreateDialog
        open={isCreateOpen}
        onOpenChange={setIsCreateOpen}
        onCreate={handleCreate}
      />
    </div>
  );
}
