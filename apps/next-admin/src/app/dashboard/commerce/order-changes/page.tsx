import { auth } from '@/auth';
import { OrderChangesTemplate } from '@rustok/commerce-admin';

export const metadata = {
  title: 'Dashboard: Order Changes'
};

export default async function Page() {
  const session = await auth();
  const opts = {
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  return <OrderChangesTemplate opts={opts} />;
}
