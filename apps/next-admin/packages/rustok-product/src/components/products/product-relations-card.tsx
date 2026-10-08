/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

'use strict';

import * as React from 'react';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell
} from '@/widgets/data-table';
import {
  Plus,
  GitFork,
  ArrowUp,
  ArrowDown,
  Trash2,
  Package,
  ExternalLink,
  Loader2
} from 'lucide-react';
import type {
  ProductRelation,
  ProductRelationType,
  ProductListItem,
  AddProductRelationInput
} from '../../api/types';
import { ProductRelationAddDialog } from './product-relation-add-dialog';
import Link from 'next/link';

interface ProductRelationsCardProps {
  productId: string;
  relations: ProductRelation[];
  onAddRelation?: (input: AddProductRelationInput) => Promise<void>;
  onRemoveRelation?: (id: string) => Promise<void>;
  onReorderRelations?: (
    productId: string,
    relationType: ProductRelationType,
    orderedIds: string[]
  ) => Promise<void>;
  onSearchProducts?: (query: string) => Promise<ProductListItem[]>;
  disabled?: boolean;
}

const RELATION_TYPE_BADGES: Record<
  ProductRelationType,
  { label: string; className: string }
> = {
  CROSS_SELL: {
    label: 'Cross-sell',
    className:
      'bg-blue-50 text-blue-700 border-blue-200 dark:bg-blue-950/40 dark:text-blue-300 dark:border-blue-800'
  },
  UP_SELL: {
    label: 'Up-sell',
    className:
      'bg-purple-50 text-purple-700 border-purple-200 dark:bg-purple-950/40 dark:text-purple-300 dark:border-purple-800'
  },
  RELATED: {
    label: 'Related',
    className:
      'bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-800'
  },
  ACCESSORY: {
    label: 'Accessory',
    className:
      'bg-amber-50 text-amber-700 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-800'
  },
  ALTERNATIVE: {
    label: 'Alternative',
    className:
      'bg-slate-50 text-slate-700 border-slate-200 dark:bg-slate-950/40 dark:text-slate-300 dark:border-slate-800'
  }
};

type FilterType = 'ALL' | ProductRelationType;

