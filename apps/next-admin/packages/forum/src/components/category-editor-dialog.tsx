'use client';

import React, { useState, useEffect } from 'react';
import type {
  AdminCategoryTreeNode,
  CreateAdminCategoryInput,
  UpdateAdminCategoryInput,
  GqlOpts
} from '../api/forum';
import {
  createAdminCategory,
  updateAdminCategory,
  archiveAdminCategorySubtree,
  restoreAdminCategorySubtree
} from '../api/forum';

interface CategoryEditorDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSaved: () => void;
  initialCategory?: AdminCategoryTreeNode | null;
  parentCategory?: AdminCategoryTreeNode | null;
  availableParents: AdminCategoryTreeNode[];
  currentLocale: string;
  gqlOpts: GqlOpts;
}

const PRESET_COLORS = [
  '#3b82f6', // Blue
  '#10b981', // Emerald
  '#f59e0b', // Amber
  '#ef4444', // Red
  '#8b5cf6', // Violet
  '#ec4899', // Pink
  '#06b6d4', // Cyan
  '#64748b'  // Slate
];

export function CategoryEditorDialog({
  isOpen,
  onClose,
  onSaved,
  initialCategory,
  parentCategory,
  availableParents,
  currentLocale,
  gqlOpts
}: CategoryEditorDialogProps) {
  const isEditing = Boolean(initialCategory);

  const [locale, setLocale] = useState(initialCategory?.requestedLocale || currentLocale);
  const [name, setName] = useState(initialCategory?.name || '');
  const [slug, setSlug] = useState(initialCategory?.slug || '');
  const [description, setDescription] = useState(initialCategory?.description || '');
  const [parentId, setParentId] = useState<string>(
    initialCategory?.parentId || parentCategory?.id || ''
  );
  const [color, setColor] = useState(initialCategory?.color || '#3b82f6');
  const [icon, setIcon] = useState(initialCategory?.icon || '');
  const [moderated, setModerated] = useState(initialCategory?.moderated ?? false);
  const [allowsTopics, setAllowsTopics] = useState(initialCategory?.allowsTopics ?? true);

  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isArchiving, setIsArchiving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initialCategory) {
      setLocale(initialCategory.requestedLocale || currentLocale);
      setName(initialCategory.name);
      setSlug(initialCategory.slug);
      setDescription(initialCategory.description || '');
      setParentId(initialCategory.parentId || '');
      setColor(initialCategory.color || '#3b82f6');
      setIcon(initialCategory.icon || '');
      setModerated(initialCategory.moderated);
      setAllowsTopics(initialCategory.allowsTopics);
    } else {
      setLocale(currentLocale);
      setName('');
      setSlug('');
      setDescription('');
      setParentId(parentCategory?.id || '');
      setColor('#3b82f6');
      setIcon('');
      setModerated(false);
      setAllowsTopics(true);
    }
    setError(null);
  }, [initialCategory, parentCategory, currentLocale, isOpen]);

  // Auto-slugify name if creating
  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setName(val);
    if (!isEditing && (!slug || slug === name.toLowerCase().replace(/[^a-z0-9]+/g, '-'))) {
      setSlug(
        val
          .toLowerCase()
          .trim()
          .replace(/[^a-z0-9]+/g, '-')
          .replace(/^-+|-+$/g, '')
      );
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Category name is required.');
      return;
    }
    if (!slug.trim()) {
      setError('Slug is required.');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      if (isEditing && initialCategory) {
        const updateInput: UpdateAdminCategoryInput = {
          locale,
          name: name.trim(),
          slug: slug.trim(),
          description: description.trim() || null,
          color: color || null,
          icon: icon.trim() || null,
          moderated,
          allowsTopics
        };
        await updateAdminCategory(initialCategory.id, updateInput, gqlOpts);
      } else {
        const createInput: CreateAdminCategoryInput = {
          locale,
          name: name.trim(),
          slug: slug.trim(),
          description: description.trim() || null,
          color: color || null,
          icon: icon.trim() || null,
          parentId: parentId || null,
          moderated,
          allowsTopics
        };
        await createAdminCategory(createInput, gqlOpts);
      }
      onSaved();
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to save category');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleToggleSubtreeArchive = async () => {
    if (!initialCategory) return;
    setIsArchiving(true);
    setError(null);
    try {
      if (initialCategory.isArchived) {
        await restoreAdminCategorySubtree(initialCategory.id, gqlOpts);
      } else {
        await archiveAdminCategorySubtree(initialCategory.id, gqlOpts);
      }
      onSaved();
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to change archive state');
    } finally {
      setIsArchiving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-xs animate-in fade-in">
      <div className="relative w-full max-w-xl overflow-hidden rounded-[2rem] border border-border bg-card p-6 shadow-2xl sm:p-8 max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border pb-4">
          <div>
            <h3 className="text-xl font-semibold text-card-foreground">
              {isEditing ? `Edit Category: ${initialCategory?.name}` : 'New Category'}
            </h3>
            <p className="mt-1 text-xs text-muted-foreground">
              {isEditing
                ? 'Update category parameters, routing identifiers, and topic policies.'
                : 'Configure category taxonomy, styling, and posting rules.'}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full border border-border p-2 text-muted-foreground transition hover:bg-muted hover:text-foreground"
          >
            ✕
          </button>
        </div>

        {error && (
          <div className="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
            {error}
          </div>
        )}

        {/* Scrollable Form Body */}
        <form onSubmit={handleSubmit} className="mt-6 flex-1 space-y-4 overflow-y-auto pr-1">
          {/* Locale & Parent Selector */}
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Locale
              </label>
              <input
                type="text"
                value={locale}
                onChange={(e) => setLocale(e.target.value)}
                placeholder="e.g. en"
                required
                className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition focus:border-primary"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Parent Category
              </label>
              <select
                value={parentId}
                onChange={(e) => setParentId(e.target.value)}
                disabled={isEditing}
                className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition focus:border-primary disabled:opacity-60"
              >
                <option value="">(Root Category - No Parent)</option>
                {availableParents
                  .filter((p) => !isEditing || p.id !== initialCategory?.id)
                  .map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.depth > 0 ? '— '.repeat(p.depth) : ''}
                      {p.name} (/{p.slug})
                    </option>
                  ))}
              </select>
            </div>
          </div>

          {/* Name & Slug */}
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Category Name
              </label>
              <input
                type="text"
                value={name}
                onChange={handleNameChange}
                placeholder="e.g. Product Feedback"
                required
                className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition focus:border-primary"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Route Slug
              </label>
              <input
                type="text"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder="e.g. product-feedback"
                required
                className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 font-mono text-sm outline-none transition focus:border-primary"
              />
            </div>
          </div>

          {/* Description */}
          <div>
            <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
              Description
            </label>
            <textarea
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Detailed description of what discussions belong here…"
              className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition focus:border-primary"
            />
          </div>

          {/* Color & Icon */}
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Accent Color
              </label>
              <div className="mt-1.5 flex items-center gap-2">
                <input
                  type="color"
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  className="h-9 w-10 cursor-pointer rounded-lg border border-border bg-background p-1"
                />
                <input
                  type="text"
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  placeholder="#3b82f6"
                  className="flex-1 rounded-xl border border-border bg-background px-3 py-2 font-mono text-xs uppercase outline-none focus:border-primary"
                />
              </div>
              {/* Preset Chips */}
              <div className="mt-2 flex flex-wrap gap-1.5">
                {PRESET_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    onClick={() => setColor(c)}
                    className={`h-5 w-5 rounded-full border transition hover:scale-110 ${
                      color.toLowerCase() === c.toLowerCase() ? 'ring-2 ring-primary ring-offset-1' : ''
                    }`}
                    style={{ backgroundColor: c }}
                  />
                ))}
              </div>
            </div>

            <div>
              <label className="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                Icon / Emoji
              </label>
              <input
                type="text"
                value={icon}
                onChange={(e) => setIcon(e.target.value)}
                placeholder="e.g. 💬, 🚀, or icon-slug"
                className="mt-1.5 w-full rounded-xl border border-border bg-background px-3.5 py-2 text-sm outline-none transition focus:border-primary"
              />
              <p className="mt-1 text-[11px] text-muted-foreground">
                Single emoji or icon identifier displayed in storefront rails.
              </p>
            </div>
          </div>

          {/* Policy & Moderation Toggles */}
          <div className="space-y-3 rounded-2xl border border-border bg-muted/20 p-4">
            <label className="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={allowsTopics}
                onChange={(e) => setAllowsTopics(e.target.checked)}
                className="mt-1 h-4 w-4 rounded border-border text-primary focus:ring-primary"
              />
              <div>
                <span className="block text-sm font-medium text-foreground">
                  Allows Direct Topics
                </span>
                <span className="block text-xs text-muted-foreground">
                  If enabled, users can post topics directly in this category. If disabled, it acts as a category container for subcategories only.
                </span>
              </div>
            </label>

            <label className="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={moderated}
                onChange={(e) => setModerated(e.target.checked)}
                className="mt-1 h-4 w-4 rounded border-border text-primary focus:ring-primary"
              />
              <div>
                <span className="block text-sm font-medium text-foreground">
                  Require Moderation
                </span>
                <span className="block text-xs text-muted-foreground">
                  Topics and replies require moderator approval before becoming visible.
                </span>
              </div>
            </label>
          </div>

          {/* Footer Actions */}
          <div className="flex flex-wrap items-center justify-between gap-3 pt-4 border-t border-border">
            {isEditing && (
              <button
                type="button"
                onClick={handleToggleSubtreeArchive}
                disabled={isArchiving || isSubmitting}
                className="rounded-full border border-destructive/40 bg-destructive/10 px-4 py-2 text-xs font-medium text-destructive transition hover:bg-destructive/20 disabled:opacity-50"
              >
                {initialCategory?.isArchived ? 'Restore Subtree' : 'Archive Subtree'}
              </button>
            )}

            <div className="flex items-center gap-2 ml-auto">
              <button
                type="button"
                onClick={onClose}
                className="rounded-full border border-border px-4 py-2 text-sm font-medium text-muted-foreground transition hover:bg-muted hover:text-foreground"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting}
                className="rounded-full bg-primary px-5 py-2 text-sm font-medium text-primary-foreground transition hover:opacity-95 disabled:opacity-50 shadow-xs"
              >
                {isSubmitting ? 'Saving…' : isEditing ? 'Save Changes' : 'Create Category'}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
