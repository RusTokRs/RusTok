'use client';

import { useEffect, useState, useMemo } from 'react';
import { useLocale, useTranslations } from '@rustok/next-fluent/client';
import { toast } from 'sonner';
import { Plus, Search, Folder } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Input } from '@/shared/ui/shadcn/input';
import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/components/ui/card';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle
} from '@/shared/ui/shadcn/alert-dialog';

import type { BlogCategory, GqlOpts } from '../api/categories';
import { listAllBlogCategories, deleteBlogCategory } from '../api/categories';
import { CategoryModal } from '../components/category-modal';

interface CategoriesPageProps {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default function CategoriesPage({
  token,
  tenantSlug,
  tenantId
}: CategoriesPageProps) {
  const t = useTranslations('blog');
  const hostLocale = useLocale();
  const gqlOpts: GqlOpts = useMemo(
    () => ({ token, tenantSlug, tenantId }),
    [token, tenantSlug, tenantId]
  );

  const [categories, setCategories] = useState<BlogCategory[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [modalOpen, setModalOpen] = useState(false);
  const [editingCategory, setEditingCategory] = useState<BlogCategory | null>(
    null
  );
  const [deletingCategory, setDeletingCategory] = useState<BlogCategory | null>(
    null
  );

  const fetchCategories = () => {
    setLoading(true);
    listAllBlogCategories({ locale: hostLocale }, gqlOpts)
      .then((res) => {
        setCategories(res);
      })
      .catch(() => {
        toast.error(t('categories.loadError'));
      })
      .finally(() => {
        setLoading(false);
      });
  };

  useEffect(() => {
    fetchCategories();
  }, [hostLocale, gqlOpts]);

  const filteredCategories = useMemo(() => {
    if (!searchQuery.trim()) return categories;
    const q = searchQuery.toLowerCase();
    return categories.filter(
      (cat) =>
        cat.name.toLowerCase().includes(q) ||
        cat.slug.toLowerCase().includes(q) ||
        (cat.description && cat.description.toLowerCase().includes(q))
    );
  }, [categories, searchQuery]);

  const handleCreate = () => {
    setEditingCategory(null);
    setModalOpen(true);
  };

  const handleEdit = (category: BlogCategory) => {
    setEditingCategory(category);
    setModalOpen(true);
  };

  const handleDeleteConfirm = async () => {
    if (!deletingCategory) return;
    try {
      await deleteBlogCategory(deletingCategory.id, gqlOpts);
      toast.success(t('categories.deleted'));
      setDeletingCategory(null);
      fetchCategories();
    } catch {
      toast.error(t('categories.deleteError'));
    }
  };

  const columns: ColumnDef<StockFeatures, BlogCategory, any>[] = [
    {
      id: 'name',
      accessorKey: 'name',
      header: t('categories.name'),
      enableSorting: false,
      cell: ({ row }) => (
        <div>
          <strong>{row.original.name}</strong>
          <p>{row.original.description}</p>
        </div>
      )
    },
    {
      id: 'slug',
      accessorKey: 'slug',
      header: t('categories.slug'),
      enableSorting: false
    },
    {
      id: 'position',
      accessorKey: 'position',
      header: t('categories.position'),
      enableSorting: false
    },
    {
      id: 'actions',
      header: t('table.actions'),
      enableSorting: false,
      cell: ({ row }) => (
        <div className='flex gap-2'>
          <Button
            variant='outline'
            size='sm'
            onClick={() => handleEdit(row.original)}
          >
            {t('table.edit')}
          </Button>
          <Button
            variant='destructive'
            size='sm'
            onClick={() => setDeletingCategory(row.original)}
          >
            {t('table.delete')}
          </Button>
        </div>
      )
    }
  ];

  return (
    <div className='flex flex-1 flex-col space-y-4'>
      <div className='flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between'>
        <div className='relative max-w-sm flex-1'>
          <Search className='text-muted-foreground absolute top-2.5 left-2.5 h-4 w-4' />
          <Input
            type='text'
            placeholder={t('categories.search')}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className='border-input placeholder:text-muted-foreground focus-visible:ring-ring flex h-9 w-full rounded-md border bg-transparent px-3 py-1 pl-9 text-sm shadow-sm transition-colors focus-visible:ring-1 focus-visible:outline-none'
          />
        </div>
        <Button onClick={handleCreate}>
          <Plus className='mr-2 h-4 w-4' />
          {t('categories.create')}
        </Button>
      </div>

      <Card>
        <CardHeader className='pb-3'>
          <CardTitle>{t('categories.title')}</CardTitle>
          <CardDescription>{t('categories.subtitle')}</CardDescription>
        </CardHeader>
        <CardContent className='p-0'>
          {loading ? (
            <div className='text-muted-foreground flex h-48 items-center justify-center text-sm'>
              {t('categories.loading')}
            </div>
          ) : filteredCategories.length === 0 ? (
            <div className='flex h-48 flex-col items-center justify-center gap-2 text-center'>
              <Folder className='text-muted-foreground h-8 w-8' />
              <p className='text-muted-foreground text-sm'>
                {searchQuery
                  ? t('categories.searchEmpty')
                  : t('categories.empty')}
              </p>
              {!searchQuery && (
                <Button variant='outline' size='sm' onClick={handleCreate}>
                  <Plus className='mr-2 h-4 w-4' />
                  {t('categories.add')}
                </Button>
              )}
            </div>
          ) : (
            <DataTableShell
              data={filteredCategories}
              columns={columns}
              clientPagination
            />
          )}
        </CardContent>
      </Card>

      <CategoryModal
        open={modalOpen}
        onOpenChange={setModalOpen}
        category={editingCategory}
        categories={categories}
        locale={hostLocale}
        gqlOpts={gqlOpts}
        onSuccess={fetchCategories}
      />

      <AlertDialog
        open={Boolean(deletingCategory)}
        onOpenChange={(open) => !open && setDeletingCategory(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('categories.deleteTitle')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('categories.deleteConfirm', {
                name: deletingCategory?.name ?? ''
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('categories.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDeleteConfirm}
              className='bg-destructive text-destructive-foreground hover:bg-destructive/90'
            >
              {t('table.delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
