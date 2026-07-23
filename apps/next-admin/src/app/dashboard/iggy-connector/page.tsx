import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { IggyConnectorPage } from '@rustok/iggy-connector-admin';

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Iggy Connector'
      pageDescription='Choose bundled Iggy or connect to an external deployment.'
    >
      <IggyConnectorPage token={token} tenantSlug={tenantSlug} />
    </PageContainer>
  );
}
