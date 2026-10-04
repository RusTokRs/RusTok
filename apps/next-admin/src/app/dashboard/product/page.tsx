import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { listProducts, ProductTable } from '@rustok/product-admin';
import { Button } from '@/shared/ui/shadcn/button';
import { Card, CardContent } from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';
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

function toPositiveInt(value: string | undefined, fallback: number): number {
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

export default async function ProductPage({ searchParams }: PageProps) {
  const session = await auth();
  const params = await searchParams;
  const page = toPositiveInt(pickParam(params.page), 1);
  const perPage = toPositiveInt(pickParam(params.perPage), 20);
  const search =
    pickParam(params.title)?.trim() ||
    pickParam(params.search)?.trim() ||
    undefined;

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let result: Awaited<ReturnType<typeof listProducts>> | null = null;
  let error: string | null = null;

  try {
    result = await listProducts(opts, { page, perPage, search });
  } catch (err) {
    error = err instanceof Error ? err.message : 'Failed to load products.';
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
