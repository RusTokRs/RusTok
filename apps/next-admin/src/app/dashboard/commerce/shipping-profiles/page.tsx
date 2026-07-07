import { auth } from '@/auth';
import { ShippingProfilesClient } from '@/modules/commerce-admin-client';

export const metadata = {
  title: 'Dashboard: Shipping Profiles'
};

export default async function Page() {
  const session = await auth();
  const opts = {
    token: session?.user?.rustokToken ?? null,
    tenantSlug: session?.user?.tenantSlug ?? null,
    tenantId: session?.user?.tenantId ?? null
  };

  return <ShippingProfilesClient opts={opts} />;
}
