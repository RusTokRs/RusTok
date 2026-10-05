import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { CategoriesPage } from '@rustok/blog-admin';

export const metadata = {
  title: 'Dashboard: Blog Categories'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      scrollable={true}
      pageTitle='Blog Categories'
      pageDescription='Manage categories for organizing blog posts'
    >
      <CategoriesPage
        token={token}
        tenantSlug={tenantSlug}
        tenantId={tenantId}
      />
    </PageContainer>
  );
}
