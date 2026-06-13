import { auth } from '@/auth';
import { ShippingProfilesTemplate } from '@rustok/commerce-admin';

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

  return <ShippingProfilesTemplate opts={opts} />;
}
