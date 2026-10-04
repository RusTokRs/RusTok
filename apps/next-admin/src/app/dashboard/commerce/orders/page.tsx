import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { listOrders, OrdersTable } from '@rustok/commerce-admin';
import { Card, CardContent } from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';

export const metadata = {
  title: 'RusTok Admin: Orders'
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

export default async function OrdersPage({ searchParams }: PageProps) {
  const session = await auth();
  const params = await searchParams;
  const page = toPositiveInt(pickParam(params.page), 1);
  const perPage = toPositiveInt(pickParam(params.perPage), 20);
  const status = pickParam(params.status)?.trim() || undefined;

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let result: Awaited<ReturnType<typeof listOrders>> | null = null;
  let error: string | null = null;

  try {
    result = await listOrders(opts, { page, perPage, status });
  } catch (err) {
    error = err instanceof Error ? err.message : 'Failed to load orders.';
  }

  const orders = result?.items ?? [];
  const total = result?.total ?? 0;

  return (
    <PageContainer
      scrollable={false}
      pageTitle='Orders'
      pageDescription='View and manage store orders, fulfillment status, and customer purchases.'
    >
      <div className='flex flex-1 flex-col space-y-4'>
        {error ? (
          <Card className='border-destructive/50'>
            <CardContent className='text-destructive py-6 text-sm'>
              {error}
            </CardContent>
          </Card>
        ) : (
          <OrdersTable data={orders} totalItems={total} />
        )}
      </div>
    </PageContainer>
  );
}
