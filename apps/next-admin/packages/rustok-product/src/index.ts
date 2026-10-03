/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { registerAdminModule } from '@/modules/registry';
import {
  graphqlRequest,
  type AdminGraphqlExecutor,
  type GqlOpts
} from '@/lib/graphql';
import type {
  ProductListItem,
  ProductDetail,
  ProductAttributeSummary,
  CatalogCategorySummary
} from './api/types';
import { productNavItems } from './nav';

registerAdminModule({
  id: 'product',
  name: 'Product Catalog',
  navItems: productNavItems
});

export { productNavItems } from './nav';
export * from './api/types';
export * from './api/categories';
export * from './api/attributes';
export * from './api/products';
export * from './api/relations';
export * from './api/pricing';
export * from './api/bundles';
export * from './components/categories/categories-table';
export * from './components/categories/category-create-dialog';
export * from './components/attributes/attributes-table';
export * from './components/attributes/attribute-create-dialog';
export * from './components/attributes/attribute-options-dialog';
export * from './components/attributes/attribute-schemas-card';
export * from './components/products/product-header-bar';
export * from './components/products/product-general-card';
export * from './components/products/product-category-card';
export * from './components/products/product-variants-card';
export * from './components/products/product-media-card';
export * from './components/products/product-relations-card';
export * from './components/products/product-relation-add-dialog';
export * from './components/products/product-bundle-card';
export * from './components/products/product-bundle-item-dialog';
export * from './components/products/product-seo-card';
export * from './components/bundles/bundles-table';
export * from './components/bundles/bundle-create-dialog';
export * from './pages/categories-page';
export * from './pages/attributes-page';
export * from './pages/product-editor-page';
export * from './pages/bundles-page';

export type ProductCatalogSearchOption = {
  value: string;
  label: string;
};

const PRODUCTS_QUERY = `
query ProductAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) {
  products(tenantId: $tenantId, locale: $locale, filter: $filter) {
    total
    page
    perPage
    hasNext
    items {
      id
      status
      title
      handle
      sellerId
      vendor
      productType
      shippingProfileSlug
      tags
      createdAt
      publishedAt
    }
  }
}`;

const PRODUCT_QUERY = `
query ProductAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) {
  product(tenantId: $tenantId, id: $id, locale: $locale) {
    id
    status
    sellerId
    vendor
    productType
    shippingProfileSlug
    primaryCategoryId
    tags
    createdAt
    updatedAt
    publishedAt
    translations {
      locale
      title
      handle
      description
      metaTitle
      metaDescription
    }
    variants {
      id
      sku
      barcode
      title
      inventoryQuantity
      inventoryPolicy
      inStock
      prices {
        currencyCode
        amount
        compareAtAmount
        onSale
      }
    }
    images {
      id
      mediaId
      url
      altText
      position
    }
  }
}`;

const PRODUCT_ATTRIBUTES_QUERY = `
query ProductCatalogSearchAttributes($tenantId: UUID!, $locale: String!) {
  productAttributes(tenantId: $tenantId, locale: $locale) {
    items {
      id
      code
      valueType
      isFilterable
      isSortable
      label
    }
  }
}`;

const CATALOG_CATEGORIES_QUERY = `
query ProductCatalogSearchCategories($tenantId: UUID!, $locale: String!) {
  catalogCategories(tenantId: $tenantId, locale: $locale) {
    items {
      id
      parentId
      code
      slug
      path
      kind
      name
    }
  }
}`;

type ProductsResponse = {
  products: {
    total: number;
    page: number;
    perPage: number;
    hasNext: boolean;
    items: ProductListItem[];
  };
};

type ProductResponse = {
  product: ProductDetail | null;
};

type ProductAttributesResponse = {
  productAttributes: {
    items: ProductAttributeSummary[];
  };
};

type CatalogCategoriesResponse = {
  catalogCategories: {
    items: CatalogCategorySummary[];
  };
};

