import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { PageBuilder } from '@/features/blog';

export const metadata = {
  title: 'Dashboard: Page Builder'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      scrollable
      pageTitle='Page Builder'
      pageDescription='Compose and reorder blocks for Pages module payloads.'
    >
      <PageBuilder
        pageId='00000000-0000-0000-0000-000000000000'
        gqlOpts={{ token, tenantSlug, tenantId: tenantId ?? undefined }}
      />
    </PageContainer>
  );
}
