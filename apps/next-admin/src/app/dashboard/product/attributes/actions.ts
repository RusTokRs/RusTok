'use server';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  bindCategoryAttribute,
  bindSchemaAttribute,
  createCategoryAttributeGroup,
  createProductAttribute,
  createProductAttributeOption,
  createProductAttributeSchema,
  createProductAttributeSchemaGroup,
  setCatalogCategorySchemaMode,
  type BindCategoryAttributePayload,
  type BindSchemaAttributePayload,
  type CreateCategoryAttributeGroupPayload,
  type CreateProductAttributePayload,
  type CreateProductAttributeOptionPayload,
  type CreateProductAttributeSchemaGroupPayload,
  type CreateProductAttributeSchemaPayload,
  type SetCategorySchemaModePayload
} from '@rustok/product-admin';
import { revalidatePath } from 'next/cache';

async function getSessionOpts() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  if (!token || !tenantSlug || !tenantId) {
    throw new Error('Sign in again to manage attributes.');
  }

  return {
    graphql: graphqlRequest,
    token,
    tenantSlug,
    tenantId
  };
}

export async function createAttributeAction(
  payload: CreateProductAttributePayload
) {
  const opts = await getSessionOpts();
  await createProductAttribute(opts, payload);
  revalidatePath('/dashboard/product/attributes');
}

export async function createAttributeOptionAction(
  payload: CreateProductAttributeOptionPayload
) {
  const opts = await getSessionOpts();
  await createProductAttributeOption(opts, payload);
  revalidatePath('/dashboard/product/attributes');
}

export async function createAttributeSchemaAction(
  payload: CreateProductAttributeSchemaPayload
) {
  const opts = await getSessionOpts();
  await createProductAttributeSchema(opts, payload);
  revalidatePath('/dashboard/product/attributes');
}

async function revalidateSchemaAuthoringPaths() {
  revalidatePath('/dashboard/product/attributes');
  revalidatePath('/dashboard/product/categories');
}

export async function setCategorySchemaModeAction(
  payload: SetCategorySchemaModePayload
) {
  const opts = await getSessionOpts();
  await setCatalogCategorySchemaMode(opts, payload);
  await revalidateSchemaAuthoringPaths();
}

export async function createSchemaAttributeGroupAction(
  payload: CreateProductAttributeSchemaGroupPayload
) {
  const opts = await getSessionOpts();
  await createProductAttributeSchemaGroup(opts, payload);
  await revalidateSchemaAuthoringPaths();
}

export async function bindSchemaAttributeAction(
  payload: BindSchemaAttributePayload
) {
  const opts = await getSessionOpts();
  await bindSchemaAttribute(opts, payload);
  await revalidateSchemaAuthoringPaths();
}

export async function createCategoryAttributeGroupAction(
  payload: CreateCategoryAttributeGroupPayload
) {
  const opts = await getSessionOpts();
  await createCategoryAttributeGroup(opts, payload);
  await revalidateSchemaAuthoringPaths();
}

export async function bindCategoryAttributeAction(
  payload: BindCategoryAttributePayload
) {
  const opts = await getSessionOpts();
  await bindCategoryAttribute(opts, payload);
  await revalidateSchemaAuthoringPaths();
}
