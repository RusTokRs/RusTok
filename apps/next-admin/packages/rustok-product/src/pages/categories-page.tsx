import * as React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/shared/ui/shadcn/card';
import { listCatalogCategories } from '../api/categories';
import type {
  GqlOpts,
  CreateCatalogCategoryPayload,
  CatalogCategorySummary
} from '../api/types';
import { CategoriesTable } from '../components/categories/categories-table';
import { CategoryCreateDialog } from '../components/categories/category-create-dialog';

export interface CategoriesPageProps {
  token: string | null;
  tenantSlug: string | null;
  tenantId: string | null;
  locale?: string;
  onCreateCategory: (payload: CreateCatalogCategoryPayload) => Promise<void>;
}

export async function CategoriesPage({
  token,
  tenantSlug,
  tenantId,
  locale = 'en',
  onCreateCategory
}: CategoriesPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let categories: CatalogCategorySummary[] = [];
  let error: string | null = null;

  try {
    categories = await listCatalogCategories(opts, locale);
  } catch (err) {
    error =
      err instanceof Error ? err.message : 'Failed to load catalog categories.';
  }

  return (
    <div className='space-y-6'>
      <div className='flex items-center justify-between'>
        <div>
          <h2 className='text-lg font-semibold tracking-tight'>
            Catalog Categories
          </h2>
          <p className='text-muted-foreground text-sm'>
            Manage category hierarchies and taxonomy bindings for your products.
          </p>
        </div>
        <CategoryCreateDialog
          categories={categories}
          onCreateCategory={onCreateCategory}
        />
      </div>

      {error ? (
        <Card className='border-destructive/50'>
          <CardContent className='text-destructive py-6 text-sm'>
            {error}
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader className='pb-3'>
            <CardTitle className='text-base'>Category Tree</CardTitle>
            <CardDescription>
              Hierarchical view of all catalog categories for this tenant.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <CategoriesTable categories={categories} />
          </CardContent>
        </Card>
      )}
    </div>
  );
}
