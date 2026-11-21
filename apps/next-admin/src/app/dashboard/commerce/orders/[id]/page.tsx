import { auth } from '@/auth';
import { graphqlRequest } from '@/shared/api/graphql';
import { getOrderDetail, OrderDetailView } from '@rustok/commerce-admin';
import { Card, CardContent } from '@/shared/ui/shadcn/card';
import { PageContainer } from '@/widgets/app-shell';
import Link from 'next/link';

export const metadata = {
  title: 'RusTok Admin: Order Detail'
};

type PageProps = {
  params: Promise<{ id: string }>;
};

export default async function OrderDetailPage({ params }: PageProps) {
  const session = await auth();
  const { id } = await params;

  const opts = {
    graphql: graphqlRequest,
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  let detail: Awaited<ReturnType<typeof getOrderDetail>> = null;
  let error: string | null = null;

  try {
    detail = await getOrderDetail(opts, id);
  } catch (err) {
    error = err instanceof Error ? err.message : 'Failed to load the order.';
  }

  return (
    <PageContainer
      scrollable
      pageTitle='Order Detail'
      pageDescription='Order summary, line items, payment collection, and fulfillment snapshot.'
    >
      <div className='flex flex-1 flex-col space-y-4'>
        {error ? (
          <Card className='border-destructive/50'>
            <CardContent className='text-destructive py-6 text-sm'>
              {error}
            </CardContent>
          </Card>
        ) : detail ? (
          <OrderDetailView detail={detail} />
        ) : (
          <Card>
            <CardContent className='text-muted-foreground flex flex-col gap-2 py-6 text-sm'>
              <span>Order not found. It may have been removed, or the id is invalid.</span>
              <Link
                href='/dashboard/commerce/orders'
                className='text-primary text-sm underline'
              >
                Back to orders
              </Link>
            </CardContent>
          </Card>
        )}
      </div>
    </PageContainer>
  );
}
