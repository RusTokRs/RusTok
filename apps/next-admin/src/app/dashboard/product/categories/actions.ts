'use server';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  createCatalogCategory,
  type CreateCatalogCategoryPayload
} from '@rustok/product-admin';
import { revalidatePath } from 'next/cache';

export async function createCategoryAction(payload: CreateCatalogCategoryPayload) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  if (!token || !tenantSlug || !tenantId) {
    throw new Error('Sign in again to manage categories.');
  }

  const opts = {
    graphql: graphqlRequest,
    token,
    tenantSlug,
    tenantId
  };

  await createCatalogCategory(opts, payload);
  revalidatePath('/dashboard/product/categories');
}
