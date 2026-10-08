/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

'use client';

import * as React from 'react';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { Input } from '@/shared/ui/shadcn/input';
import { Label } from '@/shared/ui/shadcn/label';
import { Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import type {
  BindCategoryAttributePayload,
  BindSchemaAttributePayload,
  CatalogCategorySummary,
  CreateCategoryAttributeGroupPayload,
  CreateProductAttributeSchemaGroupPayload,
  ProductAttributeSchemaSummary,
  ProductAttributeSummary,
  SetCategorySchemaModePayload
} from '../../api/types';

interface SchemaAuthoringCardProps {
  categories: CatalogCategorySummary[];
  schemas: ProductAttributeSchemaSummary[];
  attributes: ProductAttributeSummary[];
  onSetSchemaMode: (payload: SetCategorySchemaModePayload) => Promise<void>;
  onCreateSchemaGroup: (
    payload: CreateProductAttributeSchemaGroupPayload
  ) => Promise<void>;
  onCreateCategoryGroup: (
    payload: CreateCategoryAttributeGroupPayload
  ) => Promise<void>;
  onBindSchemaAttribute: (payload: BindSchemaAttributePayload) => Promise<void>;
  onBindCategoryAttribute: (
    payload: BindCategoryAttributePayload
  ) => Promise<void>;
}

const selectClassName =
  'border-input bg-background ring-offset-background focus-visible:ring-ring h-9 w-full rounded-md border px-3 py-1 text-sm focus-visible:ring-1 focus-visible:outline-none disabled:opacity-50';

/**
 * Category-schema authoring surface.
 *
 * The five owner commands behind this card (schema mode, schema groups,
 * category groups, schema attribute bindings, category attribute bindings) were
 * previously reachable only through raw GraphQL. Every section reports the
 * owner result instead of discarding it, and schema mode/group edits reuse the
 * owner-provided enums (`inherit`, `use_schema`, `clone_from_category`,
 * `custom`, and `addition`/`override`/`removal`).
 */
export function SchemaAuthoringCard({
  categories,
  schemas,
  attributes,
  onSetSchemaMode,
  onCreateSchemaGroup,
  onCreateCategoryGroup,
  onBindSchemaAttribute,
  onBindCategoryAttribute
}: SchemaAuthoringCardProps) {
  const [pending, setPending] = React.useState<string | null>(null);

  // 1. Category schema mode
  const [modeCategory, setModeCategory] = React.useState('');
  const [mode, setMode] = React.useState('inherit');
  const [modeSchema, setModeSchema] = React.useState('');
  const [modeCloneFrom, setModeCloneFrom] = React.useState('');

  // 2. Schema group
  const [groupSchema, setGroupSchema] = React.useState('');
  const [groupCode, setGroupCode] = React.useState('');
  const [groupLabel, setGroupLabel] = React.useState('');
  const [groupPosition, setGroupPosition] = React.useState('0');

  // 3. Schema attribute binding
  const [bindSchema, setBindSchema] = React.useState('');
  const [bindSchemaAttribute, setBindSchemaAttribute] = React.useState('');
  const [bindSchemaGroup, setBindSchemaGroup] = React.useState('');
  const [bindSchemaRequired, setBindSchemaRequired] = React.useState(false);
  const [bindSchemaDisabled, setBindSchemaDisabled] = React.useState(false);
  const [bindSchemaPosition, setBindSchemaPosition] = React.useState('0');

  // 4. Category group
  const [categoryGroupCategory, setCategoryGroupCategory] = React.useState('');
  const [categoryGroupCode, setCategoryGroupCode] = React.useState('');
  const [categoryGroupLabel, setCategoryGroupLabel] = React.useState('');
  const [categoryGroupPosition, setCategoryGroupPosition] = React.useState('0');

  // 5. Category attribute binding
  const [bindCategory, setBindCategory] = React.useState('');
  const [bindCategoryAttribute, setBindCategoryAttribute] = React.useState('');
  const [bindCategoryGroup, setBindCategoryGroup] = React.useState('');
  const [bindCategoryKind, setBindCategoryKind] = React.useState('addition');
  const [bindCategoryDisabled, setBindCategoryDisabled] = React.useState(false);
  const [bindCategoryPosition, setBindCategoryPosition] = React.useState('0');

  const run = async (
    key: string,
    action: () => Promise<void>,
    success: string
  ) => {
    setPending(key);
    try {
      await action();
      toast.success(success);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'Schema authoring failed'
      );
    } finally {
      setPending(null);
    }
  };

  const position = (raw: string) => {
    const parsed = Number.parseInt(raw, 10);
    return Number.isNaN(parsed) ? 0 : parsed;
  };

  return (
    <Card>
      <CardHeader className='pb-3'>
        <CardTitle className='text-base'>Category Schema Authoring</CardTitle>
        <CardDescription>
          Schema mode, attribute groups and attribute bindings for categories
          and schema templates. Owner rejections surface as errors instead of
          silent no-ops.
        </CardDescription>
      </CardHeader>
      <CardContent className='space-y-6'>
        <section className='space-y-3 rounded-lg border p-4'>
          <h3 className='text-sm font-semibold'>Category schema mode</h3>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-mode-category'>Category</Label>
              <select
                id='schema-mode-category'
                className={selectClassName}
                value={modeCategory}
                onChange={(event) => setModeCategory(event.target.value)}
              >
                <option value=''>Select a category</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-mode-mode'>Mode</Label>
              <select
                id='schema-mode-mode'
                className={selectClassName}
                value={mode}
                onChange={(event) => setMode(event.target.value)}
              >
                <option value='inherit'>inherit</option>
                <option value='use_schema'>use_schema</option>
                <option value='clone_from_category'>clone_from_category</option>
                <option value='custom'>custom</option>
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-mode-schema'>Schema (use_schema)</Label>
              <select
                id='schema-mode-schema'
                className={selectClassName}
                value={modeSchema}
                disabled={mode !== 'use_schema'}
                onChange={(event) => setModeSchema(event.target.value)}
              >
                <option value=''>No schema</option>
                {schemas.map((schema) => (
                  <option key={schema.id} value={schema.id}>
                    {schema.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-mode-clone'>Clone from category</Label>
              <select
                id='schema-mode-clone'
                className={selectClassName}
                value={modeCloneFrom}
                disabled={mode !== 'clone_from_category'}
                onChange={(event) => setModeCloneFrom(event.target.value)}
              >
                <option value=''>No source category</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <Button
            type='button'
            size='sm'
            disabled={pending !== null}
            onClick={() => {
              if (!modeCategory) {
                toast.error('Select a category');
                return;
              }
              void run(
                'mode',
                () =>
                  onSetSchemaMode({
                    categoryId: modeCategory,
                    mode,
                    schemaId: mode === 'use_schema' ? modeSchema || null : null,
                    cloneFromCategoryId:
                      mode === 'clone_from_category'
                        ? modeCloneFrom || null
                        : null
                  }),
                'Category schema mode updated'
              );
            }}
          >
            {pending === 'mode' ? (
              <Loader2 className='mr-2 h-4 w-4 animate-spin' />
            ) : null}
            Apply mode
          </Button>
        </section>

        <section className='space-y-3 rounded-lg border p-4'>
          <h3 className='text-sm font-semibold'>Schema attribute group</h3>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-group-schema'>Schema</Label>
              <select
                id='schema-group-schema'
                className={selectClassName}
                value={groupSchema}
                onChange={(event) => setGroupSchema(event.target.value)}
              >
                <option value=''>Select a schema</option>
                {schemas.map((schema) => (
                  <option key={schema.id} value={schema.id}>
                    {schema.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-group-code'>Group code</Label>
              <Input
                id='schema-group-code'
                value={groupCode}
                placeholder='specs'
                onChange={(event) => setGroupCode(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-group-label'>Group label</Label>
              <Input
                id='schema-group-label'
                value={groupLabel}
                placeholder='Specs'
                onChange={(event) => setGroupLabel(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-group-position'>Position</Label>
              <Input
                id='schema-group-position'
                type='number'
                value={groupPosition}
                onChange={(event) => setGroupPosition(event.target.value)}
              />
            </div>
          </div>
          <Button
            type='button'
            size='sm'
            disabled={pending !== null}
            onClick={() => {
              if (!groupSchema || !groupCode.trim()) {
                toast.error('Select a schema and provide the group code');
                return;
              }
              void run(
                'schema-group',
                () =>
                  onCreateSchemaGroup({
                    schemaId: groupSchema,
                    code: groupCode,
                    label: groupLabel.trim() || groupCode,
                    position: position(groupPosition)
                  }),
                'Schema attribute group created'
              );
            }}
          >
            {pending === 'schema-group' ? (
              <Loader2 className='mr-2 h-4 w-4 animate-spin' />
            ) : null}
            Create schema group
          </Button>
        </section>

        <section className='space-y-3 rounded-lg border p-4'>
          <h3 className='text-sm font-semibold'>Schema attribute binding</h3>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-bind-schema'>Schema</Label>
              <select
                id='schema-bind-schema'
                className={selectClassName}
                value={bindSchema}
                onChange={(event) => setBindSchema(event.target.value)}
              >
                <option value=''>Select a schema</option>
                {schemas.map((schema) => (
                  <option key={schema.id} value={schema.id}>
                    {schema.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-bind-attribute'>Attribute</Label>
              <select
                id='schema-bind-attribute'
                className={selectClassName}
                value={bindSchemaAttribute}
                onChange={(event) => setBindSchemaAttribute(event.target.value)}
              >
                <option value=''>Select an attribute</option>
                {attributes.map((attribute) => (
                  <option key={attribute.id} value={attribute.id}>
                    {attribute.label}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-bind-group'>Group code (optional)</Label>
              <Input
                id='schema-bind-group'
                value={bindSchemaGroup}
                placeholder='specs'
                onChange={(event) => setBindSchemaGroup(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='schema-bind-position'>Position</Label>
              <Input
                id='schema-bind-position'
                type='number'
                value={bindSchemaPosition}
                onChange={(event) => setBindSchemaPosition(event.target.value)}
              />
            </div>
          </div>
          <div className='flex flex-wrap items-center gap-4 text-sm'>
            <label className='flex items-center gap-2'>
              <input
                type='checkbox'
                checked={bindSchemaRequired}
                onChange={(event) =>
                  setBindSchemaRequired(event.target.checked)
                }
              />
              Required
            </label>
            <label className='flex items-center gap-2'>
              <input
                type='checkbox'
                checked={bindSchemaDisabled}
                onChange={(event) =>
                  setBindSchemaDisabled(event.target.checked)
                }
              />
              Disabled
            </label>
          </div>
          <Button
            type='button'
            size='sm'
            disabled={pending !== null}
            onClick={() => {
              if (!bindSchema || !bindSchemaAttribute) {
                toast.error('Select a schema and an attribute');
                return;
              }
              void run(
                'schema-bind',
                () =>
                  onBindSchemaAttribute({
                    schemaId: bindSchema,
                    attributeId: bindSchemaAttribute,
                    groupCode: bindSchemaGroup.trim() || null,
                    isRequired: bindSchemaRequired,
                    isDisabled: bindSchemaDisabled,
                    position: position(bindSchemaPosition)
                  }),
                'Attribute bound to schema'
              );
            }}
          >
            {pending === 'schema-bind' ? (
              <Loader2 className='mr-2 h-4 w-4 animate-spin' />
            ) : null}
            Bind attribute to schema
          </Button>
        </section>

        <section className='space-y-3 rounded-lg border p-4'>
          <h3 className='text-sm font-semibold'>Category attribute group</h3>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-group-category'>Category</Label>
              <select
                id='category-group-category'
                className={selectClassName}
                value={categoryGroupCategory}
                onChange={(event) =>
                  setCategoryGroupCategory(event.target.value)
                }
              >
                <option value=''>Select a category</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-group-code'>Group code</Label>
              <Input
                id='category-group-code'
                value={categoryGroupCode}
                placeholder='details'
                onChange={(event) => setCategoryGroupCode(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-group-label'>Group label</Label>
              <Input
                id='category-group-label'
                value={categoryGroupLabel}
                placeholder='Details'
                onChange={(event) => setCategoryGroupLabel(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-group-position'>Position</Label>
              <Input
                id='category-group-position'
                type='number'
                value={categoryGroupPosition}
                onChange={(event) =>
                  setCategoryGroupPosition(event.target.value)
                }
              />
            </div>
          </div>
          <Button
            type='button'
            size='sm'
            disabled={pending !== null}
            onClick={() => {
              if (!categoryGroupCategory || !categoryGroupCode.trim()) {
                toast.error('Select a category and provide the group code');
                return;
              }
              void run(
                'category-group',
                () =>
                  onCreateCategoryGroup({
                    categoryId: categoryGroupCategory,
                    code: categoryGroupCode,
                    label: categoryGroupLabel.trim() || categoryGroupCode,
                    position: position(categoryGroupPosition)
                  }),
                'Category attribute group created'
              );
            }}
          >
            {pending === 'category-group' ? (
              <Loader2 className='mr-2 h-4 w-4 animate-spin' />
            ) : null}
            Create category group
          </Button>
        </section>

        <section className='space-y-3 rounded-lg border p-4'>
          <h3 className='text-sm font-semibold'>Category attribute binding</h3>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-bind-category'>Category</Label>
              <select
                id='category-bind-category'
                className={selectClassName}
                value={bindCategory}
                onChange={(event) => setBindCategory(event.target.value)}
              >
                <option value=''>Select a category</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-bind-attribute'>Attribute</Label>
              <select
                id='category-bind-attribute'
                className={selectClassName}
                value={bindCategoryAttribute}
                onChange={(event) =>
                  setBindCategoryAttribute(event.target.value)
                }
              >
                <option value=''>Select an attribute</option>
                {attributes.map((attribute) => (
                  <option key={attribute.id} value={attribute.id}>
                    {attribute.label}
                  </option>
                ))}
              </select>
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-bind-group'>Group code (optional)</Label>
              <Input
                id='category-bind-group'
                value={bindCategoryGroup}
                placeholder='details'
                onChange={(event) => setBindCategoryGroup(event.target.value)}
              />
            </div>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-bind-kind'>Binding kind</Label>
              <select
                id='category-bind-kind'
                className={selectClassName}
                value={bindCategoryKind}
                onChange={(event) => setBindCategoryKind(event.target.value)}
              >
                <option value='addition'>addition</option>
                <option value='override'>override</option>
                <option value='removal'>removal</option>
              </select>
            </div>
          </div>
          <div className='grid gap-3 md:grid-cols-2 xl:grid-cols-4'>
            <div className='grid gap-1.5'>
              <Label htmlFor='category-bind-position'>Position</Label>
              <Input
                id='category-bind-position'
                type='number'
                value={bindCategoryPosition}
                onChange={(event) =>
                  setBindCategoryPosition(event.target.value)
                }
              />
            </div>
            <label className='flex items-end gap-2 text-sm'>
              <input
                type='checkbox'
                checked={bindCategoryDisabled}
                onChange={(event) =>
                  setBindCategoryDisabled(event.target.checked)
                }
              />
              Disabled
            </label>
          </div>
          <Button
            type='button'
            size='sm'
            disabled={pending !== null}
            onClick={() => {
              if (!bindCategory || !bindCategoryAttribute) {
                toast.error('Select a category and an attribute');
                return;
              }
              void run(
                'category-bind',
                () =>
                  onBindCategoryAttribute({
                    categoryId: bindCategory,
                    attributeId: bindCategoryAttribute,
                    groupCode: bindCategoryGroup.trim() || null,
                    bindingKind: bindCategoryKind,
                    isDisabled: bindCategoryDisabled,
                    position: position(bindCategoryPosition)
                  }),
                'Attribute bound to category'
              );
            }}
          >
            {pending === 'category-bind' ? (
              <Loader2 className='mr-2 h-4 w-4 animate-spin' />
            ) : null}
            Bind attribute to category
          </Button>
        </section>
      </CardContent>
    </Card>
  );
}
