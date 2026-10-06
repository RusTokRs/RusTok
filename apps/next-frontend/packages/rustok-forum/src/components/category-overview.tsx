'use client';

import React, { useState, useMemo } from 'react';
import type { ForumCategoryListItem } from '../api/forum';

interface CategoryOverviewProps {
  categories: ForumCategoryListItem[];
  total: number;
  onSelectCategory: (categoryId: string) => void;
  className?: string;
}

interface CategoryWithChildren {
  category: ForumCategoryListItem;
  subcategories: ForumCategoryListItem[];
  totalTopicCount: number;
  totalReplyCount: number;
}

export function CategoryOverview({
  categories,
  total,
  onSelectCategory,
  className = '',
}: CategoryOverviewProps) {
  const [searchQuery, setSearchQuery] = useState('');

  // Group into hierarchy: roots and subcategories
  const hierarchy = useMemo(() => {
    const byId = new Map<string, ForumCategoryListItem>();
    for (const cat of categories) {
      byId.set(cat.id, cat);
    }

    const roots: ForumCategoryListItem[] = [];
    const childrenMap = new Map<string, ForumCategoryListItem[]>();

    for (const cat of categories) {
      if (cat.parentId && byId.has(cat.parentId)) {
        const list = childrenMap.get(cat.parentId) || [];
        list.push(cat);
        childrenMap.set(cat.parentId, list);
      } else {
        roots.push(cat);
      }
    }

    return roots.map((root): CategoryWithChildren => {
      const subcategories = childrenMap.get(root.id) || [];
      const totalTopicCount =
        root.topicCount +
        subcategories.reduce((acc, sub) => acc + sub.topicCount, 0);
      const totalReplyCount =
        root.replyCount +
        subcategories.reduce((acc, sub) => acc + sub.replyCount, 0);

      return {
        category: root,
        subcategories,
        totalTopicCount,
        totalReplyCount,
      };
    });
  }, [categories]);

  // Filter hierarchy based on search query
  const filteredHierarchy = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return hierarchy;

    return hierarchy
      .map((item) => {
        const matchesRoot =
          item.category.name.toLowerCase().includes(q) ||
          item.category.slug.toLowerCase().includes(q) ||
          (item.category.description?.toLowerCase().includes(q) ?? false);

        const matchingSubs = item.subcategories.filter(
          (sub) =>
            sub.name.toLowerCase().includes(q) ||
            sub.slug.toLowerCase().includes(q) ||
            (sub.description?.toLowerCase().includes(q) ?? false)
        );

        if (matchesRoot || matchingSubs.length > 0) {
          return {
            ...item,
            subcategories: matchingSubs.length > 0 ? matchingSubs : item.subcategories,
          };
        }
        return null;
      })
      .filter((item): item is CategoryWithChildren => Boolean(item));
  }, [hierarchy, searchQuery]);

  // Aggregate metrics
  const totalSubcategories = useMemo(
    () => hierarchy.reduce((acc, item) => acc + item.subcategories.length, 0),
    [hierarchy]
  );
  const aggregateTopics = useMemo(
    () => hierarchy.reduce((acc, item) => acc + item.totalTopicCount, 0),
    [hierarchy]
  );

  return (
    <div className={`space-y-6 ${className}`}>
      {/* Top Controls & Search Bar */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="flex flex-wrap items-center gap-3 text-xs font-medium text-muted-foreground">
          <span className="inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1 font-semibold text-primary">
            <span className="h-2 w-2 rounded-full bg-primary" />
            {hierarchy.length} Root Categories
          </span>
          {totalSubcategories > 0 && (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-muted px-3 py-1 text-muted-foreground">
              {totalSubcategories} Subcategories
            </span>
          )}
          <span className="inline-flex items-center gap-1.5 rounded-full bg-muted px-3 py-1 text-muted-foreground">
            {aggregateTopics} Total Topics
          </span>
        </div>

        {/* Realtime Search Input */}
        <div className="relative min-w-64 max-w-sm">
          <input
            type="search"
            placeholder="Filter categories or subcategories…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition placeholder:text-muted-foreground focus:border-primary focus:ring-1 focus:ring-primary"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery('')}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground"
            >
              Clear
            </button>
          )}
        </div>
      </div>

      {/* Categories Grid */}
      {filteredHierarchy.length === 0 ? (
        <div className="rounded-[1.75rem] border border-dashed border-border p-12 text-center">
          <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
            📁
          </div>
          <h3 className="mt-4 text-base font-semibold text-foreground">
            No matching categories found
          </h3>
          <p className="mt-1 text-sm text-muted-foreground">
            Try searching with a different term or clear the filter.
          </p>
        </div>
      ) : (
        <div className="grid gap-6 md:grid-cols-2">
          {filteredHierarchy.map(({ category, subcategories, totalTopicCount, totalReplyCount }) => {
            const accentColor = category.color || '#3b82f6';

            return (
              <div
                key={category.id}
                className="group relative flex flex-col justify-between overflow-hidden rounded-[1.75rem] border border-border bg-card p-6 shadow-sm transition hover:border-primary/40 hover:shadow-md"
              >
                {/* Left Accent Bar */}
                <span
                  className="absolute inset-y-0 left-0 w-1.5 transition-all group-hover:w-2"
                  style={{ backgroundColor: accentColor }}
                />

                <div className="pl-3">
                  {/* Category Header */}
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex items-start gap-3">
                      <div
                        className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl text-lg font-bold shadow-sm"
                        style={{
                          backgroundColor: `${accentColor}18`,
                          color: accentColor,
                        }}
                      >
                        {category.icon ? category.icon : category.name.slice(0, 2).toUpperCase()}
                      </div>
                      <div>
                        <button
                          type="button"
                          onClick={() => onSelectCategory(category.id)}
                          className="text-left text-lg font-semibold text-card-foreground transition hover:text-primary"
                        >
                          {category.name}
                        </button>
                        <div className="mt-0.5 flex items-center gap-2">
                          <span className="font-mono text-xs text-muted-foreground">
                            /{category.slug}
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Quick Browse Button */}
                    <button
                      type="button"
                      onClick={() => onSelectCategory(category.id)}
                      className="shrink-0 rounded-full border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition hover:border-primary hover:bg-muted"
                    >
                      Browse →
                    </button>
                  </div>

                  {/* Description */}
                  <p className="mt-3 text-sm leading-6 text-muted-foreground line-clamp-2">
                    {category.description || 'No description provided for this section.'}
                  </p>

                  {/* Subcategories (Discourse / NodeBB Style Pill Badges) */}
                  {subcategories.length > 0 && (
                    <div className="mt-5 space-y-2 border-t border-border/60 pt-4">
                      <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                        Subcategories
                      </p>
                      <div className="flex flex-wrap gap-2">
                        {subcategories.map((sub) => {
                          const subColor = sub.color || accentColor;
                          return (
                            <button
                              key={sub.id}
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                onSelectCategory(sub.id);
                              }}
                              className="group/sub inline-flex items-center gap-1.5 rounded-full border border-border/80 bg-background/80 px-3 py-1 text-xs font-medium text-foreground transition hover:border-primary/50 hover:bg-muted hover:shadow-sm"
                            >
                              <span
                                className="h-2 w-2 rounded-full transition-transform group-hover/sub:scale-125"
                                style={{ backgroundColor: subColor }}
                              />
                              <span>{sub.name}</span>
                              <span className="text-[10px] text-muted-foreground">
                                {sub.topicCount}
                              </span>
                            </button>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </div>

                {/* Footer Metrics */}
                <div className="mt-6 flex items-center justify-between border-t border-border/40 pl-3 pt-4 text-xs font-medium text-muted-foreground">
                  <div className="flex items-center gap-4">
                    <span className="flex items-center gap-1">
                      <span className="font-semibold text-foreground">{totalTopicCount}</span> topics
                    </span>
                    <span className="flex items-center gap-1">
                      <span className="font-semibold text-foreground">{totalReplyCount}</span> replies
                    </span>
                  </div>
                  <span className="text-[11px] text-muted-foreground">
                    lang: {category.effectiveLocale}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
