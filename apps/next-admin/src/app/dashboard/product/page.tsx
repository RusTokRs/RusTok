import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import {
  listProducts,
  normalizeProductAdminPagination,
  normalizeProductStatus,
  ProductTable
} from '@rustok/product-admin';
import { Button } from '@/shared/ui/shadcn/button';
import { Card, CardContent } from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';
import { redirect } from 'next/navigation';
import { Plus } from 'lucide-react';

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
    .filter(Boolean);
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

  let result: Awaited<ReturnType<typeof listProducts>> | null = null;
  let error: string | null = null;

  try {
    result = await listProducts(opts, {
      page,
      perPage,
      search,
      status,
      vendor,
      categoryId,
      sortBy,
      sortDirection,
      attributeFilters
    });
  } catch {
    // Transport failures can carry server-controlled details. The view
    // displays only an operator-safe message, never a raw GraphQL payload.
    error =
      !opts.token || !opts.tenantSlug || !opts.tenantId
        ? 'Sign in again to manage products.'
        : 'Product catalog is temporarily unavailable.';
  }

  const products = result?.items ?? [];
  const total = result?.total ?? 0;

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
          <ProductTable data={products} totalItems={total} />
        )}
      </div>
    </PageContainer>
  );
}
