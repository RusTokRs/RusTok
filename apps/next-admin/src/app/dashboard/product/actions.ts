'use server';

import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { createProduct } from '@rustok/product-admin';
import { revalidatePath } from 'next/cache';
import { redirect } from 'next/navigation';

export async function createProductAction(formData: FormData) {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  if (!token || !tenantSlug || !tenantId) {
    throw new Error('Sign in again to create products.');
  }

  const title = (formData.get('title') as string)?.trim();
  if (!title) {
    throw new Error('Title is required');
  }

  const handle = (formData.get('handle') as string)?.trim() || undefined;
  const description =
    (formData.get('description') as string)?.trim() || undefined;
  const vendor = (formData.get('vendor') as string)?.trim() || undefined;
  const productType =
    (formData.get('productType') as string)?.trim() || 'simple';
  const sku = (formData.get('sku') as string)?.trim() || undefined;
  const priceRaw = Number(formData.get('price'));
  const priceAmount =
    Number.isFinite(priceRaw) && priceRaw > 0 ? Math.round(priceRaw * 100) : 0;
  const currencyCode =
    (formData.get('currencyCode') as string)?.trim() || 'USD';
  const inventoryQuantity = Number(formData.get('inventoryQuantity')) || 0;
  const publish =
    formData.get('publish') === 'true' || formData.get('publish') === 'on';
  const tagsRaw = (formData.get('tags') as string)?.trim();
  const tags = tagsRaw
    ? tagsRaw
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
    : [];

  const opts = {
    graphql: graphqlRequest,
    token,
    tenantSlug,
    tenantId
  };

  let createdId = '';
  try {
    const created = await createProduct(opts, {
      title,
      handle,
      description,
      vendor,
      productType,
      sku,
      priceAmount,
      currencyCode,
      inventoryQuantity,
      tags,
      publish
    });
    createdId = created.id;
  } catch (err) {
    console.error('Failed to create product:', err);
    throw err;
  }

  revalidatePath('/dashboard/product');
  redirect(`/dashboard/product/${createdId}`);
}
