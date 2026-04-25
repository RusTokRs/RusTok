import type { NavItem } from '../../../src/types';
import { graphqlRequest } from '../../../src/shared/api/graphql';

export const productNavItems: NavItem[] = [
  {
    title: 'Catalog',
    url: '#',
    icon: 'product',
    isActive: false,
    items: [
      {
        title: 'Products',
        url: '/dashboard/product',
        shortcut: ['p', 'l']
      }
    ],
    access: { role: 'manager' }
  }
];

export type ProductListItem = {
  id: string;
  status: string;
  title: string;
  handle: string;
  sellerId: string | null;
  vendor: string | null;
  productType: string | null;
  shippingProfileSlug: string | null;
  tags: string[];
  createdAt: string | null;
  publishedAt: string | null;
};

export type ProductVariant = {
  id: string;
  sku: string | null;
  barcode: string | null;
  title: string | null;
  inventoryQuantity: number;
  inventoryPolicy: string;
  inStock: boolean;
  prices: Array<{
    currencyCode: string;
    amount: number;
    compareAtAmount: number | null;
    onSale: boolean;
  }>;
};

export type ProductTranslation = {
  locale: string;
  title: string;
  handle: string;
  description: string | null;
  metaTitle: string | null;
  metaDescription: string | null;
};

export type ProductDetail = {
  id: string;
  status: string;
  sellerId: string | null;
  vendor: string | null;
  productType: string | null;
  shippingProfileSlug: string | null;
  tags: string[];
  createdAt: string | null;
  updatedAt: string | null;
  publishedAt: string | null;
  translations: ProductTranslation[];
  variants: ProductVariant[];
};

type GqlOpts = {
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
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

export async function listProducts(
  opts: GqlOpts,
  filter: { page?: number; perPage?: number; search?: string } = {},
  locale?: string
) {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const data = await graphqlRequest<
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

export async function getProduct(
  opts: GqlOpts,
  id: string,
  locale?: string
) {
  if (!opts.token || !opts.tenantSlug || !opts.tenantId) {
    throw new Error('Sign in again to manage products.');
  }

  const data = await graphqlRequest<
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
