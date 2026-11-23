import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  listProducts,
  listCatalogCategorySearchOptions,
  listCatalogAttributeSearchOptions,
  listAdminCatalogFacets,
  normalizeProductAdminPagination,
  normalizeProductAdminPageForTotal,
  normalizeProductStatus,
  ProductTable
} from '@rustok/product-admin';
import { Button } from '@/shared/ui/shadcn/button';
import { Card, CardContent } from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';
import { redirect } from 'next/navigation';
import { Plus } from 'lucide-react';
import { getLocale } from '@rustok/next-fluent/server';
import {
  EFFECTIVE_LOCALE_HEADER,
  LOCALE_COOKIE,
  locales,
  defaultLocale
} from '@/i18n/config';

export const metadata = {
  title: 'RusTok Admin: Catalog'
};

type PageProps = {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

function pickParam(value: string | string[] | undefined): string | undefined {
  return Array.isArray(value) ? value[0] : value;
}

function parseAttributeFilters(value: string | string[] | undefined): string[] {
  const entries = Array.isArray(value) ? value : value ? [value] : [];
  const normalized = entries
    .flatMap((entry) => entry.split(';'))
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const equalsAt = entry.indexOf('=');
      // The Product owner trims code and value independently. Keep the
      // facet toggles aligned with that canonical selection grammar.
      return equalsAt < 0
        ? entry
        : `${entry.slice(0, equalsAt).trim().toLowerCase()}=${entry.slice(equalsAt + 1).trim()}`;
    });
  return [...new Set(normalized)];
}

