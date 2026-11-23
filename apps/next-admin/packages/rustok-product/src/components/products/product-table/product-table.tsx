'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type {
  ProductCatalogFacet,
  ProductCatalogSearchOption,
  ProductListItem
} from '../../../api/types';
import { columns } from './columns';
import { ProductCatalogFilters } from './product-catalog-filters';

interface ProductTableProps {
  data: ProductListItem[];
  totalItems: number;
  categoryOptions: ProductCatalogSearchOption[];
  attributeOptions: ProductCatalogSearchOption[];
  facets: ProductCatalogFacet[];
  categoryId?: string;
  attributeFilters: string[];
  sortBy?: string;
  sortDirection?: string;
  perPage: number;
  routeQuery: Record<string, string>;
}

export function ProductTable({
  data,
  totalItems,
  categoryOptions,
  attributeOptions,
  facets,
  categoryId,
  attributeFilters,
  sortBy,
  sortDirection,
  perPage,
  routeQuery
}: ProductTableProps) {
  return (
    <div className='space-y-4'>
      <ProductCatalogFilters
        categoryOptions={categoryOptions}
        attributeOptions={attributeOptions}
        facets={facets}
        categoryId={categoryId}
        attributeFilters={attributeFilters}
        sortBy={sortBy}
        sortDirection={sortDirection}
        perPage={perPage}
        routeQuery={routeQuery}
      />
      <DataTableShell
        data={data}
        columns={columns}
        totalItems={totalItems}
        defaultPageSize={perPage}
        getRowId={(product) => product.id}
      />
    </div>
  );
}