export async function listProducts(
  opts: GqlOpts,
  filter: { page?: number; perPage?: number; search?: string } = {},
  locale?: string
) {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    {
      tenantId: string;
      locale?: string;
      filter: { page?: number; perPage?: number; search?: string };
    },
    ProductsResponse
  >(
    PRODUCTS_QUERY,
    {
      tenantId: opts.tenantId,
      locale,
      filter
    },
    opts.token,
    opts.tenantSlug
  );

  return data.products;
}

export async function getProduct(opts: GqlOpts, id: string, locale?: string) {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; id: string; locale?: string },
    ProductResponse
  >(
    PRODUCT_QUERY,
    { tenantId: opts.tenantId, id, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.product;
}

export async function listCatalogCategorySearchOptions(
  opts: GqlOpts,
  locale: string
): Promise<ProductCatalogSearchOption[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; locale: string },
    CatalogCategoriesResponse
  >(
    CATALOG_CATEGORIES_QUERY,
    { tenantId: opts.tenantId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.catalogCategories.items.map((category) => ({
    value: category.id,
    label: category.path || category.name || category.code
  }));
}

export async function listCatalogAttributeSearchOptions(
  opts: GqlOpts,
  locale: string
): Promise<ProductCatalogSearchOption[]> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    return [];
  }

  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { tenantId: string; locale: string },
    ProductAttributesResponse
  >(
    PRODUCT_ATTRIBUTES_QUERY,
    { tenantId: opts.tenantId, locale },
    opts.token,
    opts.tenantSlug
  );

  return data.productAttributes.items
    .filter((attribute) => attribute.isFilterable || attribute.isSortable)
    .map((attribute) => ({
      value: attribute.code,
      label: `${attribute.label || attribute.code} (${attribute.code})`
    }));
}

export type CreateProductPayload = {
  title: string;
  handle?: string;
  locale?: string;
  description?: string;
  vendor?: string;
  productType?: string;
  sellerId?: string;
  tags?: string[];
  sku?: string;
  priceAmount?: number;
  currencyCode?: string;
  inventoryQuantity?: number;
  publish?: boolean;
};

const CREATE_PRODUCT_MUTATION = `
mutation ProductAdminCreateProduct($idempotencyKey: String!, $input: CreateProductInput!) {
  createProduct(idempotencyKey: $idempotencyKey, input: $input) {
    id
    status
    title
    handle
    sellerId
    vendor
    productType
    shippingProfileSlug
    tags
    createdAt
    publishedAt
  }
}`;

type CreateProductResponse = {
  createProduct: ProductListItem;
};

export async function createProduct(
  opts: GqlOpts,
  payload: CreateProductPayload
): Promise<ProductListItem> {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to create products.');
  }

  const locale = payload.locale || 'en';
  const handle =
    payload.handle ||
    payload.title
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');

  const input = {
    translations: [
      {
        locale,
        title: payload.title,
        handle: handle || null,
        description: payload.description || null,
        metaTitle: null,
        metaDescription: null
      }
    ],
    variants: [
      {
        sku: payload.sku || null,
        barcode: null,
        shippingProfileSlug: null,
        axisValues: [],
        prices:
          payload.priceAmount !== undefined && payload.priceAmount > 0
            ? [
                {
                  currencyCode: payload.currencyCode || 'USD',
                  amount: payload.priceAmount,
                  compareAtAmount: null
                }
              ]
            : [],
        inventoryQuantity: payload.inventoryQuantity ?? 0,
        inventoryPolicy: 'deny'
      }
    ],
    sellerId: payload.sellerId || null,
    vendor: payload.vendor || null,
    productType: payload.productType || 'simple',
    shippingProfileSlug: null,
    primaryCategoryId: null,
    tags: payload.tags || [],
    publish: Boolean(payload.publish)
  };

  const idempotencyKey = crypto.randomUUID();
  const executor = opts.graphql ?? graphqlRequest;
  const data = await executor<
    { idempotencyKey: string; input: typeof input },
    CreateProductResponse
  >(
    CREATE_PRODUCT_MUTATION,
    { idempotencyKey, input },
    opts.token,
    opts.tenantSlug
  );

  return data.createProduct;
}
