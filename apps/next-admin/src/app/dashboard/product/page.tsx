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
  return entries
    .flatMap((entry) => entry.split(';'))
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const equalsAt = entry.indexOf('=');
      // The Product owner trims code and value independently. Keep the
      // facet toggles aligned with that canonical selection grammar.
      return equalsAt < 0
        ? entry
        : `${entry.slice(0, equalsAt).trim()}=${entry.slice(equalsAt + 1).trim()}`;
    });
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
  const canonicalStatus = normalizeProductStatus(rawStatus)?.toLowerCase();
  // The shared DataTableShell reads page and perPage from the URL. Canonicalize
  // invalid external links before rendering so its pagination matches the
  // Product owner's bounded GraphQL request.
  if (
    (rawPage !== undefined &&
      (rawPage.trim() !== String(page) ||
        (Array.isArray(params.page) && params.page.length !== 1))) ||
    (rawPerPage !== undefined &&
      (rawPerPage.trim() !== String(perPage) ||
        (Array.isArray(params.perPage) && params.perPage.length !== 1))) ||
    (rawStatus !== undefined &&
      (rawStatus.trim() !== (canonicalStatus ?? '') ||
        (Array.isArray(params.status) && params.status.length !== 1)))
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
    redirect(`/dashboard/product?${query.toString()}`);
  }
  const search =
    pickParam(params.title)?.trim() ||
    pickParam(params.search)?.trim() ||
    undefined;
  const status = canonicalStatus;
  const vendor = pickParam(params.vendor)?.trim() || undefined;
  const categoryId = pickParam(params.category_id)?.trim() || undefined;
  const sortBy = pickParam(params.sort_by)?.trim() || undefined;
  const sortDirection = pickParam(params.sort_direction)?.trim() || undefined;
  const attributeFilters = parseAttributeFilters(params.attribute_filters);

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
