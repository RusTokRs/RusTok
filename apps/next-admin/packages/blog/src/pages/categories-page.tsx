'use client';

import { useEffect, useState, useMemo } from 'react';
import { useLocale } from '@rustok/next-fluent';
import { toast } from 'sonner';
import {
  IconPlus,
  IconEdit,
  IconTrash,
  IconSearch,
  IconFolder
} from '@tabler/icons-react';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
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
import { listBlogCategories, deleteBlogCategory } from '../api/categories';
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
  const hostLocale = useLocale();
  const gqlOpts: GqlOpts = useMemo(
    () => ({ token, tenantSlug, tenantId }),
    [token, tenantSlug, tenantId]
  );

  const [categories, setCategories] = useState<BlogCategory[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [modalOpen, setModalOpen] = useState(false);
  const [editingCategory, setEditingCategory] = useState<BlogCategory | null>(null);
  const [deletingCategory, setDeletingCategory] = useState<BlogCategory | null>(null);

  const fetchCategories = () => {
    setLoading(true);
    listBlogCategories({ locale: hostLocale }, gqlOpts)
      .then((res) => {
        setCategories(res.items);
      })
      .catch(() => {
        toast.error('Failed to load categories');
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
      toast.success('Category deleted');
      setDeletingCategory(null);
      fetchCategories();
    } catch {
      toast.error('Failed to delete category');
    }
  };

  return (
    <div className='flex flex-1 flex-col space-y-4'>
      <div className='flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between'>
        <div className='relative flex-1 max-w-sm'>
          <IconSearch className='absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground' />
          <input
            type='text'
            placeholder='Search categories...'
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className='flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 pl-9 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring'
          />
        </div>
        <Button onClick={handleCreate}>
          <IconPlus className='mr-2 h-4 w-4' /> New Category
        </Button>
      </div>

      <Card>
        <CardHeader className='pb-3'>
          <CardTitle>Categories</CardTitle>
          <CardDescription>
            Organize blog posts into structured taxonomy categories
          </CardDescription>
        </CardHeader>
        <CardContent className='p-0'>
          {loading ? (
            <div className='flex h-48 items-center justify-center text-sm text-muted-foreground'>
              Loading categories...
            </div>
          ) : filteredCategories.length === 0 ? (
            <div className='flex h-48 flex-col items-center justify-center gap-2 text-center'>
              <IconFolder className='h-8 w-8 text-muted-foreground' />
              <p className='text-sm text-muted-foreground'>
                {searchQuery
                  ? 'No categories found matching your query.'
                  : 'No categories created yet.'}
              </p>
              {!searchQuery && (
                <Button variant='outline' size='sm' onClick={handleCreate}>
                  <IconPlus className='mr-2 h-4 w-4' /> Add Category
                </Button>
              )}
            </div>
          ) : (
            <div className='overflow-x-auto'>
              <table className='w-full text-left text-sm'>
                <thead className='border-b bg-muted/50 text-xs uppercase text-muted-foreground'>
                  <tr>
                    <th className='px-4 py-3'>Name</th>
                    <th className='px-4 py-3'>Slug</th>
                    <th className='px-4 py-3'>Posts</th>
                    <th className='px-4 py-3'>Order</th>
                    <th className='px-4 py-3'>Status</th>
                    <th className='px-4 py-3 text-right'>Actions</th>
                  </tr>
                </thead>
                <tbody className='divide-y'>
                  {filteredCategories.map((category) => (
                    <tr key={category.id} className='hover:bg-muted/50 transition-colors'>
                      <td className='px-4 py-3'>
                        <div className='font-medium'>{category.name}</div>
                        {category.description && (
                          <div className='text-xs text-muted-foreground truncate max-w-xs'>
                            {category.description}
                          </div>
                        )}
                      </td>
                      <td className='px-4 py-3 font-mono text-xs text-muted-foreground'>
                        {category.slug}
                      </td>
                      <td className='px-4 py-3'>
                        <Badge variant='secondary' className='text-xs'>
                          {category.postsCount} posts
                        </Badge>
                      </td>
                      <td className='px-4 py-3 text-xs text-muted-foreground'>
                        {category.displayOrder}
                      </td>
                      <td className='px-4 py-3'>
                        <Badge
                          variant={category.isActive ? 'default' : 'outline'}
                          className='text-xs'
                        >
                          {category.isActive ? 'Active' : 'Inactive'}
                        </Badge>
                      </td>
                      <td className='px-4 py-3 text-right space-x-2'>
                        <Button
                          variant='ghost'
                          size='icon'
                          className='h-8 w-8'
                          onClick={() => handleEdit(category)}
                        >
                          <IconEdit className='h-4 w-4' />
                          <span className='sr-only'>Edit</span>
                        </Button>
                        <Button
                          variant='ghost'
                          size='icon'
                          className='h-8 w-8 text-destructive hover:text-destructive'
                          onClick={() => setDeletingCategory(category)}
                        >
                          <IconTrash className='h-4 w-4' />
                          <span className='sr-only'>Delete</span>
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <CategoryModal
        open={modalOpen}
        onOpenChange={setModalOpen}
        category={editingCategory}
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
            <AlertDialogTitle>Delete Category</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete the category &ldquo;{deletingCategory?.name}&rdquo;?
              This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDeleteConfirm}
              className='bg-destructive text-destructive-foreground hover:bg-destructive/90'
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
