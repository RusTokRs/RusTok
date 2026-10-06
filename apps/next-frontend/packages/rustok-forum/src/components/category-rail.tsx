'use client';

import React from 'react';
import type { ForumCategoryListItem } from '../api/forum';

interface CategoryRailProps {
  categories: ForumCategoryListItem[];
  total: number;
  selectedCategoryId: string | null;
  onSelectCategory?: (categoryId: string | null) => void;
  onSwitchToOverview?: () => void;
  baseHref?: string;
}

export function CategoryRail({
  categories,
  total,
  selectedCategoryId,
  onSelectCategory,
  onSwitchToOverview,
  baseHref = '/modules/forum',
}: CategoryRailProps) {
  return (
    <aside className="space-y-4 rounded-[1.75rem] border border-border bg-card p-5 shadow-sm xl:sticky xl:top-6 xl:self-start">
      <div className="flex items-start justify-between gap-2">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
            Categories
          </p>
          <h3 className="mt-2 text-xl font-semibold text-card-foreground">
            Community map
          </h3>
          <p className="mt-2 text-sm leading-6 text-muted-foreground">
            {total} sections published from the forum module.
          </p>
        </div>
        {onSwitchToOverview && (
          <button
            type="button"
            onClick={onSwitchToOverview}
            title="Open category matrix with subcategories"
            className="shrink-0 rounded-full border border-border bg-background p-2 text-xs font-medium text-muted-foreground transition hover:border-primary hover:text-foreground"
          >
            Grid ⊞
          </button>
        )}
      </div>

      <div className="space-y-2">
        {/* All Categories Option */}
        <button
          type="button"
          onClick={() => onSelectCategory?.(null)}
          className={`relative block w-full text-left overflow-hidden rounded-[1.35rem] border p-4 transition ${
            !selectedCategoryId
              ? 'border-primary/50 bg-primary/5 shadow-xs'
              : 'border-border bg-background/50 hover:border-border/80 hover:bg-muted/50'
          }`}
        >
          <span
            className={`absolute inset-y-0 left-0 w-1.5 ${
              !selectedCategoryId ? 'bg-primary' : 'bg-transparent'
            }`}
          />
          <div className="pl-3">
            <h4 className="text-sm font-semibold text-foreground">All Topics</h4>
            <p className="mt-1 text-xs text-muted-foreground">View discussions across all categories</p>
          </div>
        </button>

        {categories.map((category) => {
          const isSelected = selectedCategoryId === category.id;
          const accentColor = category.color || 'var(--primary)';
          const isSubcategory = Boolean(category.parentId);

          return (
            <button
              key={category.id}
              type="button"
              onClick={() => onSelectCategory?.(category.id)}
              className={`relative block w-full text-left overflow-hidden rounded-[1.35rem] border p-4 transition ${
                isSubcategory ? 'ml-3 w-[calc(100%-0.75rem)] border-dashed' : ''
              } ${
                isSelected
                  ? 'border-primary/50 bg-primary/5 shadow-xs'
                  : 'border-border bg-background/50 hover:border-border/80 hover:bg-muted/50'
              }`}
            >
              <span
                className="absolute inset-y-0 left-0 w-1.5"
                style={{ backgroundColor: accentColor }}
              />
              <div className="pl-3">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h4 className="text-sm font-semibold text-foreground flex items-center gap-1.5">
                      {isSubcategory && <span className="text-xs text-muted-foreground">↳</span>}
                      <span>{category.name}</span>
                    </h4>
                    <p className="mt-1 text-xs text-muted-foreground" dir="ltr">
                      c/{category.slug}
                    </p>
                  </div>
                  <span className="rounded-full border border-border px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                    {category.topicCount}
                  </span>
                </div>
                {category.description && (
                  <p className="mt-2 line-clamp-2 text-xs text-muted-foreground">
                    {category.description}
                  </p>
                )}
              </div>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
