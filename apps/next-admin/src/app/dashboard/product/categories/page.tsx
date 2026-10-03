import { auth } from '@/auth';
import { PageContainer } from '@/widgets/app-shell';
import { CategoriesPage } from '@rustok/product-admin';
import { createCategoryAction } from './actions';

export const metadata = {
  title: 'RusTok Admin: Categories'
};

export default async function Page() {
  const session = await auth();
  const token = session?.user?.rustokToken ?? null;
  const tenantSlug = session?.user?.tenantSlug ?? null;
  const tenantId = session?.user?.tenantId ?? null;

  return (
    <PageContainer
      pageTitle='Categories'
      pageDescription='Organize your catalog with hierarchical categories and attribute schemas.'
    >
      <CategoriesPage
        token={token}
        tenantSlug={tenantSlug}
        tenantId={tenantId}
        onCreateCategory={createCategoryAction}
      />
    </PageContainer>
  );
}