export default async function ProductPage({ searchParams }: PageProps) {
  const session = await auth();
  const params = await searchParams;
  const rawPage = pickParam(params.page);
  const rawPerPage = pickParam(params.perPage);
  const { page, perPage } = normalizeProductAdminPagination({
    page: rawPage,
    perPage: rawPerPage
  });
  const rawStatus = pickParam(params.status);
  const trimmedStatus = rawStatus?.trim() ?? '';
  const canonicalStatus = normalizeProductStatus(trimmedStatus)?.toLowerCase();
  const hasNonCanonicalStatus =
    params.status !== undefined &&
    (Array.isArray(params.status) && params.status.length !== 1
      ? true
      : !trimmedStatus ||
        rawStatus !== trimmedStatus ||
        trimmedStatus !== (canonicalStatus ?? ''));
  const rawTitle = pickParam(params.title);
  const rawSearch = pickParam(params.search);
  const trimmedTitle = rawTitle?.trim() ?? '';
  const search = trimmedTitle || rawSearch?.trim() || undefined;
  const hasNonCanonicalTitle =
    params.title !== undefined &&
    (Array.isArray(params.title) && params.title.length !== 1
      ? true
      : !trimmedTitle ||
        rawTitle !== trimmedTitle ||
        trimmedTitle !== (search ?? ''));
  const hasLegacySearch = params.search !== undefined;
  const rawVendor = pickParam(params.vendor);
  const vendor = rawVendor?.trim() || undefined;
  const hasNonCanonicalVendor =
    params.vendor !== undefined &&
    (Array.isArray(params.vendor) && params.vendor.length !== 1
      ? true
      : !vendor || rawVendor !== vendor);
  const rawCategoryId = pickParam(params.category_id);
  const categoryId = rawCategoryId?.trim().toLowerCase() || undefined;
  const hasNonCanonicalCategoryId =
    params.category_id !== undefined &&
    (Array.isArray(params.category_id) && params.category_id.length !== 1
      ? true
      : !categoryId || rawCategoryId !== categoryId);
  const rawSortBy = pickParam(params.sort_by);
  const sortBy = rawSortBy?.trim().toLowerCase() || undefined;
  const hasNonCanonicalSortBy =
    params.sort_by !== undefined &&
    (Array.isArray(params.sort_by) && params.sort_by.length !== 1
      ? true
      : !sortBy || rawSortBy !== sortBy);
  const rawSortDirection = pickParam(params.sort_direction);
  const sortDirection = rawSortDirection?.trim().toLowerCase() || undefined;
  const hasNonCanonicalSortDirection =
    params.sort_direction !== undefined &&
    (Array.isArray(params.sort_direction) && params.sort_direction.length !== 1
      ? true
      : !sortDirection || rawSortDirection !== sortDirection);
  const rawAttributeFilters = Array.isArray(params.attribute_filters)
    ? params.attribute_filters.join(';')
    : params.attribute_filters ?? '';
  const attributeFilters = parseAttributeFilters(params.attribute_filters);
  const canonicalAttributeFilters = attributeFilters.join(';');
  const hasNonCanonicalAttributeFilters =
    params.attribute_filters !== undefined &&
    ((Array.isArray(params.attribute_filters) &&
      params.attribute_filters.length !== 1) ||
      !canonicalAttributeFilters ||
      rawAttributeFilters !== canonicalAttributeFilters);

  // Canonicalize route-owned filters before hydration. The table owns `title`,
  // while older links may still use `search`; facets, sorting and column
  // filters must all describe the same owner query after this boundary.
  if (
    (rawPage !== undefined &&
      (rawPage.trim() !== String(page) ||
        (Array.isArray(params.page) && params.page.length !== 1))) ||
    (rawPerPage !== undefined &&
      (rawPerPage.trim() !== String(perPage) ||
        (Array.isArray(params.perPage) && params.perPage.length !== 1))) ||
    hasNonCanonicalStatus ||
    hasNonCanonicalTitle ||
    hasLegacySearch ||
    hasNonCanonicalVendor ||
    hasNonCanonicalCategoryId ||
    hasNonCanonicalSortBy ||
    hasNonCanonicalSortDirection ||
    hasNonCanonicalAttributeFilters
  ) {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (Array.isArray(value)) {
        for (const entry of value) query.append(key, entry);
      } else if (value !== undefined) {
        query.append(key, value);
      }
    }
    query.set('page', String(page));
    query.set('perPage', String(perPage));
    if (canonicalStatus) {
      query.set('status', canonicalStatus);
    } else {
      query.delete('status');
    }
    if (search) {
      query.set('title', search);
    } else {
      query.delete('title');
    }
    query.delete('search');
    if (vendor) {
      query.set('vendor', vendor);
    } else {
      query.delete('vendor');
    }
    if (categoryId) {
      query.set('category_id', categoryId);
    } else {
      query.delete('category_id');
    }
    if (sortBy) {
      query.set('sort_by', sortBy);
    } else {
      query.delete('sort_by');
    }
    if (sortDirection) {
      query.set('sort_direction', sortDirection);
    } else {
      query.delete('sort_direction');
    }
    if (canonicalAttributeFilters) {
      query.set('attribute_filters', canonicalAttributeFilters);
    } else {
      query.delete('attribute_filters');
    }
    redirect(`/dashboard/product?${query.toString()}`);
  }
  const status = canonicalStatus;

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  const locale = await getLocale({
    headerName: EFFECTIVE_LOCALE_HEADER,
    cookieName: LOCALE_COOKIE,
    locales,
    defaultLocale
  });
  const filter = {
    page,
    perPage,
    search,
    status,
    vendor,
    categoryId,
    sortBy,
    sortDirection,
    attributeFilters
  };
  let result: Awaited<ReturnType<typeof listProducts>> | null = null;
  let error: string | null = null;

  try {
    result = await listProducts(opts, filter, locale);
  } catch {
    // Transport failures can carry server-controlled details. The view
    // displays only an operator-safe message, never a raw GraphQL payload.
    error =
      !opts.token || !opts.tenantSlug || !opts.tenantId
        ? 'Sign in again to manage products.'
        : 'Product catalog is temporarily unavailable.';
  }

  if (result) {
    const normalizedPage = normalizeProductAdminPageForTotal(
      page,
      result.total,
      perPage
    );
    if (normalizedPage !== page) {
      const query = new URLSearchParams();
      for (const [key, value] of Object.entries(params)) {
        if (Array.isArray(value)) {
          for (const entry of value) query.append(key, entry);
        } else if (value !== undefined) {
          query.append(key, value);
        }
      }
      query.set('page', String(normalizedPage));
      query.set('perPage', String(perPage));
      redirect(`/dashboard/product?${query.toString()}`);
    }
  }

  const products = result?.items ?? [];
  const total = result?.total ?? 0;
  const categoryOptions = result
    ? await listCatalogCategorySearchOptions(opts, locale).catch(() => [])
    : [];
  const attributeOptions = result
    ? await listCatalogAttributeSearchOptions(opts, locale).catch(() => [])
    : [];
  // Only eight facets can be counted per owner request. Prefer already selected
  // attributes so their clear/toggle controls do not disappear in a large schema.
  const selectedFacetCodes = new Set(
    attributeFilters
      .filter((entry) => entry.includes('='))
      .map((entry) => entry.slice(0, entry.indexOf('=')).toLowerCase())
  );
  const orderedFacetCodes = [
    ...attributeOptions
      .filter((option) => selectedFacetCodes.has(option.value.toLowerCase()))
      .map((option) => option.value),
    ...attributeOptions
      .filter((option) => !selectedFacetCodes.has(option.value.toLowerCase()))
      .map((option) => option.value)
  ];
  const facets = result
    ? await listAdminCatalogFacets(
        opts,
        filter,
        orderedFacetCodes,
        locale
      ).catch(() => [])
    : [];
  const routeQuery: Record<string, string> = Object.fromEntries(
    Object.entries(params).flatMap(([key, value]) => {
      const selected = pickParam(value);
      return selected === undefined ? [] : [[key, selected]];
    })
  );
  if (attributeFilters.length > 0) {
    routeQuery.attribute_filters = attributeFilters.join(';');
  }

  return (
    <PageContainer
      scrollable={false}
      pageTitle='Product catalog'
      pageDescription='Module-owned RusTok product read-side backed by GraphQL.'
      pageHeaderAction={
        <Button asChild>
          <Link href='/dashboard/product/new'>
            <Plus className='mr-1.5 h-4 w-4' />
            Add Product
          </Link>
        </Button>
      }
    >
      <div className='flex flex-1 flex-col space-y-4'>
        {error ? (
          <Card className='border-destructive/50'>
            <CardContent className='text-destructive py-6 text-sm'>
              {error}
            </CardContent>
          </Card>
        ) : (
          <ProductTable
            data={products}
            totalItems={total}
            categoryOptions={categoryOptions}
            attributeOptions={attributeOptions}
            facets={facets}
            categoryId={categoryId}
            attributeFilters={attributeFilters}
            sortBy={sortBy}
            sortDirection={sortDirection}
            perPage={perPage}
            routeQuery={routeQuery}
          />
        )}
      </div>
    </PageContainer>
  );
}
