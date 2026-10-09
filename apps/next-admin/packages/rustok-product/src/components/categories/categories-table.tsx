'use client';

import * as React from 'react';
import { DataTableStatic } from '@/widgets/data-table/data-table-static';
import type { DataTableStaticColumn } from '@/widgets/data-table/data-table-static';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Input } from '@/shared/ui/shadcn/input';
import { FolderTree, Search, CornerDownRight } from 'lucide-react';
import type { CatalogCategorySummary } from '../../api/types';

interface CategoriesTableProps {
  categories: CatalogCategorySummary[];
}

export function CategoriesTable({ categories }: CategoriesTableProps) {
  const [search, setSearch] = React.useState('');

  const filteredCategories = React.useMemo(() => {
    if (!search.trim()) return categories;
    const q = search.toLowerCase().trim();
    return categories.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.code.toLowerCase().includes(q) ||
        c.slug.toLowerCase().includes(q) ||
        c.path.toLowerCase().includes(q)
    );
  }, [categories, search]);

  // Build parent map to resolve parent names
  const categoryMap = React.useMemo(() => {
    const map = new Map<string, CatalogCategorySummary>();
    for (const c of categories) {
      map.set(c.id, c);
    }
    return map;
  }, [categories]);

  // Determine hierarchy depth from path or parentId chain
  const getDepth = (cat: CatalogCategorySummary): number => {
    if (cat.path) {
      const parts = cat.path.split('/').filter(Boolean);
      return Math.max(0, parts.length - 1);
    }
    let depth = 0;
    let curr = cat.parentId ? categoryMap.get(cat.parentId) : undefined;
    while (curr) {
      depth++;
      curr = curr.parentId ? categoryMap.get(curr.parentId) : undefined;
    }
    return depth;
  };

  const columns: DataTableStaticColumn<CatalogCategorySummary>[] = [
    {
      id: 'category',
      header: 'Category',
      headerClassName: 'w-[350px]',
const indentScale = ['pl-0', 'pl-5', 'pl-10', 'pl-14', 'pl-20', 'pl-24', 'pl-28', 'pl-32'];
function categoryIndentClass(d: number): string {
  return indentScale[Math.min(d, indentScale.length - 1)] ?? 'pl-36';
}

        const depth = getDepth(cat);
        return (
          <div
            className={`flex items-center gap-2 ${categoryIndentClass(depth)}`}
          >
            {depth > 0 ? (
              <CornerDownRight className='text-muted-foreground h-3.5 w-3.5 flex-shrink-0' />
            ) : (
              <FolderTree className='text-primary/70 h-4 w-4 flex-shrink-0' />
            )}
            <span className='text-sm font-medium'>{cat.name || cat.code}</span>
          </div>
        );
      }
    },
    {
      id: 'code',
      header: 'Code',
      cell: (cat) => (
        <code className='bg-muted rounded px-1.5 py-0.5 font-mono text-xs'>
          {cat.code}
        </code>
      )
    },
    {
      id: 'slug',
      header: 'Slug',
      cellClassName: 'text-muted-foreground font-mono text-sm',
      cell: (cat) => cat.slug
    },
    {
      id: 'kind',
      header: 'Kind',
      cell: (cat) => (
        <Badge
          variant={cat.kind === 'virtual' ? 'outline' : 'secondary'}
          className='text-xs capitalize'
        >
          {cat.kind}
        </Badge>
      )
    },
    {
      id: 'path',
      header: 'Path / Parent',
      cellClassName: 'text-muted-foreground text-xs',
      cell: (cat) => {
        const parentCat = cat.parentId
          ? categoryMap.get(cat.parentId)
          : undefined;
        return (
          cat.path || (parentCat ? parentCat.name || parentCat.code : 'Root')
        );
      }
    }
  ];

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-3'>
        <div className='relative max-w-sm flex-1'>
          <Search className='text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2' />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder='Filter categories by name, code or path...'
            className='pl-9'
          />
        </div>
        <p className='text-muted-foreground ml-auto text-xs'>
          Total categories:{' '}
          <span className='text-foreground font-semibold'>
            {categories.length}
          </span>
        </p>
      </div>

      <DataTableStatic
        rows={filteredCategories}
        columns={columns}
        getRowKey={(cat) => cat.id}
        emptyState={
          <div className='flex flex-col items-center justify-center gap-1.5'>
            <FolderTree className='text-muted-foreground/50 h-8 w-8' />
            <p>No categories found.</p>
          </div>
        }
      />
    </div>
  );
}