export function ProductRelationsCard({
  productId,
  relations,
  onAddRelation,
  onRemoveRelation,
  onReorderRelations,
  onSearchProducts,
  disabled
}: ProductRelationsCardProps) {
  const [activeFilter, setActiveFilter] = React.useState<FilterType>('ALL');
  const [isAddOpen, setIsAddOpen] = React.useState(false);
  const [deletingId, setDeletingId] = React.useState<string | null>(null);
  const [isReordering, setIsReordering] = React.useState(false);

  // Filtered relations sorted by position
  const filteredRelations = React.useMemo(() => {
    let list = [...relations];
    if (activeFilter !== 'ALL') {
      list = list.filter((r) => r.relationType === activeFilter);
    }
    return list.sort((a, b) => a.position - b.position);
  }, [relations, activeFilter]);

  // Counts by type
  const counts = React.useMemo(() => {
    const map: Record<string, number> = { ALL: relations.length };
    for (const r of relations) {
      map[r.relationType] = (map[r.relationType] || 0) + 1;
    }
    return map;
  }, [relations]);

  const handleDelete = async (id: string) => {
    if (!onRemoveRelation) return;
    if (!confirm('Удалить эту связь с товаром?')) return;
    setDeletingId(id);
    try {
      await onRemoveRelation(id);
    } finally {
      setDeletingId(null);
    }
  };

  const handleMove = async (
    index: number,
    direction: 'up' | 'down',
    type: ProductRelationType
  ) => {
    if (!onReorderRelations) return;
    const sameTypeRelations = relations
      .filter((r) => r.relationType === type)
      .sort((a, b) => a.position - b.position);

    const targetIndex = direction === 'up' ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= sameTypeRelations.length) return;

    const reordered = [...sameTypeRelations];
    const [moved] = reordered.splice(index, 1);
    reordered.splice(targetIndex, 0, moved);

    const orderedIds = reordered.map((r) => r.id);
    setIsReordering(true);
    try {
      await onReorderRelations(productId, type, orderedIds);
    } finally {
      setIsReordering(false);
    }
  };

  return (
    <>
      <Card className='border-border rounded-2xl shadow-sm'>
        <CardHeader className='pb-3'>
          <div className='flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between'>
            <div>
              <CardTitle className='flex items-center gap-2 text-sm font-semibold'>
                <GitFork className='text-primary h-4 w-4' />
                Связанные товары (Cross-sell, Up-sell, Комплекты)
              </CardTitle>
              <CardDescription className='text-xs'>
                Управление мерчандайзинговыми связями: сопутствующие товары,
                аксессуары, апселл и замены
              </CardDescription>
            </div>
            {onAddRelation && (
              <Button
                type='button'
                size='sm'
                variant='outline'
                onClick={() => setIsAddOpen(true)}
                disabled={disabled}
                className='h-8 gap-1.5 self-start rounded-xl text-xs sm:self-auto'
              >
                <Plus className='h-3.5 w-3.5' />
                Добавить связь
              </Button>
            )}
          </div>

          {/* Filter Pills */}
          <div className='flex flex-wrap gap-1.5 pt-3'>
            {(
              [
                'ALL',
                'CROSS_SELL',
                'UP_SELL',
                'RELATED',
                'ACCESSORY',
                'ALTERNATIVE'
              ] as FilterType[]
            ).map((ft) => {
              const count = counts[ft] || 0;
              const isActive = activeFilter === ft;
              const label =
                ft === 'ALL'
                  ? 'Все'
                  : RELATION_TYPE_BADGES[ft as ProductRelationType]?.label ||
                    ft;

              return (
                <button
                  key={ft}
                  type='button'
                  onClick={() => setActiveFilter(ft)}
                  className={`inline-flex items-center gap-1.5 rounded-xl px-3 py-1 text-xs font-medium transition ${
                    isActive
                      ? 'bg-primary text-primary-foreground shadow-sm'
                      : 'bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground'
                  }`}
                >
                  <span>{label}</span>
                  <span
                    className={`py-0.2 rounded-full px-1.5 text-[10px] ${
                      isActive
                        ? 'bg-primary-foreground/20 text-primary-foreground'
                        : 'bg-background text-muted-foreground'
                    }`}
                  >
                    {count}
                  </span>
                </button>
              );
            })}
          </div>
        </CardHeader>

        <CardContent className='pt-0'>
          {filteredRelations.length === 0 ? (
            <div className='border-border/70 rounded-xl border border-dashed p-8 text-center'>
              <div className='bg-muted text-muted-foreground mx-auto mb-3 flex h-10 w-10 items-center justify-center rounded-xl'>
                <GitFork className='h-5 w-5' />
              </div>
              <p className='text-foreground text-xs font-medium'>
                Нет связанных товаров
              </p>
              <p className='text-muted-foreground mx-auto mt-0.5 max-w-sm text-[11px]'>
                Добавьте кросс-селл или аксессуары, чтобы повысить средний чек и
                помочь покупателям найти нужный комплект.
              </p>
              {onAddRelation && (
                <Button
                  type='button'
                  size='sm'
                  variant='secondary'
                  onClick={() => setIsAddOpen(true)}
                  disabled={disabled}
                  className='mt-3.5 h-8 gap-1.5 rounded-xl text-xs'
                >
                  <Plus className='h-3.5 w-3.5' />
                  Связать товар
                </Button>
              )}
            </div>
          ) : (
            <div className='border-border overflow-x-auto rounded-xl border'>
              <Table>
                <TableHeader>
                  <TableRow className='bg-muted/30 text-[11px]'>
                    <TableHead className='w-16 text-center'>Порядок</TableHead>
                    <TableHead>Связанный товар</TableHead>
                    <TableHead className='w-32'>Тип связи</TableHead>
                    <TableHead className='w-28'>Добавлено</TableHead>
                    <TableHead className='w-20 text-right'>Действия</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredRelations.map((rel, idx) => {
                    const badge = RELATION_TYPE_BADGES[rel.relationType];
                    const relProduct = rel.relatedProduct;
                    const isDeleting = deletingId === rel.id;

                    return (
                      <TableRow key={rel.id} className='text-xs'>
                        {/* Position / Reorder Controls */}
                        <TableCell className='py-2 text-center'>
                          <div className='flex items-center justify-center gap-0.5'>
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              className='text-muted-foreground hover:text-foreground h-6 w-6 rounded-md'
                              disabled={disabled || isReordering || idx === 0}
                              onClick={() =>
                                handleMove(idx, 'up', rel.relationType)
                              }
                              title='Переместить выше'
                            >
                              <ArrowUp className='h-3 w-3' />
                            </Button>
                            <span className='text-muted-foreground w-4 text-center font-mono text-[11px]'>
                              {rel.position}
                            </span>
                            <Button
                              type='button'
                              variant='ghost'
                              size='icon'
                              className='text-muted-foreground hover:text-foreground h-6 w-6 rounded-md'
                              disabled={
                                disabled ||
                                isReordering ||
                                idx === filteredRelations.length - 1
                              }
                              onClick={() =>
                                handleMove(idx, 'down', rel.relationType)
                              }
                              title='Переместить ниже'
                            >
                              <ArrowDown className='h-3 w-3' />
                            </Button>
                          </div>
                        </TableCell>

                        {/* Related Product Details */}
                        <TableCell className='py-2'>
                          <div className='flex items-center gap-3'>
                            {relProduct?.thumbnail ? (
                              <img
                                src={relProduct.thumbnail}
                                alt={relProduct.title}
                                className='border-border h-9 w-9 shrink-0 rounded-lg border object-cover'
                              />
                            ) : (
                              <div className='bg-muted text-muted-foreground border-border flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border'>
                                <Package className='h-4 w-4' />
                              </div>
                            )}
                            <div className='min-w-0'>
                              <div className='flex items-center gap-1.5'>
                                <Link
                                  href={`/dashboard/product/${rel.relatedProductId}`}
                                  className='text-foreground hover:text-primary truncate font-semibold transition'
                                >
                                  {relProduct?.title ||
                                    `Товар #${rel.relatedProductId.slice(0, 8)}...`}
                                </Link>
                                <ExternalLink className='text-muted-foreground h-3 w-3 shrink-0' />
                              </div>
                              <div className='text-muted-foreground truncate font-mono text-[10px]'>
                                {relProduct?.sku
                                  ? `SKU: ${relProduct.sku} • `
                                  : ''}
                                ID: {rel.relatedProductId}
                              </div>
                            </div>
                          </div>
                        </TableCell>

                        {/* Relation Type Badge */}
                        <TableCell className='py-2'>
                          <Badge
                            variant='outline'
                            className={`px-2 py-0.5 text-[10px] font-medium uppercase ${badge?.className || ''}`}
                          >
                            {badge?.label || rel.relationType}
                          </Badge>
                        </TableCell>

                        {/* Date Added */}
                        <TableCell className='text-muted-foreground py-2 text-[11px]'>
                          {rel.createdAt
                            ? new Date(rel.createdAt).toLocaleDateString()
                            : '—'}
                        </TableCell>

                        {/* Actions */}
                        <TableCell className='py-2 text-right'>
                          <Button
                            type='button'
                            variant='ghost'
                            size='icon'
                            className='h-7 w-7 rounded-lg text-rose-500 hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/30'
                            disabled={disabled || isDeleting}
                            onClick={() => handleDelete(rel.id)}
                            title='Удалить связь'
                          >
                            {isDeleting ? (
                              <Loader2 className='h-3.5 w-3.5 animate-spin' />
                            ) : (
                              <Trash2 className='h-3.5 w-3.5' />
                            )}
                          </Button>
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Add Relation Modal Dialog */}
      {onAddRelation && (
        <ProductRelationAddDialog
          open={isAddOpen}
          onOpenChange={setIsAddOpen}
          productId={productId}
          onSearchProducts={onSearchProducts}
          onAddRelation={onAddRelation}
          disabled={disabled}
        />
      )}
    </>
  );
}
