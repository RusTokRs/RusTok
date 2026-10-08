'use client';

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import type { AdminCategoryTreeNode, GqlOpts } from '../api/forum';
import {
  fetchAdminCategoryTree,
  flattenCategoryTree,
  moveAdminCategory,
  deleteAdminCategory
} from '../api/forum';
import { CategoryEditorDialog } from './category-editor-dialog';

interface CategoryTreeAdminProps {
  initialCategories: AdminCategoryTreeNode[];
  gqlOpts: GqlOpts;
  currentLocale?: string;
}

export function CategoryTreeAdmin({
  initialCategories,
  gqlOpts,
  currentLocale = 'en'
}: CategoryTreeAdminProps) {
  const [categories, setCategories] =
    useState<AdminCategoryTreeNode[]>(initialCategories);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // Dialog State
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [selectedCategory, setSelectedCategory] =
    useState<AdminCategoryTreeNode | null>(null);
  const [targetParent, setTargetParent] =
    useState<AdminCategoryTreeNode | null>(null);

  // Reload tree from server
  const loadTree = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchAdminCategoryTree(gqlOpts, currentLocale);
      setCategories(data);
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : 'Failed to load category tree'
      );
    } finally {
      setLoading(false);
    }
  }, [gqlOpts, currentLocale]);

  useEffect(() => {
    setCategories(initialCategories);
  }, [initialCategories]);

  // Flat list for parent selector in dialog
  const flatCategories = useMemo(
    () => flattenCategoryTree(categories),
    [categories]
  );

  // Handle Move Position Up / Down among siblings
  const handleMoveSibling = async (
    category: AdminCategoryTreeNode,
    siblings: AdminCategoryTreeNode[],
    direction: 'up' | 'down'
  ) => {
    const currentIndex = siblings.findIndex((s) => s.id === category.id);
    if (currentIndex === -1) return;

    const targetIndex =
      direction === 'up' ? currentIndex - 1 : currentIndex + 1;
    if (targetIndex < 0 || targetIndex >= siblings.length) return;

    try {
      await moveAdminCategory(
        category.id,
        {
          parentId: category.parentId,
          position: targetIndex
        },
        gqlOpts
      );
      loadTree();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to move category');
    }
  };

  // Handle Delete
  const handleDelete = async (category: AdminCategoryTreeNode) => {
    const hasChildren = category.children && category.children.length > 0;
    const confirmMsg = hasChildren
      ? `Delete category "${category.name}" and all its subcategories?`
      : `Delete category "${category.name}"?`;

    if (!window.confirm(confirmMsg)) return;

    try {
      await deleteAdminCategory(category.id, gqlOpts);
      loadTree();
    } catch (err: unknown) {
      setError(
        err instanceof Error ? err.message : 'Failed to delete category'
      );
    }
  };

  // Filter tree nodes recursively based on search query
  const filteredTree = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return categories;

    function filterNode(
      node: AdminCategoryTreeNode
    ): AdminCategoryTreeNode | null {
      const matchesSelf =
        node.name.toLowerCase().includes(q) ||
        node.slug.toLowerCase().includes(q) ||
        (node.description?.toLowerCase().includes(q) ?? false);

      const filteredChildren = (node.children || [])
        .map(filterNode)
        .filter((child): child is AdminCategoryTreeNode => Boolean(child));

      if (matchesSelf || filteredChildren.length > 0) {
        return {
          ...node,
          children: filteredChildren
        };
      }
      return null;
    }

    return categories
      .map(filterNode)
      .filter((node): node is AdminCategoryTreeNode => Boolean(node));
  }, [categories, searchQuery]);

  // Render a recursive node and its children
  const renderCategoryNode = (
    node: AdminCategoryTreeNode,
    siblings: AdminCategoryTreeNode[],
    index: number
  ) => {
    const isFirst = index === 0;
    const isLast = index === siblings.length - 1;
    const accentColor = node.color || '#3b82f6';
    const depthPadding = node.depth * 28;

    return (
      <div key={node.id} className='group relative'>
        <div
          style={{ paddingLeft: `${depthPadding}px` }}
          className='hover:bg-muted/30 relative transition'
        >
          {/* Depth connector guidelines */}
          {node.depth > 0 && (
            <span
              style={{ left: `${depthPadding - 16}px` }}
              className='border-border/80 absolute top-1/2 -mt-2 h-4 w-3 rounded-bl-md border-b-2 border-l-2'
            />
          )}

          <div className='border-border bg-card flex flex-col gap-3 rounded-2xl border p-4 shadow-2xs sm:flex-row sm:items-center sm:justify-between'>
            {/* Left Info */}
            <div className='flex min-w-0 flex-1 items-start gap-3 sm:items-center'>
              {/* Color Accent Pill */}
              <span
                className='h-9 w-2 shrink-0 rounded-full'
                style={{ backgroundColor: accentColor }}
              />

              <div className='min-w-0 flex-1'>
                <div className='flex flex-wrap items-center gap-2'>
                  <h4 className='text-foreground truncate text-sm font-semibold'>
                    {node.icon ? `${node.icon} ` : ''}
                    {node.name}
                  </h4>
                  <span className='border-border bg-muted/40 text-muted-foreground rounded-full border px-2 py-0.5 font-mono text-[11px]'>
                    /{node.slug}
                  </span>
                  <span className='bg-muted text-muted-foreground rounded-full px-2 py-0.5 text-[10px] font-medium tracking-wider uppercase'>
                    {node.effectiveLocale}
                  </span>
                  {node.isArchived && (
                    <span className='bg-destructive/15 text-destructive rounded-full px-2 py-0.5 text-[11px] font-medium'>
                      Archived
                    </span>
                  )}
                  {node.allowsTopics ? (
                    <span className='rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-600 dark:text-emerald-400'>
                      Allows Topics
                    </span>
                  ) : (
                    <span className='rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400'>
                      Container Only
                    </span>
                  )}
                  {node.moderated && (
                    <span className='rounded-full bg-purple-500/10 px-2 py-0.5 text-[10px] font-medium text-purple-600 dark:text-purple-400'>
                      Moderated
                    </span>
                  )}
                </div>

                {node.description && (
                  <p className='text-muted-foreground mt-1 line-clamp-1 text-xs'>
                    {node.description}
                  </p>
                )}
              </div>
            </div>

            {/* Metrics & Action Controls */}
            <div className='flex shrink-0 flex-wrap items-center gap-2 sm:gap-4'>
              {/* Counts */}
              <div className='text-muted-foreground flex items-center gap-2 text-xs font-medium'>
                <span>{node.topicCount} topics</span>
                <span>·</span>
                <span>{node.replyCount} replies</span>
              </div>

              {/* Position Up / Down */}
              <div className='border-border bg-background flex items-center rounded-lg border p-0.5'>
                <button
                  type='button'
                  title='Move Up'
                  disabled={isFirst}
                  onClick={() => handleMoveSibling(node, siblings, 'up')}
                  className='text-muted-foreground hover:bg-muted rounded px-1.5 py-0.5 text-xs transition disabled:opacity-30'
                >
                  ▲
                </button>
                <button
                  type='button'
                  title='Move Down'
                  disabled={isLast}
                  onClick={() => handleMoveSibling(node, siblings, 'down')}
                  className='text-muted-foreground hover:bg-muted rounded px-1.5 py-0.5 text-xs transition disabled:opacity-30'
                >
                  ▼
                </button>
              </div>

              {/* Action Buttons */}
              <button
                type='button'
                onClick={() => {
                  setSelectedCategory(null);
                  setTargetParent(node);
                  setIsDialogOpen(true);
                }}
                className='border-border bg-background text-foreground hover:border-primary hover:bg-muted rounded-lg border px-2.5 py-1 text-xs font-medium transition'
              >
                + Subcategory
              </button>
              <button
                type='button'
                onClick={() => {
                  setSelectedCategory(node);
                  setTargetParent(null);
                  setIsDialogOpen(true);
                }}
                className='border-border bg-background text-foreground hover:border-primary hover:bg-muted rounded-lg border px-2.5 py-1 text-xs font-medium transition'
              >
                Edit
              </button>
              <button
                type='button'
                onClick={() => handleDelete(node)}
                className='border-destructive/30 bg-destructive/5 text-destructive hover:bg-destructive/15 rounded-lg border px-2.5 py-1 text-xs font-medium transition'
              >
                Delete
              </button>
            </div>
          </div>
        </div>

        {/* Recursive Children */}
        {node.children && node.children.length > 0 && (
          <div className='mt-2 space-y-2'>
            {node.children.map((child, idx) =>
              renderCategoryNode(child, node.children, idx)
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className='space-y-6'>
      {/* Top Action Bar */}
      <div className='border-border bg-card flex flex-col gap-4 rounded-2xl border p-4 shadow-sm sm:flex-row sm:items-center sm:justify-between'>
        <div className='flex flex-wrap items-center gap-3'>
          <button
            type='button'
            onClick={() => {
              setSelectedCategory(null);
              setTargetParent(null);
              setIsDialogOpen(true);
            }}
            className='bg-primary text-primary-foreground rounded-full px-4 py-2 text-xs font-medium shadow-xs transition hover:opacity-95'
          >
            + New Category
          </button>
          <button
            type='button'
            onClick={loadTree}
            disabled={loading}
            className='border-border text-foreground hover:bg-muted rounded-full border px-3.5 py-2 text-xs font-medium transition disabled:opacity-50'
          >
            {loading ? 'Refreshing…' : '↻ Refresh'}
          </button>
          <span className='text-muted-foreground text-xs'>
            {flatCategories.length} categories total
          </span>
        </div>

        {/* Search / Filter */}
        <div className='relative max-w-sm min-w-64'>
          <input
            type='search'
            placeholder='Filter categories by name or slug…'
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className='border-border bg-background focus:border-primary w-full rounded-xl border px-3.5 py-1.5 text-xs transition outline-none'
          />
        </div>
      </div>

      {error && (
        <div className='border-destructive/30 bg-destructive/10 text-destructive rounded-xl border px-4 py-3 text-xs'>
          {error}
        </div>
      )}

      {/* Categories Tree List */}
      {filteredTree.length === 0 ? (
        <div className='border-border rounded-[1.75rem] border border-dashed p-12 text-center'>
          <p className='text-foreground text-sm font-semibold'>
            {searchQuery
              ? 'No categories matched your filter'
              : 'No categories created yet'}
          </p>
          <p className='text-muted-foreground mt-1 text-xs'>
            {searchQuery
              ? 'Try clearing the search query to see all categories.'
              : 'Click "+ New Category" above to establish your forum hierarchy.'}
          </p>
        </div>
      ) : (
        <div className='space-y-3'>
          {filteredTree.map((root, idx) =>
            renderCategoryNode(root, filteredTree, idx)
          )}
        </div>
      )}

      {/* Editor Modal */}
      <CategoryEditorDialog
        isOpen={isDialogOpen}
        onClose={() => setIsDialogOpen(false)}
        onSaved={loadTree}
        initialCategory={selectedCategory}
        parentCategory={targetParent}
        availableParents={flatCategories}
        currentLocale={currentLocale}
        gqlOpts={gqlOpts}
      />
    </div>
  );
}
