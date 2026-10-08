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
  '#64748b' // Slate
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

  const [locale, setLocale] = useState(
    initialCategory?.requestedLocale || currentLocale
  );
  const [name, setName] = useState(initialCategory?.name || '');
  const [slug, setSlug] = useState(initialCategory?.slug || '');
  const [description, setDescription] = useState(
    initialCategory?.description || ''
  );
  const [parentId, setParentId] = useState<string>(
    initialCategory?.parentId || parentCategory?.id || ''
  );
  const [color, setColor] = useState(initialCategory?.color || '#3b82f6');
  const [icon, setIcon] = useState(initialCategory?.icon || '');
  const [moderated, setModerated] = useState(
    initialCategory?.moderated ?? false
  );
  const [allowsTopics, setAllowsTopics] = useState(
    initialCategory?.allowsTopics ?? true
  );

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
    if (
      !isEditing &&
      (!slug || slug === name.toLowerCase().replace(/[^a-z0-9]+/g, '-'))
    ) {
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
      setError(
        err instanceof Error ? err.message : 'Failed to change archive state'
      );
    } finally {
      setIsArchiving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className='animate-in fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-xs'>
      <div className='border-border bg-card relative flex max-h-[90vh] w-full max-w-xl flex-col overflow-hidden rounded-[2rem] border p-6 shadow-2xl sm:p-8'>
        {/* Header */}
        <div className='border-border flex items-center justify-between border-b pb-4'>
          <div>
            <h3 className='text-card-foreground text-xl font-semibold'>
              {isEditing
                ? `Edit Category: ${initialCategory?.name}`
                : 'New Category'}
            </h3>
            <p className='text-muted-foreground mt-1 text-xs'>
              {isEditing
                ? 'Update category parameters, routing identifiers, and topic policies.'
                : 'Configure category taxonomy, styling, and posting rules.'}
            </p>
          </div>
          <button
            type='button'
            onClick={onClose}
            className='border-border text-muted-foreground hover:bg-muted hover:text-foreground rounded-full border p-2 transition'
          >
            ✕
          </button>
        </div>

        {error && (
          <div className='border-destructive/30 bg-destructive/10 text-destructive mt-4 rounded-xl border px-4 py-2 text-xs'>
            {error}
          </div>
        )}

        {/* Scrollable Form Body */}
        <form
          onSubmit={handleSubmit}
          className='mt-6 flex-1 space-y-4 overflow-y-auto pr-1'
        >
          {/* Locale & Parent Selector */}
          <div className='grid gap-4 sm:grid-cols-2'>
            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Locale
              </label>
              <input
                type='text'
                value={locale}
                onChange={(e) => setLocale(e.target.value)}
                placeholder='e.g. en'
                required
                className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 text-sm transition outline-none'
              />
            </div>

            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Parent Category
              </label>
              <select
                value={parentId}
                onChange={(e) => setParentId(e.target.value)}
                disabled={isEditing}
                className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 text-sm transition outline-none disabled:opacity-60'
              >
                <option value=''>(Root Category - No Parent)</option>
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
          <div className='grid gap-4 sm:grid-cols-2'>
            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Category Name
              </label>
              <input
                type='text'
                value={name}
                onChange={handleNameChange}
                placeholder='e.g. Product Feedback'
                required
                className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 text-sm transition outline-none'
              />
            </div>

            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Route Slug
              </label>
              <input
                type='text'
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
                placeholder='e.g. product-feedback'
                required
                className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 font-mono text-sm transition outline-none'
              />
            </div>
          </div>

          {/* Description */}
          <div>
            <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
              Description
            </label>
            <textarea
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder='Detailed description of what discussions belong here…'
              className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 text-sm transition outline-none'
            />
          </div>

          {/* Color & Icon */}
          <div className='grid gap-4 sm:grid-cols-2'>
            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Accent Color
              </label>
              <div className='mt-1.5 flex items-center gap-2'>
                <input
                  type='color'
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  className='border-border bg-background h-9 w-10 cursor-pointer rounded-lg border p-1'
                />
                <input
                  type='text'
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  placeholder='#3b82f6'
                  className='border-border bg-background focus:border-primary flex-1 rounded-xl border px-3 py-2 font-mono text-xs uppercase outline-none'
                />
              </div>
              {/* Preset Chips */}
              <div className='mt-2 flex flex-wrap gap-1.5'>
                {PRESET_COLORS.map((c) => (
                  <button
                    key={c}
                    type='button'
                    onClick={() => setColor(c)}
                    className={`h-5 w-5 rounded-full border transition hover:scale-110 ${
                      color.toLowerCase() === c.toLowerCase()
                        ? 'ring-primary ring-2 ring-offset-1'
                        : ''
                    }`}
                    style={{ backgroundColor: c }}
                  />
                ))}
              </div>
            </div>

            <div>
              <label className='text-muted-foreground block text-xs font-semibold tracking-wider uppercase'>
                Icon / Emoji
              </label>
              <input
                type='text'
                value={icon}
                onChange={(e) => setIcon(e.target.value)}
                placeholder='e.g. 💬, 🚀, or icon-slug'
                className='border-border bg-background focus:border-primary mt-1.5 w-full rounded-xl border px-3.5 py-2 text-sm transition outline-none'
              />
              <p className='text-muted-foreground mt-1 text-[11px]'>
                Single emoji or icon identifier displayed in storefront rails.
              </p>
            </div>
          </div>

          {/* Policy & Moderation Toggles */}
          <div className='border-border bg-muted/20 space-y-3 rounded-2xl border p-4'>
            <label className='flex cursor-pointer items-start gap-3'>
              <input
                type='checkbox'
                checked={allowsTopics}
                onChange={(e) => setAllowsTopics(e.target.checked)}
                className='border-border text-primary focus:ring-primary mt-1 h-4 w-4 rounded'
              />
              <div>
                <span className='text-foreground block text-sm font-medium'>
                  Allows Direct Topics
                </span>
                <span className='text-muted-foreground block text-xs'>
                  If enabled, users can post topics directly in this category.
                  If disabled, it acts as a category container for subcategories
                  only.
                </span>
              </div>
            </label>

            <label className='flex cursor-pointer items-start gap-3'>
              <input
                type='checkbox'
                checked={moderated}
                onChange={(e) => setModerated(e.target.checked)}
                className='border-border text-primary focus:ring-primary mt-1 h-4 w-4 rounded'
              />
              <div>
                <span className='text-foreground block text-sm font-medium'>
                  Require Moderation
                </span>
                <span className='text-muted-foreground block text-xs'>
                  Topics and replies require moderator approval before becoming
                  visible.
                </span>
              </div>
            </label>
          </div>

          {/* Footer Actions */}
          <div className='border-border flex flex-wrap items-center justify-between gap-3 border-t pt-4'>
            {isEditing && (
              <button
                type='button'
                onClick={handleToggleSubtreeArchive}
                disabled={isArchiving || isSubmitting}
                className='border-destructive/40 bg-destructive/10 text-destructive hover:bg-destructive/20 rounded-full border px-4 py-2 text-xs font-medium transition disabled:opacity-50'
              >
                {initialCategory?.isArchived
                  ? 'Restore Subtree'
                  : 'Archive Subtree'}
              </button>
            )}

            <div className='ml-auto flex items-center gap-2'>
              <button
                type='button'
                onClick={onClose}
                className='border-border text-muted-foreground hover:bg-muted hover:text-foreground rounded-full border px-4 py-2 text-sm font-medium transition'
              >
                Cancel
              </button>
              <button
                type='submit'
                disabled={isSubmitting}
                className='bg-primary text-primary-foreground rounded-full px-5 py-2 text-sm font-medium shadow-xs transition hover:opacity-95 disabled:opacity-50'
              >
                {isSubmitting
                  ? 'Saving…'
                  : isEditing
                    ? 'Save Changes'
                    : 'Create Category'}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
