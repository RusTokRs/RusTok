import { auth } from '@/auth';
import { CartPromotionsClient } from '@/modules/commerce-admin-client';

export const metadata = {
  title: 'Dashboard: Cart Promotions'
};

export default async function Page() {
  const session = await auth();
  const opts = {
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  return <CartPromotionsClient opts={opts} />;
}
