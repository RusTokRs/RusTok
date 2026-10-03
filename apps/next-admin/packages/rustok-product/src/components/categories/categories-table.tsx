'use client';

import * as React from 'react';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@/widgets/data-table/table';
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

  return (
    <div className='space-y-4'>
      <div className='flex items-center gap-3'>
        <div className='relative flex-1 max-w-sm'>
          <Search className='text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2' />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder='Filter categories by name, code or path...'
            className='pl-9'
          />
        </div>
        <p className='text-muted-foreground text-xs ml-auto'>
          Total categories: <span className='font-semibold text-foreground'>{categories.length}</span>
        </p>
      </div>

      <div className='rounded-md border'>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className='w-[350px]'>Category</TableHead>
              <TableHead>Code</TableHead>
              <TableHead>Slug</TableHead>
              <TableHead>Kind</TableHead>
              <TableHead>Path / Parent</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {filteredCategories.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className='text-muted-foreground py-8 text-center text-sm'>
                  <div className='flex flex-col items-center justify-center gap-1.5'>
                    <FolderTree className='h-8 w-8 text-muted-foreground/50' />
                    <p>No categories found.</p>
                  </div>
                </TableCell>
              </TableRow>
            ) : (
              filteredCategories.map((cat) => {
                const depth = getDepth(cat);
                const parentCat = cat.parentId ? categoryMap.get(cat.parentId) : undefined;

                return (
                  <TableRow key={cat.id}>
                    <TableCell>
                      <div
                        className='flex items-center gap-2'
                        style={{ paddingLeft: `${depth * 20}px` }}
                      >
                        {depth > 0 ? (
                          <CornerDownRight className='text-muted-foreground h-3.5 w-3.5 flex-shrink-0' />
                        ) : (
                          <FolderTree className='text-primary/70 h-4 w-4 flex-shrink-0' />
                        )}
                        <span className='font-medium text-sm'>{cat.name || cat.code}</span>
                      </div>
                    </TableCell>
                    <TableCell>
                      <code className='bg-muted rounded px-1.5 py-0.5 text-xs font-mono'>
                        {cat.code}
                      </code>
                    </TableCell>
                    <TableCell className='text-muted-foreground text-sm font-mono'>
                      {cat.slug}
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={cat.kind === 'virtual' ? 'outline' : 'secondary'}
                        className='capitalize text-xs'
                      >
                        {cat.kind}
                      </Badge>
                    </TableCell>
                    <TableCell className='text-muted-foreground text-xs'>
                      {cat.path || (parentCat ? parentCat.name || parentCat.code : 'Root')}
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}
