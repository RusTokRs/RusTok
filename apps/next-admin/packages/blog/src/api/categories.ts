import { graphqlRequest } from '@/lib/graphql';
import type { GqlOpts } from './posts';
export type { GqlOpts } from './posts';

// ---------- Types (matches GraphQL schema) ----------

export interface BlogCategory {
  id: string;
  locale: string;
  effectiveLocale: string;
  name: string;
  slug: string;
  description: string | null;
  parentId: string | null;
  postsCount: number;
  displayOrder: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface BlogCategoryListResponse {
  items: BlogCategory[];
  total: number;
}

export interface BlogCategoriesFilter {
  locale?: string;
  parentId?: string;
  isActive?: boolean;
  page?: number;
  perPage?: number;
}

export interface CreateBlogCategoryInput {
  locale: string;
  name: string;
  slug?: string;
  description?: string;
  parentId?: string;
  displayOrder?: number;
  isActive?: boolean;
}

export interface UpdateBlogCategoryInput {
  locale?: string;
  name?: string;
  slug?: string;
  description?: string;
  parentId?: string;
  displayOrder?: number;
  isActive?: boolean;
}

export interface BlogTag {
  name: string;
  slug: string;
  postsCount: number;
}

export interface BlogTagListResponse {
  items: BlogTag[];
  total: number;
}

export interface BlogTagsFilter {
  locale?: string;
  page?: number;
  perPage?: number;
}

// ---------- GraphQL queries & mutations ----------

export const BLOG_CATEGORIES_QUERY = `
query BlogCategories($tenantId: UUID, $filter: BlogCategoriesFilter) {
  blogCategories(tenantId: $tenantId, filter: $filter) {
    items {
      id
      locale
      effectiveLocale
      name
      slug
      description
      parentId
      postsCount
      displayOrder
      isActive
      createdAt
      updatedAt
    }
    total
  }
}
`;

export const BLOG_CATEGORY_QUERY = `
query BlogCategory($tenantId: UUID, $id: UUID!) {
  blogCategory(tenantId: $tenantId, id: $id) {
    id
    locale
    effectiveLocale
    name
    slug
    description
    parentId
    postsCount
    displayOrder
    isActive
    createdAt
    updatedAt
  }
}
`;

export const CREATE_BLOG_CATEGORY_MUTATION = `
mutation CreateBlogCategory($tenantId: UUID, $input: CreateBlogCategoryInput!) {
  createBlogCategory(tenantId: $tenantId, input: $input)
}
`;

export const UPDATE_BLOG_CATEGORY_MUTATION = `
mutation UpdateBlogCategory($id: UUID!, $tenantId: UUID, $input: UpdateBlogCategoryInput!) {
  updateBlogCategory(id: $id, tenantId: $tenantId, input: $input)
}
`;

export const DELETE_BLOG_CATEGORY_MUTATION = `
mutation DeleteBlogCategory($id: UUID!, $tenantId: UUID) {
  deleteBlogCategory(id: $id, tenantId: $tenantId)
}
`;

export const BLOG_TAGS_QUERY = `
query BlogTags($tenantId: UUID, $filter: BlogTagsFilter) {
  blogTags(tenantId: $tenantId, filter: $filter) {
    items {
      name
      slug
      postsCount
    }
    total
  }
}
`;

// ---------- Response types ----------

interface BlogCategoriesQueryResponse {
  blogCategories: BlogCategoryListResponse;
}

interface BlogCategoryQueryResponse {
  blogCategory: BlogCategory | null;
}

interface CreateBlogCategoryResponse {
  createBlogCategory: string;
}

interface UpdateBlogCategoryResponse {
  updateBlogCategory: boolean;
}

interface DeleteBlogCategoryResponse {
  deleteBlogCategory: boolean;
}

interface BlogTagsQueryResponse {
  blogTags: BlogTagListResponse;
}

// ---------- API functions ----------

export async function listBlogCategories(
  filter: BlogCategoriesFilter = {},
  opts: GqlOpts = {}
): Promise<BlogCategoryListResponse> {
  const data = await graphqlRequest<
    { tenantId?: string | null; filter?: BlogCategoriesFilter },
    BlogCategoriesQueryResponse
  >(
    BLOG_CATEGORIES_QUERY,
    {
      tenantId: opts.tenantId,
      filter: Object.keys(filter).length > 0 ? filter : undefined
    },
    opts.token,
    opts.tenantSlug
  );
  return data.blogCategories;
}

export async function getBlogCategory(
  id: string,
  opts: GqlOpts = {}
): Promise<BlogCategory | null> {
  const data = await graphqlRequest<
    { tenantId?: string | null; id: string },
    BlogCategoryQueryResponse
  >(
    BLOG_CATEGORY_QUERY,
    { tenantId: opts.tenantId, id },
    opts.token,
    opts.tenantSlug
  );
  return data.blogCategory;
}

export async function createBlogCategory(
  input: CreateBlogCategoryInput,
  opts: GqlOpts = {}
): Promise<string> {
  const data = await graphqlRequest<
    { tenantId?: string | null; input: CreateBlogCategoryInput },
    CreateBlogCategoryResponse
  >(
    CREATE_BLOG_CATEGORY_MUTATION,
    { tenantId: opts.tenantId, input },
    opts.token,
    opts.tenantSlug
  );
  return data.createBlogCategory;
}

export async function updateBlogCategory(
  id: string,
  input: UpdateBlogCategoryInput,
  opts: GqlOpts = {}
): Promise<boolean> {
  const data = await graphqlRequest<
    { id: string; tenantId?: string | null; input: UpdateBlogCategoryInput },
    UpdateBlogCategoryResponse
  >(
    UPDATE_BLOG_CATEGORY_MUTATION,
    { id, tenantId: opts.tenantId, input },
    opts.token,
    opts.tenantSlug
  );
  return data.updateBlogCategory;
}

export async function deleteBlogCategory(
  id: string,
  opts: GqlOpts = {}
): Promise<boolean> {
  const data = await graphqlRequest<
    { id: string; tenantId?: string | null },
    DeleteBlogCategoryResponse
  >(
    DELETE_BLOG_CATEGORY_MUTATION,
    { id, tenantId: opts.tenantId },
    opts.token,
    opts.tenantSlug
  );
  return data.deleteBlogCategory;
}

export async function listBlogTags(
  filter: BlogTagsFilter = {},
  opts: GqlOpts = {}
): Promise<BlogTagListResponse> {
  const data = await graphqlRequest<
    { tenantId?: string | null; filter?: BlogTagsFilter },
    BlogTagsQueryResponse
  >(
    BLOG_TAGS_QUERY,
    {
      tenantId: opts.tenantId,
      filter: Object.keys(filter).length > 0 ? filter : undefined
    },
    opts.token,
    opts.tenantSlug
  );
  return data.blogTags;
}
