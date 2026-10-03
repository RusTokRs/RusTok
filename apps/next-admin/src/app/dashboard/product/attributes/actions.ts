'use server';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  createProductAttribute,
  createProductAttributeOption,
  createProductAttributeSchema,
  type CreateProductAttributePayload,
  type CreateProductAttributeOptionPayload,
  type CreateProductAttributeSchemaPayload
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

export async function createAttributeAction(payload: CreateProductAttributePayload) {
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
