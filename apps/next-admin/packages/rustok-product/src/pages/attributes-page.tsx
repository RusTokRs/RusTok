import * as React from 'react';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription
} from '@/shared/ui/shadcn/card';
import {
  listProductAttributes,
  listProductAttributeSchemas
} from '../api/attributes';
import type {
  GqlOpts,
  CreateProductAttributePayload,
  CreateProductAttributeOptionPayload,
  CreateProductAttributeSchemaPayload,
  ProductAttributeSummary,
  ProductAttributeSchemaSummary
} from '../api/types';
import { AttributesTable } from '../components/attributes/attributes-table';
import { AttributeCreateDialog } from '../components/attributes/attribute-create-dialog';
import { AttributeSchemasCard } from '../components/attributes/attribute-schemas-card';

export interface AttributesPageProps {
  token: string | null;
  tenantSlug: string | null;
  tenantId: string | null;
  locale?: string;
  onCreateAttribute: (payload: CreateProductAttributePayload) => Promise<void>;
  onCreateOption: (
    payload: CreateProductAttributeOptionPayload
  ) => Promise<void>;
  onCreateSchema: (
    payload: CreateProductAttributeSchemaPayload
  ) => Promise<void>;
}

export async function AttributesPage({
  token,
  tenantSlug,
  tenantId,
  locale = 'en',
  onCreateAttribute,
  onCreateOption,
  onCreateSchema
}: AttributesPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  let attributes: ProductAttributeSummary[] = [];
  let schemas: ProductAttributeSchemaSummary[] = [];
  let error: string | null = null;

  try {
    const [attrsRes, schemasRes] = await Promise.all([
      listProductAttributes(opts, locale),
      listProductAttributeSchemas(opts, locale)
    ]);
    attributes = attrsRes;
    schemas = schemasRes;
  } catch (err) {
    error =
      err instanceof Error ? err.message : 'Failed to load product attributes.';
  }

  return (
    <div className='space-y-6'>
      <div className='flex items-center justify-between'>
        <div>
          <h2 className='text-lg font-semibold tracking-tight'>
            Product Attributes & Schemas
          </h2>
          <p className='text-muted-foreground text-sm'>
            Define dynamic attributes, option dictionaries, and schema templates
            for catalog products.
          </p>
        </div>
        <AttributeCreateDialog onCreateAttribute={onCreateAttribute} />
      </div>

      {error ? (
        <Card className='border-destructive/50'>
          <CardContent className='text-destructive py-6 text-sm'>
            {error}
          </CardContent>
        </Card>
      ) : (
        <div className='space-y-6'>
          <Card>
            <CardHeader className='pb-3'>
              <CardTitle className='text-base'>Attributes Directory</CardTitle>
              <CardDescription>
                Catalog-wide typed attributes with filtering, sorting, and
                localized values support.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <AttributesTable
                attributes={attributes}
                onCreateOption={onCreateOption}
              />
            </CardContent>
          </Card>

          <AttributeSchemasCard
            schemas={schemas}
            onCreateSchema={onCreateSchema}
          />
        </div>
      )}
    </div>
  );
}
