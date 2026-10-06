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
  const [categories, setCategories] = useState<AdminCategoryTreeNode[]>(initialCategories);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // Dialog State
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [selectedCategory, setSelectedCategory] = useState<AdminCategoryTreeNode | null>(null);
  const [targetParent, setTargetParent] = useState<AdminCategoryTreeNode | null>(null);

  // Reload tree from server
  const loadTree = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchAdminCategoryTree(gqlOpts, currentLocale);
      setCategories(data);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to load category tree');
    } finally {
      setLoading(false);
    }
  }, [gqlOpts, currentLocale]);

  useEffect(() => {
    setCategories(initialCategories);
  }, [initialCategories]);

  // Flat list for parent selector in dialog
  const flatCategories = useMemo(() => flattenCategoryTree(categories), [categories]);

  // Handle Move Position Up / Down among siblings
  const handleMoveSibling = async (
    category: AdminCategoryTreeNode,
    siblings: AdminCategoryTreeNode[],
    direction: 'up' | 'down'
  ) => {
    const currentIndex = siblings.findIndex((s) => s.id === category.id);
    if (currentIndex === -1) return;

    const targetIndex = direction === 'up' ? currentIndex - 1 : currentIndex + 1;
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
      setError(err instanceof Error ? err.message : 'Failed to delete category');
    }
  };

  // Filter tree nodes recursively based on search query
  const filteredTree = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return categories;

    function filterNode(node: AdminCategoryTreeNode): AdminCategoryTreeNode | null {
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
      <div key={node.id} className="group relative">
        <div
          style={{ paddingLeft: `${depthPadding}px` }}
          className="relative transition hover:bg-muted/30"
        >
          {/* Depth connector guidelines */}
          {node.depth > 0 && (
            <span
              style={{ left: `${depthPadding - 16}px` }}
              className="absolute top-1/2 -mt-2 h-4 w-3 rounded-bl-md border-b-2 border-l-2 border-border/80"
            />
          )}

          <div className="flex flex-col gap-3 rounded-2xl border border-border bg-card p-4 shadow-2xs sm:flex-row sm:items-center sm:justify-between">
            {/* Left Info */}
            <div className="flex items-start sm:items-center gap-3 min-w-0 flex-1">
              {/* Color Accent Pill */}
              <span
                className="h-9 w-2 rounded-full shrink-0"
                style={{ backgroundColor: accentColor }}
              />

              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h4 className="text-sm font-semibold text-foreground truncate">
                    {node.icon ? `${node.icon} ` : ''}
                    {node.name}
                  </h4>
                  <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                    /{node.slug}
                  </span>
                  <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                    {node.effectiveLocale}
                  </span>
                  {node.isArchived && (
                    <span className="rounded-full bg-destructive/15 px-2 py-0.5 text-[11px] font-medium text-destructive">
                      Archived
                    </span>
                  )}
                  {node.allowsTopics ? (
                    <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-600 dark:text-emerald-400">
                      Allows Topics
                    </span>
                  ) : (
                    <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium text-amber-600 dark:text-amber-400">
                      Container Only
                    </span>
                  )}
                  {node.moderated && (
                    <span className="rounded-full bg-purple-500/10 px-2 py-0.5 text-[10px] font-medium text-purple-600 dark:text-purple-400">
                      Moderated
                    </span>
                  )}
                </div>

                {node.description && (
                  <p className="mt-1 line-clamp-1 text-xs text-muted-foreground">
                    {node.description}
                  </p>
                )}
              </div>
            </div>

            {/* Metrics & Action Controls */}
            <div className="flex flex-wrap items-center gap-2 sm:gap-4 shrink-0">
              {/* Counts */}
              <div className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
                <span>{node.topicCount} topics</span>
                <span>·</span>
                <span>{node.replyCount} replies</span>
              </div>

              {/* Position Up / Down */}
              <div className="flex items-center rounded-lg border border-border bg-background p-0.5">
                <button
                  type="button"
                  title="Move Up"
                  disabled={isFirst}
                  onClick={() => handleMoveSibling(node, siblings, 'up')}
                  className="rounded px-1.5 py-0.5 text-xs text-muted-foreground transition hover:bg-muted disabled:opacity-30"
                >
                  ▲
                </button>
                <button
                  type="button"
                  title="Move Down"
                  disabled={isLast}
                  onClick={() => handleMoveSibling(node, siblings, 'down')}
                  className="rounded px-1.5 py-0.5 text-xs text-muted-foreground transition hover:bg-muted disabled:opacity-30"
                >
                  ▼
                </button>
              </div>

              {/* Action Buttons */}
              <button
                type="button"
                onClick={() => {
                  setSelectedCategory(null);
                  setTargetParent(node);
                  setIsDialogOpen(true);
                }}
                className="rounded-lg border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition hover:border-primary hover:bg-muted"
              >
                + Subcategory
              </button>
              <button
                type="button"
                onClick={() => {
                  setSelectedCategory(node);
                  setTargetParent(null);
                  setIsDialogOpen(true);
                }}
                className="rounded-lg border border-border bg-background px-2.5 py-1 text-xs font-medium text-foreground transition hover:border-primary hover:bg-muted"
              >
                Edit
              </button>
              <button
                type="button"
                onClick={() => handleDelete(node)}
                className="rounded-lg border border-destructive/30 bg-destructive/5 px-2.5 py-1 text-xs font-medium text-destructive transition hover:bg-destructive/15"
              >
                Delete
              </button>
            </div>
          </div>
        </div>

        {/* Recursive Children */}
        {node.children && node.children.length > 0 && (
          <div className="mt-2 space-y-2">
            {node.children.map((child, idx) =>
              renderCategoryNode(child, node.children, idx)
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="space-y-6">
      {/* Top Action Bar */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={() => {
              setSelectedCategory(null);
              setTargetParent(null);
              setIsDialogOpen(true);
            }}
            className="rounded-full bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-xs transition hover:opacity-95"
          >
            + New Category
          </button>
          <button
            type="button"
            onClick={loadTree}
            disabled={loading}
            className="rounded-full border border-border px-3.5 py-2 text-xs font-medium text-foreground transition hover:bg-muted disabled:opacity-50"
          >
            {loading ? 'Refreshing…' : '↻ Refresh'}
          </button>
          <span className="text-xs text-muted-foreground">
            {flatCategories.length} categories total
          </span>
        </div>

        {/* Search / Filter */}
        <div className="relative min-w-64 max-w-sm">
          <input
            type="search"
            placeholder="Filter categories by name or slug…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-xl border border-border bg-background px-3.5 py-1.5 text-xs outline-none transition focus:border-primary"
          />
        </div>
      </div>

      {error && (
        <div className="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive">
          {error}
        </div>
      )}

      {/* Categories Tree List */}
      {filteredTree.length === 0 ? (
        <div className="rounded-[1.75rem] border border-dashed border-border p-12 text-center">
          <p className="text-sm font-semibold text-foreground">
            {searchQuery ? 'No categories matched your filter' : 'No categories created yet'}
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            {searchQuery
              ? 'Try clearing the search query to see all categories.'
              : 'Click "+ New Category" above to establish your forum hierarchy.'}
          </p>
        </div>
      ) : (
        <div className="space-y-3">
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
